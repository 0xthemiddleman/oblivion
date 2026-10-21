import { Button } from "@opencode/ui/button"
import { Icon } from "@opencode/ui/icon"
import { IconButton } from "@opencode/ui/icon-button"
import { Keybind } from "@opencode/ui/keybind"
import { ScrollView } from "@opencode/ui/scroll-view"
import {
  displayName,
  getProjectAvatarGenerated,
  getProjectAvatarSource,
  getProjectAvatarVariant,
  ProjectAvatar,
} from "@opencode/ui/project-avatar"
import { TextShimmer } from "@opencode/ui/text-shimmer"
import { Tooltip } from "@opencode/ui/tooltip"
import { createEffect, createMemo, For, Match, Show, Switch, type JSX } from "solid-js"
import { useLanguage } from "@/runtime/i18n/language"
import { useGlobal } from "@/runtime/server/runtime"
import { ServerConnection, useServers } from "@/runtime/server/registry"
import { useSettingsCommand } from "@/settings/command"
import { useSettingsSurface } from "@/settings/surface"
import { usePlatform } from "@/runtime/platform/platform"
import { useCommand } from "@/shell/commands/command"
import { SessionTabAvatarView } from "@/shell/layout/session-tab-avatar"
import { sessionTabTitle } from "@/shell/titlebar/tab-title"
import type { InboxEntry } from "@/shell/titlebar/inbox-model"
import { createStartModel, type StartModel, type StartServer } from "./model"

type Language = ReturnType<typeof useLanguage>

/** Home: a greeting, every session sorted by what it needs from you, and recent projects beside them. */
export function StartHome() {
  const model = createStartModel()
  // Settings opens from Home like from a session; the palette is the shell's.
  useSettingsCommand()

  return (
    <div
      data-slot="home-panel"
      class="@container mx-[var(--shell-inline-inset,8px)] mb-[var(--shell-bottom-inset,8px)] mt-[var(--shell-top-inset,8px)] flex min-h-0 flex-1 flex-col self-stretch overflow-hidden rounded-[10px] bg-v2-background-bg-base shadow-[var(--v2-elevation-raised)]"
    >
      <ScrollView class="min-h-0 flex-1">
        <div class="mx-auto grid w-full max-w-[1240px] grid-cols-[minmax(0,880px)] justify-center gap-10 px-6 pb-16 pt-[max(48px,9vh)] @[1180px]:grid-cols-[minmax(0,880px)_240px]">
          <div class="flex min-w-0 flex-col gap-8">
            <StartGreeting model={model} />
            <StartUnreachable model={model} />
            <StartSections model={model} />
          </div>
          <aside class="hidden @[1180px]:block">
            <div class="sticky top-0 pt-[68px]">
              <StartProjects model={model} />
            </div>
          </aside>
        </div>
      </ScrollView>
    </div>
  )
}

function StartGreeting(props: { model: StartModel }) {
  const language = useLanguage()
  const command = useCommand()
  const platform = usePlatform()
  const groups = props.model.groups
  const hour = () => new Date(props.model.inbox.now()).getHours()
  const period = () => (hour() < 12 ? "morning" : hour() < 18 ? "afternoon" : "evening")

  const greeting = () => {
    const name = props.model.name()

    return name ? language.t(`start.greeting.${period()}.named`, { name }) : language.t(`start.greeting.${period()}`)
  }

  // Each count is its own complete phrase, so locales never assemble a sentence from fragments.
  const summary = createMemo(() =>
    [
      groups().needs.length > 0 ? language.plural("start.summary.needsYou", groups().needs.length) : undefined,
      groups().working.length > 0 ? language.plural("start.summary.running", groups().working.length) : undefined,
      groups().done.length > 0 ? language.plural("start.summary.done", groups().done.length) : undefined,
    ].filter((item) => item !== undefined),
  )

  return (
    <div class="flex items-end justify-between gap-4 px-2">
      {/* Both lines keep their height while they wait, so nothing below moves when they fill in. */}
      <div class="min-w-0">
        <h1
          class="truncate text-[20px] font-medium leading-7 text-v2-text-text-base transition-opacity duration-200"
          classList={{ "opacity-0": !props.model.greeted() }}
        >
          {greeting()}
        </h1>
        <p class="flex h-5 items-center truncate text-[13px] leading-5 text-v2-text-text-muted">
          <Show when={!props.model.loading()} fallback={<Bar class="w-44" />}>
            <Show when={summary().length > 0} fallback={language.t("start.caughtUp")}>
              {summary().join(" · ")}
            </Show>
          </Show>
        </p>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <Tooltip placement="bottom" value={language.t("sidebar.help")}>
          <IconButton
            variant="ghost-muted"
            size="large"
            aria-label={language.t("sidebar.help")}
            icon={<Icon name="help" />}
            onClick={() => platform.openExternal("https://opencode.ai/desktop-feedback")}
          />
        </Tooltip>
        <Tooltip
          placement="bottom"
          value={
            <>
              {language.t("sidebar.settings")}
              <Show when={command.keybindParts("settings.open").length > 0}>
                <Keybind keys={command.keybindParts("settings.open")} variant="neutral" />
              </Show>
            </>
          }
        >
          <IconButton
            variant="ghost-muted"
            size="large"
            aria-label={language.t("sidebar.settings")}
            icon={<Icon name="settings-gear" />}
            onClick={() => command.trigger("settings.open")}
          />
        </Tooltip>
        <Tooltip
          placement="bottom"
          value={
            <>
              {language.t("command.session.new")}
              <Show when={command.keybindParts("tab.new").length > 0}>
                <Keybind keys={command.keybindParts("tab.new")} variant="neutral" />
              </Show>
            </>
          }
        >
          <Button variant="neutral" icon="plus" class="ms-1" onClick={props.model.newSession}>
            {language.t("command.session.new")}
          </Button>
        </Tooltip>
      </div>
    </div>
  )
}

