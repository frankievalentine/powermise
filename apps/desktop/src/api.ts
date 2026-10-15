// Typed bridge to the Rust backend (src-tauri/src/lib.rs). Field names mirror
// the serialized domain types: catalog entries keep their snake_case TOML
// names; app-specific views are camelCase.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Platform = "darwin" | "linux" | "win32" | "wsl";

export const PLATFORMS: Platform[] = ["darwin", "linux", "win32", "wsl"];

export const PLATFORM_LABEL: Record<Platform, string> = {
  darwin: "macOS",
  linux: "Linux",
  win32: "Windows",
  wsl: "WSL",
};

export interface Harness {
  id: string;
  title: string;
  description: string;
  kind: string;
  category: "terminal" | "ide" | "gui" | string;
  extends: string | null;
  supported_platforms: Platform[];
  required_tool_ids: string[];
  desktop_tool_ids: string[];
  desktop_required: boolean;
  integration_ids: string[];
  mcp_server_ids: string[];
}

export type ToolProviderKind =
  | "mise-tool"
  | "bootstrap-apt"
  | "bootstrap-brew"
  | "bootstrap-brew-cask"
  | "winget"
  | "scoop";

export interface ToolProvider {
  kind: ToolProviderKind;
  package: string;
  version: string;
  command: string | null;
  supported_platforms: Platform[];
}

export interface Tool {
  id: string;
  title: string;
  description: string;
  providers: ToolProvider[];
  unsupported_reason: string | null;
}

export interface ConfigEntry {
  id: string;
  title: string;
  description: string;
  providers: { kind: "config-adapter"; config_target: string; supported_platforms: Platform[] }[];
  unsupported_reason: string | null;
}

export interface CatalogBundle {
  revision: string;
  harnesses: { harnesses: Harness[] };
  tools: { tools: Tool[] };
  apps: { apps: Tool[] };
  integrations: { integrations: ConfigEntry[] };
  mcp: { mcp_servers: ConfigEntry[] };
}

export interface EnvironmentInfo {
  hostTarget: Platform;
  powermiseDir: string;
  miseConfigDir: string;
  misePath: string | null;
}

// ---------------------------------------------------------------------------
// AI Setups (src-tauri/src/setups.rs)
// ---------------------------------------------------------------------------

export interface SavedSelection {
  name: string;
  path: string;
  harnesses: string[];
  includeDesktopApps: boolean;
  /** The mise fragment this setup writes: conf.d/powermise-<name>.toml. */
  fragmentPath: string;
  /** Whether that fragment exists, i.e. mise currently includes the setup. */
  applied: boolean;
}

export interface SelectionDraft {
  name: string;
  harnesses: string[];
  includeDesktopApps: boolean;
}

export interface FileChange {
  path: string;
  kind: "create" | "update" | "unchanged";
  diff: string;
}

/** How a resolved tool gets installed, as the compiler chose it. */
export interface Install {
  toolId: string;
  kind: ToolProviderKind;
  /** The [tools] or [bootstrap.packages] key, e.g. `node` or `brew:git`. */
  key: string;
  version: string;
}

export interface Preview {
  installs: Install[];
  resolved: {
    harnessIds: string[];
    toolIds: string[];
    desktopAppIds: string[];
    unavailableDesktopIds: string[];
    appIds: string[];
    integrationIds: string[];
    mcpServerIds: string[];
  };
  fragmentPath: string;
  fragmentContents: string;
  change: FileChange;
  applyCommand: string;
}

export interface WrittenSetup {
  path: string;
  change: FileChange;
}

/** `mise bootstrap plan --json`. */
export interface BootstrapPlan {
  resources: { id: { kind: string; name: string }; current: string; desired: string; action: string }[];
  summary: { create: number; update: number; remove: number; unchanged: number; unknown: number };
}

