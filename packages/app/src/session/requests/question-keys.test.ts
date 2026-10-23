import { describe, expect, test } from "bun:test"
import { questionKey, type QuestionKeyZone } from "./question-keys"

const press = (
  key: string,
  options: {
    zone?: QuestionKeyZone
    mac?: boolean
    count?: number
    repeat?: boolean
    meta?: boolean
    alt?: boolean
    shift?: boolean
  } = {},
) =>
  questionKey(
    {
      key,
      repeat: options.repeat ?? false,
      metaKey: options.meta ?? false,
      ctrlKey: false,
      altKey: options.alt ?? false,
      shiftKey: options.shift ?? false,
      isComposing: false,
    },
    { mac: options.mac ?? true, zone: options.zone ?? "option", count: options.count ?? 3 },
  )

describe("questionKey", () => {
  test("Up and Down move through the answers", () => {
    expect(press("ArrowDown")).toEqual({ type: "move", step: 1 })
    expect(press("ArrowUp", { zone: "custom" })).toEqual({ type: "move", step: -1 })
    expect(press("Home")).toEqual({ type: "edge", to: "first" })
    expect(press("End")).toEqual({ type: "edge", to: "last" })
  })

  test("Left and Right switch questions anywhere but the text field", () => {
    expect(press("ArrowRight")).toEqual({ type: "question", step: 1 })
    expect(press("ArrowLeft", { zone: "custom" })).toEqual({ type: "question", step: -1 })
    expect(press("ArrowRight", { zone: "other" })).toEqual({ type: "question", step: 1 })
    expect(press("ArrowLeft", { zone: "input" })).toEqual({ type: "none" })
    expect(press("ArrowRight", { zone: "input" })).toEqual({ type: "none" })
    expect(press("ArrowRight", { shift: true })).toEqual({ type: "none" })
  })

  test("Up and Down outside the answers leave focus alone", () => {
    expect(press("ArrowDown", { zone: "other" })).toEqual({ type: "none" })
    expect(press("ArrowUp", { zone: "input" })).toEqual({ type: "none" })
  })

  test("Space acts on key down and ignores its repeats", () => {
    expect(press(" ")).toEqual({ type: "activate" })
    expect(press(" ", { zone: "custom" })).toEqual({ type: "activate" })
    expect(press(" ", { repeat: true })).toEqual({ type: "swallow" })
    expect(press(" ", { zone: "input" })).toEqual({ type: "none" })
    expect(press(" ", { zone: "other" })).toEqual({ type: "none" })
  })

  test("number keys pick answers in range, once per press", () => {
    expect(press("1")).toEqual({ type: "number", index: 0 })
    expect(press("3", { zone: "other" })).toEqual({ type: "number", index: 2 })
    expect(press("4")).toEqual({ type: "none" })
    expect(press("2", { repeat: true })).toEqual({ type: "none" })
    expect(press("2", { alt: true })).toEqual({ type: "none" })
    expect(press("2", { zone: "input" })).toEqual({ type: "none" })
  })

  test("the shortcuts work everywhere, including the text field", () => {
    expect(press("Escape", { zone: "input" })).toEqual({ type: "dismiss" })
    expect(press("[", { meta: true, zone: "input" })).toEqual({ type: "question", step: -1 })
    expect(press("ArrowLeft", { alt: true, mac: false })).toEqual({ type: "question", step: -1 })
    expect(press("Enter", { meta: true, zone: "input" })).toEqual({ type: "advance" })
    expect(press("Enter", { meta: true, repeat: true })).toEqual({ type: "none" })
  })

  test("typing on the custom answer's toggle starts the answer", () => {
    expect(press("a", { zone: "custom" })).toEqual({ type: "type" })
    expect(press("a")).toEqual({ type: "none" })
    expect(press("Tab", { zone: "custom" })).toEqual({ type: "none" })
  })
})