/** Servers that do not answer leave their sessions out of every list, so each gets a line saying so. */
function StartUnreachable(props: { model: StartModel }) {
  const language = useLanguage()
  const servers = useServers()
  const global = useGlobal()
  const settings = useSettingsSurface()

  return (
    <Show when={props.model.unreachable().length > 0}>
      <div class="flex flex-col gap-1">
        <For each={props.model.unreachable()}>
          {(conn) => (
            <div class="flex h-9 items-center gap-3 rounded-[8px] bg-v2-background-bg-layer-01 pe-1.5 ps-2 text-[13px] leading-4">
              <span class="flex size-4 shrink-0 items-center justify-center">
                <Icon name="server" size="small" class="text-v2-icon-icon-muted" />
              </span>
              <span class="min-w-0 flex-1 truncate text-v2-text-text-muted">
                {language.t(
                  global.servers.health[ServerConnection.key(conn)]?.unauthorized
                    ? "start.server.unauthorized"
                    : "start.server.unreachable",
                  { server: servers.label(conn) },
                )}
              </span>
              <Button
                size="small"
                variant="ghost-muted"
                onClick={() => settings.openServers(ServerConnection.key(conn))}
              >
                {language.t("start.server.manage")}
              </Button>
            </div>
          )}
        </For>
      </div>
    </Show>
  )
}

/** Needs you, then results waiting to be read, then work in flight, then everything already read. */
function StartSections(props: { model: StartModel }) {
  const language = useLanguage()
  const groups = props.model.groups

  createEffect(() =>
    props.model.inbox.loadBranches([...groups().needs, ...groups().done, ...groups().working, ...groups().earlier]),
  )

  return (
    <Show when={!props.model.loading()} fallback={<Skeleton />}>
      <div class="flex flex-col gap-6">
        <Show when={groups().needs.length > 0} fallback={<InboxZero />}>
          <Section title={language.t("start.section.needsYou")} count={groups().needs.length} icon="warning">
            <For each={groups().needs}>{(entry) => <StartRow model={props.model} entry={entry} />}</For>
          </Section>
        </Show>
        <Show when={groups().done.length > 0}>
          <Section
            title={language.t("start.section.done")}
            count={groups().done.length}
            icon="circle-check"
            action={
              <Button size="small" variant="ghost-muted" onClick={props.model.markAllViewed}>
                {language.t("start.markAllRead")}
              </Button>
            }
          >
            <For each={groups().done}>{(entry) => <StartRow model={props.model} entry={entry} />}</For>
          </Section>
        </Show>
        <Show when={groups().working.length > 0}>
          <Section title={language.t("start.section.running")} count={groups().working.length} icon="status-active">
            <For each={groups().working}>{(entry) => <StartRow model={props.model} entry={entry} />}</For>
          </Section>
        </Show>
        <Show when={groups().earlier.length > 0}>
          <Section title={language.t("start.section.earlier")} icon="reset">
            <For each={groups().earlier}>{(entry) => <StartRow model={props.model} entry={entry} />}</For>
          </Section>
        </Show>
      </div>
    </Show>
  )
}