export interface SetupItemStatus {
  toolId: string;
  key: string;
  kind: ToolProviderKind;
  state: string;
  installed: boolean;
  /** Another config's version wins for this tool (usually your own). */
  overriddenBy: string | null;
}

export interface SetupStatus {
  name: string;
  fragmentPath: string;
  change: FileChange;
  items: SetupItemStatus[];
  applyCommand: string;
}

export interface RemovedSetup {
  removed: string[];
  cleanupCommand: string;
}

// ---------------------------------------------------------------------------
// mise (src-tauri/src/mise_commands.rs; shapes from powermise-mise models)
// ---------------------------------------------------------------------------

export interface MiseInfo {
  path: string | null;
  version: string | null;
  latest: string | null;
  minimum: string;
  recommended: string;
  /** New enough for powermise at all. */
  supported: boolean;
  /** Meets the recommended version, so every feature is available. */
  recommendedMet: boolean;
}

export interface ConfigSource {
  type: string;
  path: string;
}

export interface ToolVersion {
  version: string;
  requested_version?: string | null;
  install_path: string;
  source?: ConfigSource | null;
  installed: boolean;
  active: boolean;
}

export type ToolInventory = Record<string, ToolVersion[]>;

export interface OutdatedTool {
  name: string;
  requested: string | null;
  current: string | null;
  bump: string | null;
  latest: string;
  source: ConfigSource | null;
}

export type OutdatedReport = Record<string, OutdatedTool>;

export interface RegistryEntry {
  short: string;
  backends: string[];
  description: string | null;
  aliases: string[];
}

export interface RemoteVersion {
  version: string;
  created_at: string | null;
  release_url: string | null;
}

export interface Doctor {
  version: string;
  activated: boolean;
  shims_on_path: boolean;
  self_update_available: boolean;
  config_files: string[];
  errors: string[];
  warnings: string[];
  shell: { name: string; version: string | null } | null;
}

export type MiseAction =
  | { kind: "useGlobal"; tool: string; version: string }
  | { kind: "unuseGlobal"; tool: string }
  | { kind: "uninstall"; tool: string; version: string }
  | { kind: "upgrade"; tool: string | null }
  | { kind: "selfUpdate" }
  | { kind: "installProject"; project: string }
  | { kind: "trustProject"; project: string }
  | { kind: "runTask"; project: string; task: string; args: string[] }
  | { kind: "useInProject"; project: string; tool: string; version: string }
  | { kind: "setEnv"; project: string; key: string; value: string; file: string | null }
  | { kind: "unsetEnv"; project: string; key: string; file: string | null }
  | { kind: "bootstrapPackagesAndTools" }
  | { kind: "pruneTools" };

// ---------------------------------------------------------------------------
// Projects (src-tauri/src/projects.rs)
// ---------------------------------------------------------------------------

export interface ProjectEntry {
  path: string;
  name: string;
  exists: boolean;
  lastOpenedAt: number;
}

export interface ConfigFile {
  path: string;
  tools: string[];
}

export interface Task {
  name: string;
  description: string;
  aliases: string[];
  source: string;
  dir: string | null;
  hide: boolean;
  run: string[];
}

export interface EnvVarView {
  name: string;
  value: string;
  source: string | null;
  secret: boolean;
}

/** What an untrusted config would do once trusted, read without executing. */
export interface ConfigPreview {
  path: string;
  tools: [string, string][];
  envNames: string[];
  envDirectives: string[];
  hooks: [string, string][];
  tasks: [string, string][];
  parseError: string | null;
}

export interface ProjectOverview {
  path: string;
  name: string;
  trust: { path: string; trusted: boolean };
  previews: ConfigPreview[];
  tools: ToolInventory | null;
  configFiles: ConfigFile[] | null;
  tasks: Task[] | null;
  env: EnvVarView[] | null;
  problems: string[];
}

export interface TaskInput {
  name: string;
  usage: string;
  help: string | null;
  required: boolean;
  takesValue: boolean;
  choices: string[];
  default: string | null;
}

