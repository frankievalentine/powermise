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
  /** Plugins a new setup with this agent starts with. */
  integration_ids: string[];
  /** The CLI that installs this agent's plugins. */
  plugin_agent: AgentCli | null;
  /** MCP servers a new setup with this agent starts with. */
  mcp_server_ids: string[];
  /** Where the agent reads MCP servers, when powermise can configure it. */
  mcp_client: McpClientId | null;
  /** The desktop app's MCP config, used when the desktop apps are in. */
  desktop_mcp_client: McpClientId | null;
}

export type McpClientId =
  | "claude-code"
  | "claude-desktop"
  | "codex"
  | "gemini-cli"
  | "opencode"
  | "cursor"
  | "vscode"
  | "copilot-cli"
  | "cline"
  | "antigravity"
  | "zed";

/** An MCP server: a program mise installs and runs, or a remote `url`. */
export interface McpServer {
  id: string;
  title: string;
  description: string;
  homepage: string | null;
  tool: string | null;
  version: string | null;
  command: string | null;
  args: string[];
  url: string | null;
}

export type ToolProviderKind =
  | "mise-tool"
  /** Comes with the operating system: nothing to install. */
  | "system"
  | "bootstrap-apt"
  | "bootstrap-brew"
  | "bootstrap-brew-cask"
  /** A Mac app from the vendor's checksummed download. */
  | "macos-app"
  | "winget"
  | "scoop";

