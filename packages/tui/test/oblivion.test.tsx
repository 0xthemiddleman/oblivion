import { expect, test } from "bun:test"
import { InputRenderable, TextareaRenderable } from "@opentui/core"
import { createTestRenderer } from "@opentui/core/testing"
import { Oblivion } from "@opencode/util/oblivion"
import { createTuiClipboard } from "../src/clipboard"
import { createAppFixture } from "./fixture/app"
import { directory, json } from "./fixture/tui-client"

// Run with bun test --define OBLIVION_PROFILE=true --timeout 30000 test/oblivion.test.tsx.
// The normal suite exercises upstream behavior with the profile compiled out.
test.skipIf(!Oblivion.enabled)("Oblivion never exposes the shared clipboard boundary", async () => {
  const setup = await createTestRenderer({ width: 80, height: 24, kittyKeyboard: true })
  const clipboard = createTuiClipboard(setup.renderer)
  try {
    expect(await clipboard.read()).toBeUndefined()
    await expect(clipboard.write("synthetic clipboard marker")).rejects.toThrow("Clipboard condivisa non disponibile")
  } finally {
    await clipboard.dispose()
    setup.renderer.destroy()
  }
})

test.skipIf(!Oblivion.enabled).each([100, 44])("Oblivion limits the actual app surface at width %i", async (width) => {
  const location = { directory, project: { id: "project", directory, canonical: directory } }
  const mutations: string[] = []
  await using setup = await createAppFixture({
    width,
    height: 30,
    config: {
      animations: false,
      tabs: { enabled: true, mode: "on" },
      debug: { devtools: true },
      keybinds: { "command.palette.show": "f2", "model.list": "f3", "agent.list": "f4" },
    },
    fetch: (url, request) => {
      if (request.method === "POST") mutations.push(url.pathname)
      if (url.pathname === "/api/location") return json(location)
      if (url.pathname === "/api/agent")
        return json({ location, data: [{ id: "build", mode: "primary", hidden: false, permissions: [] }] })
      if (url.pathname === "/api/provider")
        return json({
          location,
          data: [
            { id: "abliteration-ai", name: "Abliteration" },
            { id: "other", name: "Other" },
          ],
        })
      if (url.pathname === "/api/model")
        return json({
          location,
          data: ["abliteration-ai", "other"].map((providerID) => ({
            id: "fixture",
            providerID,
            name: providerID === "abliteration-ai" ? "Allowed model" : "Excluded model",
            variants: [],
            cost: [],
            time: { released: 0 },
          })),
        })
    },
  })
  await setup.ready
  await visible(
    setup,
    (frame) =>
      frame.includes("OBLIVION") &&
      frame.includes("Allowed model") &&
      setup.renderer.currentFocusedRenderable instanceof TextareaRenderable,
  )
  expect(setup.captureCharFrame()).toContain("Conversazioni e documenti")
  expect(setup.captureCharFrame()).not.toContain("DevTools")
  setup.mockInput.pressKey("F4")
  await setup.renderOnce()
  expect(setup.captureCharFrame()).not.toContain("Select agent")
  setup.mockInput.pressKey("F2")
  await visible(
    setup,
    (frame) =>
      frame.includes("Nuova sessione protetta") && setup.renderer.currentFocusedRenderable instanceof InputRenderable,
  )
  const palette = setup.captureCharFrame()
  expect(palette).toContain("Cambia sessione protetta")
  expect(palette).not.toContain("Connect integration")
  expect(palette).not.toContain("Switch agent")
  expect(palette).not.toContain("worktree")
  setup.mockInput.pressEscape()
  await visible(setup, () => setup.renderer.currentFocusedRenderable instanceof TextareaRenderable)
  setup.mockInput.pressKey("F3")
  await visible(
    setup,
    (frame) => frame.includes("Select model") && setup.renderer.currentFocusedRenderable instanceof InputRenderable,
  )
  expect(setup.captureCharFrame()).toContain("Allowed model")
  expect(setup.captureCharFrame()).not.toContain("Excluded model")
  expect(mutations).toEqual([])
})

async function visible(setup: Awaited<ReturnType<typeof createAppFixture>>, predicate: (frame: string) => boolean) {
  const deadline = Date.now() + 3_000
  while (!predicate(setup.captureCharFrame())) {
    if (Date.now() >= deadline) throw new Error(`Oblivion frame not ready:\n${setup.captureCharFrame()}`)
    await Bun.sleep(10)
  }
}
