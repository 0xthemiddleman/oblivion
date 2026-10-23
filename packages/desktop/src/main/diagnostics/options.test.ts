import { expect, test } from "bun:test"
import { memoryLogInterval, remoteDebuggingPort } from "./options"

test.each([
  [undefined, undefined],
  ["", undefined],
  ["0", undefined],
  ["-1", undefined],
  ["NaN", undefined],
  ["Infinity", undefined],
  ["0.5", undefined],
  ["2147484", undefined],
  ["1", 1000],
  ["2.5", 2500],
  ["60", 60000],
] as const)("memory interval %s is opt-in and bounded", (value, expected) => {
  expect(memoryLogInterval(value)).toBe(expected)
})

test.each([
  [true, undefined, undefined],
  [false, undefined, "9222"],
  [true, "9333", "9333"],
  [false, "9333", "9333"],
  [true, "", undefined],
  [true, "0", undefined],
  [true, "65536", undefined],
  [true, "9222junk", undefined],
  [true, "127.0.0.1:9222", undefined],
] as const)("packaged=%s remote port=%s", (packaged, value, expected) => {
  expect(remoteDebuggingPort(packaged, value)).toBe(expected)
})