export interface ToolProvider {
  kind: ToolProviderKind;
  package: string;
  version: string;
  /** `macos-app` downloads are per Mac architecture. */
  arch?: "arm64" | "x64";
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

export type AgentCli = "claude" | "codex" | "gemini";

/** A plugin or extension a setup can include, installed with the agent's CLI. */
export interface Integration {
  id: string;
  title: string;
  description: string;
  agent: AgentCli;
  /** A plugin id (`github@anthropic-plugin-directory`) or, for Gemini, a source URL. */
  plugin: string;
  /** Gemini: the extension's name once installed. */
  installed_name: string | null;
  homepage: string | null;
}

/** A plugin as an agent's CLI reports it. */
export interface AgentPlugin {
  id: string;
  name: string;
  description: string | null;
  marketplace: string | null;
  version: string | null;
  installed: boolean;
  enabled: boolean;
  /** Signs in to a service as it installs (Codex), so it installs in the terminal. */
  signsInOnInstall: boolean;
}

export interface PluginReport {
  installed: AgentPlugin[];
  available: AgentPlugin[];
  error: string | null;
}

/** A skill installed for one or more agents (`~/.claude/skills`, …). */
export interface InstalledSkill {
  name: string;
  description: string | null;
  /** `owner/repo`, when the skills CLI installed it. */
  source: string | null;
  /** The skills CLI's agent names (`claude-code`, `codex`, …). */
  agents: string[];
}

export interface SkillsReport {
  installed: InstalledSkill[];
  agents: { id: string; title: string; detected: boolean }[];
  /** Skills that tools mise manages ship (`mise skills ls`). */
  toolSkills: { name: string; tool: string; version: string }[];
  /** The skills CLI mise runs, like `npm:skills@1.7.2`. */
  cli: string;
}

/** A plugin a setup installs. */
export interface SetupPlugin {
  id: string;
  title: string;
  agent: AgentCli;
  command: string;
  /** Codex plugins sign in while installing, so they install in the terminal. */
  inTerminal: boolean;
  /** Status only: whether the agent reports it installed (`undefined` when it can't be asked). */
  installed?: boolean;
}

export interface CatalogBundle {
  revision: string;
  harnesses: { harnesses: Harness[] };
  tools: { tools: Tool[] };
  apps: { apps: Tool[] };
  integrations: { integrations: Integration[] };
  mcp: { mcp_servers: McpServer[] };
}

export interface EnvironmentInfo {
  hostTarget: Platform;
  /** The home directory, for showing paths as `~/…`. */
  home: string | null;
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
  /** The MCP servers picked, or `null` for the agents' recommendations. */
  mcpServers: string[] | null;
  /** The plugins picked, or `null` for the agents' recommendations. */
  integrations: string[] | null;
  includeDesktopApps: boolean;
  /** The mise fragment this setup writes: conf.d/powermise-<name>.toml. */
  fragmentPath: string;
  /** Whether that fragment exists, i.e. mise currently includes the setup. */
  applied: boolean;
}

export interface SelectionDraft {
  name: string;
  harnesses: string[];
  /** `null` keeps the agents' recommended MCP servers. */
  mcpServers: string[] | null;
  /** `null` keeps the agents' recommended plugins. */
  integrations: string[] | null;
  includeDesktopApps: boolean;
}

/** A setup file read for the editor to review (from Export on another computer). */
export interface SetupFile {
  path: string;
  draft: SelectionDraft;
  /** Agents, MCP servers, and plugins this powermise doesn't have, left out. */
  unknown: string[];
}

/** An agent config file a setup adds MCP servers to. */
export interface McpTarget {
  client: McpClientId;
  title: string;
  /** From home, like `~/.claude.json`. */
  target: string;
  servers: string[];
  /** From `mise dotfiles status` (setup status only): `applied`, `differs`, `not applied`. */
  state?: string;
}

export interface RemovedServer {
  client: McpClientId;
  server: string;
  path: string;
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
  /** Tools left out on this machine, with why (Homebrew formulas on an Intel Mac). */
  skipped: { id: string; reason: string }[];
  /** On Linux/WSL: Homebrew's folder, when it needs a one-time `sudo` to create. */
  homebrewSetup: string | null;
  /** The agents' config files the setup adds MCP servers to. */
  mcp: McpTarget[];
  plugins: SetupPlugin[];
}

export interface WrittenSetup {
  path: string;
  change: FileChange;
  /** MCP servers the setup dropped, taken out of agents' configs. */
  removedServers: RemovedServer[];
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
  mcp: McpTarget[];
  plugins: SetupPlugin[];
}

export interface RemovedSetup {
  removed: string[];
  removedServers: RemovedServer[];
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
  /** `latest` is newer than the installed version. */
  updateAvailable: boolean;
  /** Who updates mise when `mise self-update` is turned off for this install. */
  updater: MiseUpdater | null;
}

/** How a packaged mise is updated (src/updater.rs in powermise-mise). */
export type MiseUpdater =
  | { kind: "homebrew" }
  | { kind: "winget" }
  | { kind: "scoop" }
  | { kind: "instructions"; message: string }
  | { kind: "unknown" };

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
  /** The commands the tool puts on PATH. */
  bins?: string[];
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
  /** Whether `mise self-update` works for this install (packagers can turn
   *  it off) — not whether a newer mise is out. */
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
  | { kind: "updateMiseWithManager" }
  | { kind: "installProject"; project: string }
  | { kind: "trustProject"; project: string }
  | { kind: "runTask"; project: string; task: string; args: string[] }
  | { kind: "useInProject"; project: string; tool: string; version: string }
  | { kind: "setEnv"; project: string; key: string; value: string; file: string | null }
  | { kind: "unsetEnv"; project: string; key: string; file: string | null }
  | { kind: "bootstrapPackagesAndTools" }
  /** Packages and tools, then the setup's MCP servers into agents' configs. */
  | { kind: "applySetup"; name: string }
  | { kind: "agentPlugin"; agent: AgentCli; id: string; remove: boolean }
  | { kind: "skillsAdd"; source: string; skills: string[]; agents: string[]; telemetry: boolean }
  | { kind: "skillsRemove"; name: string; telemetry: boolean }
  | { kind: "skillsUpdate"; telemetry: boolean }
  | { kind: "skillsSyncTools" }
  | { kind: "pruneTools" }
  | { kind: "setSetting"; key: string; value: string }
  | { kind: "unsetSetting"; key: string }
  | { kind: "importVersions"; source: ImportSource; tool: string; makeDefault: boolean }
  | { kind: "useGlobalMany"; tools: [string, string][] }
  /** `[tool_alias] <tool> = "core:<tool>"`: mise's built-in installer over a same-named plugin. */
  | { kind: "useCoreInstaller"; tool: string }
  | { kind: "importDryRun"; tools: [string, string][]; packages: string[] }
  | {
      kind: "applyImport";
      versions: [ImportSource, string, boolean][];
      tools: [string, string][];
      packages: string[];
    }
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
  | { kind: "relaunchApp"; app: string }
  | { kind: "usePackage"; spec: string }
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

export type ImportSource =
  | "nvm"
  | "nodenv"
  | "node-brew"
  | "pyenv"
  | "uv"
  | "ruby-brew"
  | "fnm"
  | "volta"
  | "n"
  | "nvm-windows"
  | "rbenv"
  | "rvm"
  | "chruby"
  | "goenv"
  | "asdf";

export interface ImportVersion {
  version: string;
  path: string;
  /** mise already has this version. */
  inMise: boolean;
}

/** One tool's installs from another version manager. */
export interface DetectedImport {
  source: ImportSource;
  tool: string;
  versions: ImportVersion[];
  /** The version that manager had as your default. */
  defaultVersion: string | null;
}

/** A startup-file line that still turns on another version manager. */
export interface ShellConflict {
  file: string;
  line: number;
  text: string;
  manager: string;
}

/** A program in an installer's folder (Homebrew, an install script, `go install`). */
export interface InstalledProgram {
  name: string;
  path: string;
  foundIn: string;
  /** The Homebrew formula or cask it belongs to. */
  package: string | null;
  /** The command that removes the old copy (`brew uninstall flyctl`). Shown, never run. */
  removeCommand: string | null;
}

export interface BrewItem {
  /** Tap-qualified for other taps (`mongodb/brew/mongodb-community`). */
  name: string;
  version: string | null;
}

export interface Brewfile {
  path: string;
  taps: string[];
  formulas: string[];
  casks: string[];
  /** `[name, App Store id]`. */
  mas: [string, string][];
  skipped: string[];
}

export interface MasApp {
  id: string;
  name: string;
  version: string | null;
}

export interface DefaultPackage {
  backend: "npm" | "pipx" | "gem";
  name: string;
  version: string | null;
  file: string;
}

/** A Windows package from winget, Scoop, or Chocolatey. */
export interface WindowsPackage {
  manager: "winget" | "scoop" | "choco";
  /** The winget id, the Scoop app (`extras/vscode` outside `main`), or the Chocolatey id. */
  name: string;
  version: string | null;
  foundIn: string;
}

export interface ImportScan {
  homebrew: { formulas: BrewItem[]; casks: BrewItem[] };
  brewfiles: Brewfile[];
  masApps: MasApp[];
  defaultPackages: DefaultPackage[];
  /** Packages already in powermise's import file. */
  importedPackages: string[];
  /** mise installs Homebrew formulas here (Apple Silicon Macs and Linux). */
  homebrewFormulas: boolean;
  /** winget, Scoop, and Chocolatey packages (Windows only). */
  windowsPackages: WindowsPackage[];
}

export interface ImportFile {
  path: string;
  contents: string;
  change: FileChange;
}

/** A command-line package installed globally outside mise. */
export interface GlobalPackage {
  backend: "npm" | "pipx" | "cargo" | "gem";
  name: string;
  version: string | null;
  foundIn: string;
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

export interface MacosState {
  /** What powermise's file asks mise to set. */
  managed: { domain: string; key: string; value: boolean | number | string }[];
  /** `defaults read` output per requested key; null when macOS's default applies. */
  current: (string | null)[];
  fragmentPath: string;
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
  idiomatic: {
    file: string;
    tool: string;
    version: string;
    enabled: boolean;
    /** Turned off on its own in `idiomatic_version_file_disable_files`. */
    disabledFile: boolean;
    disabledFiles: string[];
  }[];
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
  bootstrapInTerminal: () => call<void>("bootstrap_in_terminal"),
  removeSetup: (name: string) => call<RemovedSetup>("remove_setup", { name }),
  skillsReport: () => call<SkillsReport>("skills_report"),
  agentPlugins: (agent: AgentCli) => call<PluginReport>("agent_plugins", { agent }),
  installPluginInTerminal: (agent: AgentCli, id: string) => call<void>("install_plugin_in_terminal", { agent, id }),
  reveal: (path: string) => call<void>("reveal_path", { path }),
  exportSetup: (name: string) => call<string | null>("export_setup", { name }),
  readSetupFile: () => call<SetupFile | null>("read_setup_file"),

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
  shellConflicts: () => call<ShellConflict[]>("shell_conflicts"),
  globalPackages: () => call<GlobalPackage[]>("global_packages"),
  installedPrograms: () => call<InstalledProgram[]>("installed_programs"),
  importScan: () => call<ImportScan>("import_scan"),
  readBrewfile: () => call<Brewfile | null>("read_brewfile"),
  readPackageList: () => call<WindowsPackage[] | null>("read_package_list"),
  importFilePreview: (tools: [string, string][], packages: string[]) =>
    call<ImportFile>("import_file_preview", { tools, packages }),
  writeImport: (tools: [string, string][], packages: string[]) => call<string>("write_import", { tools, packages }),
  miseInstallers: () => call<InstallerOption[]>("mise_installers"),
  openUrl: (url: string) => call<void>("open_url", { url }),
  diskUsage: () => call<DiskUsage>("disk_usage"),
  /** Declared system packages and their state, keyed `manager:package`. */
  packageStates: () => call<Record<string, string>>("package_states"),
  usePackageInTerminal: (spec: string) => call<void>("use_package_in_terminal", { spec }),
  macosSettings: (keys: { domain: string; key: string }[]) => call<MacosState>("macos_settings", { keys }),
  setMacosDefault: (domain: string, key: string, value: boolean | number | string | null) =>
    call<MacosState["managed"]>("set_macos_default", { domain, key, value }),
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

export interface QuietResult {
  /** The exact command line that ran. */
  command: string;
  ok: boolean;
  /** Output lines, for showing why it failed. */
  output: string[];
}

/** Runs an action without the run sheet and resolves when it finishes. For
 *  quick, easily undone changes (like a setting) whose command is shown
 *  inline instead; it still lands in the activity log. */
export async function runQuietly(action: MiseAction): Promise<QuietResult> {
  const output: string[] = [];
  let jobId: number | null = null;
  const early: OutputEvent[] = [];
  const finishedEarly: ActivityEntry[] = [];
  let resolve!: (entry: ActivityEntry) => void;
  const finished = new Promise<ActivityEntry>((done) => (resolve = done));
  // Subscribe first: a fast job can finish before `run` returns its id.
  const stopOutput = await onMiseOutput((event) => {
    if (jobId === null) early.push(event);
    else if (event.jobId === jobId) output.push(event.text);
  });
  const stopFinished = await onMiseFinished((entry) => {
    if (jobId === null) finishedEarly.push(entry);
    else if (entry.id === jobId) resolve(entry);
  });
  try {
    const id = await api.run(action);
    jobId = id;
    output.push(...early.filter((event) => event.jobId === id).map((event) => event.text));
    const already = finishedEarly.find((entry) => entry.id === id);
    if (already) resolve(already);
    const entry = await finished;
    return {
      command: entry.command,
      ok: !!entry.outcome?.success && !entry.error,
      output: entry.error ? [...output, entry.error] : output,
    };
  } finally {
    stopOutput();
    stopFinished();
  }
}