/** Row-shaped placeholders while the first load settles; claiming inbox zero before then would be wrong. */
function Skeleton() {
  return (
    <div class="flex flex-col gap-1" aria-hidden="true">
      <div class="flex h-7 items-center gap-3 px-2">
        <span class="size-4 shrink-0" />
        <Bar class="w-20" />
      </div>
      <For each={[56, 44, 64, 38]}>
        {(width) => (
          <div class="flex h-12 items-center gap-3 px-2">
            <span class="size-4 shrink-0 animate-pulse rounded-[4px] bg-v2-background-bg-layer-03" />
            <div class="flex min-w-0 flex-1 flex-col gap-1.5">
              <Bar style={{ width: `${width}%` }} />
              <Bar class="w-28" />
            </div>
          </div>
        )}
      </For>
    </div>
  )
}

function Bar(props: { class?: string; style?: JSX.CSSProperties }) {
  return (
    <span
      class={`block h-2.5 max-w-full animate-pulse rounded-full bg-v2-background-bg-layer-03 ${props.class ?? ""}`}
      style={props.style}
    />
  )
}

/** Recent projects per server. Choosing one starts a session there; projects are added on the server, not here. */
function StartProjects(props: { model: StartModel }) {
  const language = useLanguage()
  const servers = useServers()
  const several = () => props.model.projectServers().length > 1

  return (
    <div class="flex flex-col gap-4">
      <For each={props.model.projectServers()}>
        {(server, index) => (
          <div class="flex flex-col gap-1">
            <div class="flex h-7 items-center gap-1.5 px-2 text-[12px] leading-4 font-medium text-v2-text-text-muted">
              <Show when={several()} fallback={language.t("start.projects")}>
                <Icon
                  name={servers.current(server.conn) ? "monitor" : "server"}
                  size="small"
                  class="shrink-0 text-v2-icon-icon-faint"
                />
                <span class="min-w-0 truncate">{servers.label(server.conn)}</span>
              </Show>
              <Show when={!several() && index() === 0 && servers.tag(server.conn)}>
                {(name) => <span class="min-w-0 truncate text-v2-text-text-faint">{name()}</span>}
              </Show>
            </div>
            <ProjectList model={props.model} server={server} />
          </div>
        )}
      </For>
    </div>
  )
}

function ProjectList(props: { model: StartModel; server: StartServer }) {
  const language = useLanguage()

  // Projects that share a name, like a repository and its worktrees, are told apart by their parent folder.
  const repeated = createMemo(() => {
    const names = props.server.projects.map((item) => displayName(item.project))

    return new Set(names.filter((name, index) => names.indexOf(name) !== index))
  })

  const parent = (worktree: string) =>
    worktree
      .replace(/[/\\]+$/, "")
      .split(/[/\\]/)
      .at(-2)

  return (
    <For each={props.server.projects}>
      {(item) => (
        <button
          type="button"
          aria-label={language.t("start.projects.newSession", { project: displayName(item.project) })}
          class="flex h-8 w-full items-center gap-2 rounded-[6px] px-2 text-start text-[13px] leading-4 hover:bg-v2-overlay-simple-overlay-hover focus-visible:bg-v2-overlay-simple-overlay-hover focus-visible:outline-none"
          onClick={() => props.model.openProject(props.server, item)}
        >
          <ProjectAvatar
            class="shrink-0"
            fallback={displayName(item.project)}
            src={getProjectAvatarSource(item.project.id, item.project.icon)}
            variant={getProjectAvatarVariant(item.project.icon?.color)}
            generated={getProjectAvatarGenerated(item.project)}
          />
          <Tooltip value={item.project.worktree} placement="left" class="flex min-w-0 flex-1" openDelay={800}>
            <span class="flex min-w-0 items-baseline gap-1.5">
              <span class="min-w-0 truncate text-v2-text-text-base">{displayName(item.project)}</span>
              <Show when={repeated().has(displayName(item.project))}>
                <span class="min-w-0 shrink-[10] truncate text-[12px] text-v2-text-text-faint">
                  {parent(item.project.worktree)}
                </span>
              </Show>
            </span>
          </Tooltip>
          <ProjectCounts waiting={item.waiting} running={item.running} />
        </button>
      )}
    </For>
  )
}

function ProjectCounts(props: { waiting: number; running: number }) {
  const language = useLanguage()

  return (
    <span class="flex shrink-0 items-center gap-2 text-[12px] leading-4 tabular-nums">
      <Show when={props.waiting}>
        <Tooltip value={language.plural("start.projects.waiting", props.waiting)} placement="top" class="flex">
          <span class="text-v2-state-fg-warning">{props.waiting}</span>
        </Tooltip>
      </Show>
      <Show when={props.running}>
        <Tooltip value={language.plural("start.projects.running", props.running)} placement="top" class="flex">
          <span class="text-v2-text-text-faint">{props.running}</span>
        </Tooltip>
      </Show>
    </span>
  )
}

