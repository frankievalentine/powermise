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
  | { kind: "upgradeProject"; project: string; tool: string | null }
  | { kind: "selfUpdate" }
  | { kind: "installProject"; project: string }
  | { kind: "trustProject"; project: string }
  | { kind: "runTask"; project: string; task: string; args: string[] }
  | { kind: "useInProject"; project: string; tool: string; version: string }
  | { kind: "setEnv"; project: string; key: string; value: string; file: string | null }
  | { kind: "unsetEnv"; project: string; key: string; file: string | null }
  | { kind: "bootstrapPackagesAndTools" }
  | { kind: "pruneTools" }
  | { kind: "setSetting"; key: string; value: string }
  | { kind: "unsetSetting"; key: string }
  | { kind: "syncVersions"; source: ImportSource }
  | { kind: "installMise"; installer: MiseInstaller }
  | { kind: "untrust"; path: string }
  | { kind: "lock"; project: string }
  | {
      kind: "configSet";
      project: string;
      file: string;
      key: string;
      value: string;
      valueType: "string" | "integer" | "float" | "bool" | "list" | null;
    }
  | { kind: "generate"; project: string; generator: Generator; task: string | null }
  | { kind: "watch"; project: string; task: string }
  | { kind: "setVar"; project: string; name: string; value: string }
  | { kind: "unsetVar"; project: string; name: string }
  | { kind: "addSetting"; key: string; value: string }
  | { kind: "toolAutoUpdate"; tool: string; on: boolean }
  | { kind: "bootstrapPreview"; only: string | null }
  | { kind: "bootstrapApply"; only: string | null }
  | { kind: "dotfilesSave" }
  | { kind: "dotfilesSync" }
  | { kind: "dotfilesOriginSet"; url: string }
  | { kind: "dotfilesRollback"; checkpoint: number; dryRun: boolean }
  | { kind: "dotfilesUndo" }
  | { kind: "setToolAlias"; tool: string; alias: string; version: string }
  | { kind: "unsetToolAlias"; tool: string; alias: string }
  | { kind: "formatConfig"; project: string }
  | { kind: "cachePrune" }
  | { kind: "cacheClear" }
  | { kind: "reshim" }
  | { kind: "link"; tool: string; version: string; path: string };

// ---------------------------------------------------------------------------
// Machine (src-tauri/src/machine.rs)
// ---------------------------------------------------------------------------

/** One mise setting (`mise settings ls --all --json-extended`, flattened). */
export interface Setting {
  key: string;
  value: unknown;
  /** mise's type name: `boolean`, `number`, `string`, `array`, or a union. */
  type: string;
  description: string;
  /** The file (or variable) that sets it; null means the default. */
  source: string | null;
  deprecated: boolean;
}

export interface GithubStatus {
  /** `mise token github`, masked. */
  token: { host: string; token: string | null; source: string | null };
  ghSignedIn: boolean;
  credentialCommand: string;
  useGitCredentials: boolean;
}

export interface PrunableVersion {
  tool: string;
  version: string;
  installPath: string;
  bytes: number;
}

export type ImportSource = "nvm" | "nodenv" | "node-brew" | "pyenv" | "uv" | "ruby-brew";

export interface DetectedImport {
  source: ImportSource;
  tool: string;
  versions: string[];
}

/** `mise tool --json`. */
export interface ToolInfo {
  backend: string;
  description: string | null;
  url: string | null;
  installed_versions: string[];
  requested_versions: string[] | null;
  active_versions: string[] | null;
  config_source: ConfigSource | null;
  tool_options: Record<string, unknown>;
  /** `type` is `checksum`, `github_attestations`, `slsa`, …; the rest is detail. */
  security: ({ type: string } & Record<string, unknown>)[];
}

export interface ProjectUpdates {
  path: string;
  name: string;
  outdated: OutdatedReport;
}

export interface MachineStatus {
  sections: { id: string; items: { name: string; state: string | null; detail: string | null }[] }[];
  history: {
    enabled: boolean;
    trackedFiles: number;
    checkpoints: number;
    latest: unknown;
    unsaved: unknown;
    pendingOperations: number;
  } | null;
  origin: string | null;
  supportsHistory: boolean;
  checkpoints: {
    id: number;
    createdAt: string | null;
    trigger: string | null;
    summary: string | null;
    machine: string | null;
    changed: string[];
  }[];
}

