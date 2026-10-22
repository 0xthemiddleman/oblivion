export * as OpencodeVoice from "./opencode-voice.js"

import { Schema } from "effect"

// Hosted voice models the Console starts for the caller's workspace, the inference base that creates them, and the
// workspace's display name, which older Consoles omit. Both Console response formats offer it in this shape.
export const Offer = Schema.Struct({
  baseURL: Schema.String,
  models: Schema.Array(Schema.String),
  workspace: Schema.String.pipe(Schema.optional),
})
export type Offer = typeof Offer.Type