export interface TaskDetails {
  name: string;
  description: string;
  run: string[];
  depends: string[];
  args: TaskInput[];
  flags: TaskInput[];
}

export interface RunOutcome {
  success: boolean;
  code: number | null;
  cancelled: boolean;
}

export interface ActivityEntry {
  id: number;
  command: string;
  startedAt: number;
  finishedAt: number | null;
  outcome: RunOutcome | null;
  error: string | null;
}

export interface OutputEvent {
  jobId: number;
  stream: "stdout" | "stderr";
  text: string;
}

/** Whether the page runs inside the Tauri shell (vs. a plain browser preview). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) {
    const { browserPreview } = await import("./dev/browserPreview");
    return browserPreview<T>(command, args);
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(typeof error === "string" ? error : String(error));
  }
}

export const api = {
  environment: () => call<EnvironmentInfo>("get_environment"),
  catalog: () => call<CatalogBundle>("get_catalog"),
  selections: () => call<SavedSelection[]>("list_selections"),
  save: (draft: SelectionDraft, replace: boolean) =>
    call<string>("save_selection", { draft, replace }),
  preview: (draft: SelectionDraft, target: Platform) =>
    call<Preview>("preview_selection", { draft, target }),
  /** Writes the setup's fragment into mise's conf.d (installing is a separate run). */
  writeSetup: (name: string, target: Platform) => call<WrittenSetup>("write_setup", { name, target }),
  setupStatus: (name: string, target: Platform) => call<SetupStatus>("setup_status", { name, target }),
  bootstrapPlan: () => call<BootstrapPlan>("bootstrap_plan"),
  removeSetup: (name: string) => call<RemovedSetup>("remove_setup", { name }),
  reveal: (path: string) => call<void>("reveal_path", { path }),

  miseInfo: () => call<MiseInfo>("mise_info"),
  installed: () => call<ToolInventory>("installed_tools"),
  outdated: () => call<OutdatedReport>("outdated_tools"),
  registry: () => call<RegistryEntry[]>("registry"),
  remoteVersions: (tool: string) => call<RemoteVersion[]>("remote_versions", { tool }),
  doctor: () => call<Doctor>("doctor"),
  describe: (action: MiseAction) => call<string>("describe_action", { action }),
  run: (action: MiseAction) => call<number>("run_action", { action }),
  cancel: (jobId: number) => call<void>("cancel_job", { jobId }),
  activity: () => call<ActivityEntry[]>("activity"),

  projects: () => call<ProjectEntry[]>("list_projects"),
  /** Opens the system folder picker unless a path is given. `null` = cancelled. */
  addProject: (path?: string) => call<ProjectEntry | null>("add_project", { path: path ?? null }),
  forgetProject: (path: string) => call<void>("forget_project", { path }),
  projectOverview: (path: string) => call<ProjectOverview>("project_overview", { path }),
  taskDetails: (path: string, task: string) => call<TaskDetails>("task_details", { path, task }),
};

/** Subscribes to a backend event, in Tauri or from the browser stub. */
async function subscribe<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    const { browserEvents } = await import("./dev/browserPreview");
    return browserEvents.on(event, handler as (payload: unknown) => void);
  }
  return listen<T>(event, (message) => handler(message.payload));
}

/** Output lines from running mise jobs. */
export const onMiseOutput = (handler: (event: OutputEvent) => void) =>
  subscribe<OutputEvent>("mise-output", handler);

/** A mise job finished (successfully, with an error, or cancelled). */
export const onMiseFinished = (handler: (entry: ActivityEntry) => void) =>
  subscribe<ActivityEntry>("mise-finished", handler);

/** Matches the one naming rule powermise enforces for setups. */
export function isValidSetupName(name: string): boolean {
  return /^[a-z0-9]+(-[a-z0-9]+)*$/.test(name);
}
