export * as ConfigIntegration from "./integration.js"

import { Schema } from "effect"
import { Config } from "@opencode/schema/config"
import { ConfigMCP } from "@opencode/schema/config/mcp"
import { ConfigProvider } from "@opencode/schema/config/provider"
import { Connection } from "@opencode/schema/connection"
import { IntegrationID } from "@opencode/schema/integration-id"

/**
 * A config document delivered by an integration connection, such as the OpenCode Console's config. Config loads it
 * right after managed policies, so user and project config override its settings while its policies outrank theirs.
 * Its providers authenticate with, and only exist for, that connection.
 */
export class Document extends Config.Document.extend<Document>("Config.IntegrationDocument")({
  integrationID: IntegrationID,
  connection: Connection.Info,
  /** Organization behind the connection, named in denials by the document's policies. */
  organization: Schema.optional(Schema.String),
}) {}

/**
 * Builds an integration document from the allow-listed keys of a remote config: providers, remote MCP servers,
 * policies, and the web search selection. A remote source can never load plugins, run commands, or change any
 * other local behavior through config.
 */
export function make(input: {
  readonly integrationID: IntegrationID
  readonly connection: Connection.Info
  readonly organization?: string
  readonly info: Config.Info
}) {
  const servers = input.info.mcp?.servers
  return new Document({
    type: "document",
    integrationID: input.integrationID,
    connection: input.connection,
    organization: input.organization,
    info: new Config.Info({
      providers:
        input.info.providers &&
        Object.fromEntries(
          Object.entries(input.info.providers).map(([id, provider]) => [
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
      // Local servers would run a command on this machine.
      mcp:
        servers &&
        new ConfigMCP.Info({
          servers: Object.fromEntries(Object.entries(servers).filter(([, server]) => server.type === "remote")),
        }),
      policies: input.info.policies,
      websearch: input.info.websearch,
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
