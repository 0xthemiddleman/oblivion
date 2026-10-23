import { app, type BrowserWindow } from "electron"
import { mkdir, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { writeHeapSnapshot } from "node:v8"
import type { MainSetup } from "../sdk/main"
import { HeapSnapshots } from "./contract"
import type definition from "./index"
import { captureServer } from "./server-heap"
import { snapshotDirectory } from "./signal"

/** A desktop process the manifest lists: the file written, or why there is none. */
type Snapshot = { readonly process: "main" | "window"; readonly pid: number; readonly window?: number } & (
  | { readonly file: string }
  | { readonly error: string }
)

/** The last capture; the next one starts after it settles. */
type Queue = { tail: Promise<unknown> }

const setup: MainSetup<typeof definition> = (ctx) => {
  const queue: Queue = { tail: Promise.resolve() }

  const capture = async (root: string) => {
    const directory = path.join(root, `opencode-heap-${new Date().toISOString().replace(/[:.]/g, "")}`)

    // Snapshots hold prompts, file contents and credentials. Recursive, because Downloads can be missing on Linux.
    await mkdir(directory, { recursive: true, mode: 0o700 })
    ctx.log.write("info", "capturing heap snapshots", { directory })

    // The server is another process, so it serializes its heap while the windows do. Windows go one at a time: a
    // snapshot can double a process's memory, and a leaking app may not have room for every window at once. Main goes
    // last: writing its own heap blocks the loop that receives the others.
    const [serverSnapshot, windows] = await Promise.all([
      captureServer(ctx.serverEndpoints.get("sidecar"), directory, ctx.scope.signal),
      (async () => {
        const list: Snapshot[] = []

        for (const window of ctx.windows.list()) list.push(await captureWindow(window, directory))

        return list
      })(),
    ])

    const main = await Promise.resolve()
      .then(() => writeHeapSnapshot(path.join(directory, `main-${process.pid}.heapsnapshot`)))
      .then(
        (file): Snapshot => ({ process: "main", pid: process.pid, file: path.basename(file) }),
        (error: Error): Snapshot => ({ process: "main", pid: process.pid, error: error.message }),
      )

    const snapshots = await Promise.all(
      [serverSnapshot, ...windows, main].map(async (snapshot) => {
        if ("error" in snapshot) return snapshot
        const file = "file" in snapshot ? path.join(directory, snapshot.file) : snapshot.path

        const bytes = await stat(file).then(
          (info) => info.size,
          () => null,
        )

        ctx.log.write("info", "heap snapshot written", { file, bytes })

        return { ...snapshot, bytes }
      }),
    )

    // Written last, so a script can wait for it.
    await writeFile(
      path.join(directory, "manifest.json"),
      JSON.stringify(
        {
          created: new Date().toISOString(),
          app: {
            name: app.getName(),
            version: app.getVersion(),
            packaged: app.isPackaged,
            platform: process.platform,
            arch: process.arch,
            versions: process.versions,
          },
          snapshots,
          // Per-process memory as the OS sees it: a process that grows while its heap does not leaks native memory.
          metrics: app.getAppMetrics(),
          metricsMemoryUnit: "KiB",
          mainMemory: process.memoryUsage(),
        },
        null,
        2,
      ),
      { mode: 0o600 },
    )
    const complete = snapshots.every((snapshot) => !("error" in snapshot))
    ctx.log.write(complete ? "info" : "warn", "heap snapshots captured", { directory, complete })

    return { directory, complete }
  }

  const enqueue = (directory: string) => {
    const run = queue.tail.then(() => capture(directory))
    queue.tail = run.catch(() => undefined)

    return run
  }

  ctx.provide(HeapSnapshots, {
    // A window that closes meanwhile does not cancel the capture.
    write: () => enqueue(app.getPath("downloads")),
  })

  const directory = snapshotDirectory(process.env)

  if (!directory || process.platform === "win32") return
  let capturing = false

  const onSignal = () => {
    // Signals coalesce while a capture is running; repeated signals must not fill the disk with queued dumps.
    if (capturing || ctx.scope.signal.aborted) return
    capturing = true
    void enqueue(directory)
      .catch((error: Error) => ctx.log.write("error", "heap snapshot capture failed", { error: error.message }))
      .finally(() => {
        capturing = false
      })
  }

  process.on("SIGUSR2", onSignal)
  ctx.scope.addFinalizer(() => {
    process.off("SIGUSR2", onSignal)
  })
  ctx.log.write("info", "heap snapshot signal enabled", { pid: process.pid, signal: "SIGUSR2", directory })
}

function captureWindow(window: BrowserWindow, directory: string) {
  const contents = window.webContents
  const pid = contents.getOSProcessId()
  const file = `window-${window.id}-${pid}.heapsnapshot`

  return contents.takeHeapSnapshot(path.join(directory, file)).then(
    (): Snapshot => ({ process: "window", pid, window: window.id, file }),
    (error: Error): Snapshot => ({ process: "window", pid, window: window.id, error: error.message }),
  )
}

export default setup
