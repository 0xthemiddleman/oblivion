export * as ConfigIntegration from "./integration.js"

import { Config } from "@opencode/schema/config"
import { ConfigProvider } from "@opencode/schema/config/provider"
import { Connection } from "@opencode/schema/connection"
import { IntegrationID } from "@opencode/schema/integration-id"

/**
 * A config document delivered by an integration connection, such as the OpenCode Console's providers. It layers
 * below user and project config, and its providers authenticate with, and only exist for, that connection.
 */
export class Document extends Config.Document.extend<Document>("Config.IntegrationDocument")({
  integrationID: IntegrationID,
  connection: Connection.Info,
}) {}

/**
 * Builds an integration document from the allow-listed keys of a remote config, only `providers` today, so a remote
 * source can never load plugins, run commands, or change policy through config.
 */
export function make(input: {
  readonly integrationID: IntegrationID
  readonly connection: Connection.Info
  readonly info: { readonly providers?: Readonly<Record<string, ConfigProvider.Info>> }
}) {
  return new Document({
    type: "document",
    integrationID: input.integrationID,
    connection: input.connection,
    info: new Config.Info({
      providers: Object.fromEntries(
        Object.entries(input.info.providers ?? {}).map(([id, provider]) => [
          id,
          new ConfigProvider.Info({
            ...provider,
            // The connection supplies credentials; `env` would read local variables as this provider's key.
            env: undefined,
            settings: withoutCredentials(provider.settings),
            models:
              provider.models &&
              Object.fromEntries(
                Object.entries(provider.models).map(([modelID, model]) => [
                  modelID,
                  {
                    ...model,
                    settings: withoutCredentials(model.settings),
                    variants: model.variants?.map((variant) => ({
                      ...variant,
                      settings: withoutCredentials(variant.settings),
                    })),
                  },
                ]),
              ),
          }),
        ]),
      ),
    }),
  })
}

function withoutCredentials<Value>(settings: Readonly<Record<string, Value>> | undefined) {
  return (
    settings &&
    Object.fromEntries(
      Object.entries(settings).filter(([key]) => !["apiKey", "authToken", "accessToken"].includes(key)),
    )
  )
}
