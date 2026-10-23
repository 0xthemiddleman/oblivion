import { appendFile } from "node:fs/promises"
import path from "node:path"
import { app, webContents } from "electron"
import log from "electron-log/main.js"
import { Schema } from "effect"
import { memoryRecord, RendererMemory, type RendererRecord } from "./record"

// Runs only when diagnostics are enabled; no preload hooks, observers, or store subscriptions.
const probe = `(() => {
  const memory = performance.memory;
  return {
    jsHeap: memory ? { used: memory.usedJSHeapSize, total: memory.totalJSHeapSize, limit: memory.jsHeapSizeLimit } : null,
    domElements: document.getElementsByTagName('*').length
  };
})()`

export function startMemoryLog(interval: number) {
  const file = path.join(path.dirname(log.transports.file.getFile().path), "memory.jsonl")
  const pending = new WeakSet<Electron.WebContents>()
  let stopped = false
  let timer: ReturnType<typeof setTimeout> | undefined

  const sample = async () => {
    const metrics = app.getAppMetrics()

    const renderers = await Promise.all(
      webContents.getAllWebContents().map(async (contents): Promise<RendererRecord> => {
        const identity = { id: contents.id, pid: contents.getOSProcessId(), type: contents.getType() }

        if (pending.has(contents)) return { ...identity, error: "Previous renderer sample is still pending" }
        pending.add(contents)
        let deadline: ReturnType<typeof setTimeout> | undefined
        // A hung renderer must not stop native-memory logging or accumulate evaluation requests.
        const reading = contents.executeJavaScript(probe).then(Schema.decodeUnknownSync(RendererMemory))
        void reading.finally(() => pending.delete(contents)).catch(() => undefined)

        return Promise.race([
          reading,
          new Promise<never>((_, reject) => {
            deadline = setTimeout(() => reject(new Error("Renderer sample timed out")), 2000)
          }),
        ])
          .then(
            (sample) => ({ ...identity, sample }),
            (error: Error) => ({ ...identity, error: error.message }),
          )
          .finally(() => clearTimeout(deadline))
      }),
    )

    if (!stopped) await appendFile(file, `${JSON.stringify(memoryRecord(metrics, renderers))}\n`, { mode: 0o600 })
  }

  const tick = () => {
    void sample()
      .catch((error: Error) => console.error("memory diagnostics failed", error.message))
      .finally(() => {
        if (!stopped) timer = setTimeout(tick, interval)
      })
  }

  log.info("memory diagnostics enabled", { file, intervalMs: interval })
  timer = setTimeout(tick, interval)

  return () => {
    stopped = true
    clearTimeout(timer)
  }
}