function Section(props: { title: string; count?: number; icon: string; action?: JSX.Element; children: JSX.Element }) {
  return (
    <section class="flex flex-col gap-1">
      {/* The icon sits in the rows' 16px avatar column so the label lines up with the session titles. */}
      <div class="flex h-7 items-center gap-3 px-2 text-[12px] leading-4 font-medium text-v2-text-text-muted">
        <span class="flex size-4 shrink-0 items-center justify-center">
          <Icon name={props.icon} size="small" class="text-v2-icon-icon-faint" />
        </span>
        <span class="flex items-center gap-1.5">
          {props.title}
          <Show when={props.count}>
            <span class="tabular-nums text-v2-text-text-faint">{props.count}</span>
          </Show>
        </span>
        <Show when={props.action}>
          <span class="ms-auto flex items-center">{props.action}</span>
        </Show>
      </div>
      <div class="flex flex-col gap-px">{props.children}</div>
    </section>
  )
}

/** Nothing is waiting on you: the one state worth celebrating, so it takes Needs you's place instead of an empty list. */
function InboxZero() {
  const language = useLanguage()

  return (
    <div data-slot="start-inbox-zero" class="flex h-12 items-center gap-3 rounded-[8px] px-2">
      <span class="flex size-4 shrink-0 items-center justify-center motion-safe:animate-in motion-safe:zoom-in-50 motion-safe:fade-in motion-safe:duration-300">
        <Icon name="circle-check" class="text-v2-state-fg-success" />
      </span>
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="text-[13px] font-medium leading-4 text-v2-text-text-base">{language.t("start.inboxZero")}</span>
        <span class="text-[12px] leading-4 text-v2-text-text-faint">{language.t("start.inboxZero.description")}</span>
      </div>
    </div>
  )
}

function StartRow(props: { model: StartModel; entry: InboxEntry }) {
  const language = useLanguage()
  const inbox = props.model.inbox
  const loud = () => props.entry.status !== "ready"

  return (
    <div
      role="button"
      tabIndex={0}
      data-slot="start-row"
      data-status={props.entry.status}
      class="group flex h-12 cursor-default items-center gap-3 rounded-[8px] px-2 hover:bg-v2-overlay-simple-overlay-hover focus-visible:bg-v2-overlay-simple-overlay-hover focus-visible:outline-none"
      onClick={() => inbox.open(props.entry)}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return
        event.preventDefault()
        inbox.open(props.entry)
      }}
    >
      <SessionAvatar entry={props.entry} />
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span
          dir="auto"
          class="min-w-0 truncate text-[13px] leading-4"
          classList={{ "font-medium text-v2-text-text-base": loud(), "text-v2-text-text-muted": !loud() }}
        >
          {sessionTabTitle(props.entry.session.title, language.t("session.tab.session"))}
        </span>
        <Meta model={props.model} entry={props.entry} />
      </div>
      <div class="flex shrink-0 items-center gap-3 text-[12px] leading-4" onClick={(event) => event.stopPropagation()}>
        <RowActions model={props.model} entry={props.entry} />
        <span class="flex w-16 justify-end">
          <Status model={props.model} entry={props.entry} />
        </span>
      </div>
    </div>
  )
}

function RowActions(props: { model: StartModel; entry: InboxEntry }) {
  const language = useLanguage()
  const inbox = props.model.inbox

  return (
    <Switch>
      <Match when={props.entry.status === "approval"}>
        <div class="flex items-center gap-1">
          <Button size="small" variant="contrast" onClick={() => inbox.decide(props.entry, "once")}>
            {language.t("start.allow")}
          </Button>
          <Button size="small" variant="ghost-muted" onClick={() => inbox.decide(props.entry, "reject")}>
            {language.t("start.deny")}
          </Button>
        </div>
      </Match>
      <Match when={props.entry.status === "input"}>
        <Button size="small" variant="neutral" onClick={() => inbox.open(props.entry)}>
          {language.t("start.answer")}
        </Button>
      </Match>
      <Match when={props.entry.status === "done" || props.entry.status === "failed"}>
        <Tooltip value={language.t("start.markRead")} placement="top">
          <IconButton
            variant="ghost-muted"
            size="small"
            class="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
            aria-label={language.t("start.markRead")}
            icon={<Icon name="check" size="small" />}
            onClick={() => inbox.markViewed(props.entry)}
          />
        </Tooltip>
      </Match>
    </Switch>
  )
}