export interface DiskUsage {
  cacheDir: string;
  cacheBytes: number;
  installsDir: string;
  installsBytes: number;
}

export interface TrustReport {
  trusted: { path: string; exists: boolean }[];
  ignored: string[];
}

export type MiseInstaller = "homebrew" | "winget" | "scoop" | "script";

export interface InstallerOption {
  id: MiseInstaller;
  label: string;
  command: string;
}

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
  /** Tasks that run first: names, `name arg…` strings, or arrays. */
  depends: (string | string[])[];
  /** Needs a real terminal. */
  interactive: boolean;
  /** Files whose changes `mise watch` reacts to. */
  sources: string[];
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
  /** Environments with config (`mise.<env>.toml`), for `MISE_ENV`. */
  environments: string[];
  /** The environment this overview previews, if any. */
  environment: string | null;
  idiomatic: { file: string; tool: string; version: string; enabled: boolean }[];
  lockfile: { path: string; changes: LockChange[] } | null;
  /** `[vars]` on mise 2026.10.6+, null on older mise. */
  vars: VarView[] | null;
  /** Secret names and their source (experimental in mise); never values. */
  secrets: { source: string | null; keys: string[] } | null;
  /** The workflow and pre-commit generators need a Git repository. */
  isGitRepo: boolean;
  problems: string[];
}

export interface VarView {
  name: string;
  /** null when a required var has no value yet. */
  value: string | null;
  source: string | null;
  /** Saved on this computer with `mise vars NAME=VALUE`. */
  saved: boolean;
  /** Declared with a prompt, default, or required — a saved value applies. */
  settable: boolean;
  prompt: string | null;
  required: boolean;
}

export interface LockChange {
  name: string;
  backend: string | null;
  lockfile: string | null;
  old_versions: string[];
  new_versions: string[];
}

export type Generator = "github-action" | "git-pre-commit" | "devcontainer";

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
  settings: () => call<Setting[]>("settings"),
  githubStatus: () => call<GithubStatus>("github_status"),
  prunableVersions: () => call<PrunableVersion[]>("prunable_versions"),
  detectImports: () => call<DetectedImport[]>("detect_imports"),
  miseInstallers: () => call<InstallerOption[]>("mise_installers"),
  openUrl: (url: string) => call<void>("open_url", { url }),
  diskUsage: () => call<DiskUsage>("disk_usage"),
  machineStatus: () => call<MachineStatus>("machine_status"),
  resolveDotfileConflicts: () => call<void>("resolve_dotfile_conflicts"),
  toolAliases: (tool: string) =>
    call<{ alias: string; version: string; custom: boolean }[]>("tool_aliases", { tool }),
  plugins: () => call<{ name: string; url: string | null; revision: string | null }[]>("plugins"),
  toolInfo: (tool: string) => call<ToolInfo>("tool_info", { tool }),
  backends: () => call<string[]>("backends"),
  projectUpdates: () => call<ProjectUpdates[]>("project_updates"),
  trustedFolders: () => call<TrustReport>("trusted_folders"),
  pickFolder: () => call<string | null>("pick_folder"),
  describe: (action: MiseAction) => call<string>("describe_action", { action }),
  run: (action: MiseAction) => call<number>("run_action", { action }),
  cancel: (jobId: number) => call<void>("cancel_job", { jobId }),
  activity: () => call<ActivityEntry[]>("activity"),

  projects: () => call<ProjectEntry[]>("list_projects"),
  /** Opens the system folder picker unless a path is given. `null` = cancelled. */
  addProject: (path?: string) => call<ProjectEntry | null>("add_project", { path: path ?? null }),
  forgetProject: (path: string) => call<void>("forget_project", { path }),
  projectOverview: (path: string, environment: string | null = null) =>
    call<ProjectOverview>("project_overview", { path, environment }),
  generatePreview: (path: string, generator: Generator, task: string | null) =>
    call<string>("generate_preview", { path, generator, task }),
  openTerminal: (path: string, task: string | null = null) => call<void>("open_terminal", { path, task }),
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