function Meta(props: { model: StartModel; entry: InboxEntry }) {
  const language = useLanguage()
  const servers = useServers()
  const inbox = props.model.inbox
  const project = () => props.entry.ctx.projects.forSession(props.entry.session)
  const branch = () => inbox.branch(props.entry)
  const permission = () => (props.entry.status === "approval" ? inbox.permission(props.entry) : undefined)
  const form = () => (props.entry.status === "input" ? inbox.form(props.entry) : undefined)

  return (
    <span class="flex h-4 min-w-0 items-center gap-2 text-[12px] leading-4 text-v2-text-text-faint">
      {/* Sessions on this computer are not marked. */}
      <Show when={servers.tag(props.entry.conn)}>
        {(name) => (
          <span class="flex min-w-0 shrink items-center gap-1">
            <Icon name="server" size="small" class="shrink-0" />
            <span class="min-w-0 truncate">{name()}</span>
          </span>
        )}
      </Show>
      <span class="shrink-0">{displayName(project() ?? { worktree: props.entry.session.location.directory })}</span>
      <Show when={branch()}>
        {(name) => (
          <span class="flex min-w-0 shrink items-center gap-1">
            <Icon name={inbox.inWorktree(props.entry) ? "outline-worktree" : "branch"} size="small" class="shrink-0" />
            <span dir="ltr" class="min-w-0 truncate">
              {name()}
            </span>
          </span>
        )}
      </Show>
      <Show when={permission()}>
        {(request) => (
          <span class="flex min-w-0 items-center gap-1.5">
            <Icon name="shield" size="small" class="shrink-0" />
            <span class="min-w-0 truncate font-mono">{request().resources.join(" ") || request().action}</span>
          </span>
        )}
      </Show>
      <Show when={form()}>
        {(request) => (
          <span class="flex min-w-0 items-center gap-1.5">
            <Icon name="speech-bubble" size="small" class="shrink-0" />
            <span class="min-w-0 truncate">{request().title || language.t("start.question")}</span>
          </span>
        )}
      </Show>
    </span>
  )
}

function Status(props: { model: StartModel; entry: InboxEntry }) {
  const language = useLanguage()
  const now = props.model.inbox.now
  const status = () => props.entry.status

  return (
    <Switch>
      <Match when={status() === "working"}>
        <span class="tabular-nums [&_[data-component=text-shimmer]]:[--text-shimmer-base-color:var(--v2-state-fg-info)]">
          <span class="sr-only">{language.t("sidebar.inbox.status.working")}</span>
          <span aria-hidden="true" class="text-v2-state-fg-info">
            <TextShimmer text={elapsed(language, now(), props.entry.session.time.updated)} active />
          </span>
        </span>
      </Match>
      <Match when={status() === "ready"}>
        <span class="text-v2-text-text-faint tabular-nums">
          {age(language, now(), props.entry.session.time.idle ?? props.entry.session.time.updated)}
        </span>
      </Match>
      <Match when={true}>
        <span
          classList={{
            "text-v2-state-fg-warning": status() === "approval" || status() === "input",
            "text-v2-state-fg-danger": status() === "failed",
            "text-v2-text-text-muted": status() === "done",
          }}
        >
          {
            // SAFETY: working and ready matched above, so the remaining statuses each have a label.
            language.t(`sidebar.inbox.status.${status() as "approval" | "input" | "failed" | "done"}`)
          }
        </span>
      </Match>
    </Switch>
  )
}

function SessionAvatar(props: { entry: InboxEntry }) {
  return (
    <span class="flex size-4 shrink-0 items-center justify-center">
      <SessionTabAvatarView
        project={props.entry.ctx.projects.forSession(props.entry.session)}
        directory={props.entry.session.location.directory}
        unread={props.entry.status !== "ready" && props.entry.status !== "working"}
        loading={props.entry.status === "working"}
      />
    </span>
  )
}

function compact(language: Language, seconds: number) {
  if (seconds < 3600) return language.plural("sidebar.inbox.elapsed.minutes", Math.floor(seconds / 60))

  if (seconds < 86_400) return language.plural("sidebar.inbox.elapsed.hours", Math.floor(seconds / 3600))

  return language.plural("sidebar.inbox.elapsed.days", Math.floor(seconds / 86_400))
}

function elapsed(language: Language, now: number, from: number) {
  const seconds = Math.max(0, Math.floor((now - from) / 1000))

  return seconds < 60 ? language.plural("sidebar.inbox.elapsed.seconds", seconds) : compact(language, seconds)
}

function age(language: Language, now: number, from: number) {
  const seconds = Math.max(0, Math.floor((now - from) / 1000))

  return seconds < 60 ? language.t("sidebar.inbox.time.now") : compact(language, seconds)
}
