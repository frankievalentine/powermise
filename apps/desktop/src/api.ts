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
  miseVersion: string | null;
}

export type OperationOutcome = "succeeded" | "failed" | "skipped";

export interface OperationResult {
  operation_id: string;
  outcome: OperationOutcome;
  message?: string;
}

export interface RunReport {
  selection: string;
  platform: Platform;
  operations: OperationResult[];
}

export interface SavedSelection {
  name: string;
  path: string;
  harnesses: string[];
  includeDesktopApps: boolean;
  applied: boolean;
  lastRun: RunReport | null;
}

export interface SelectionDraft {
  name: string;
  harnesses: string[];
  includeDesktopApps: boolean;
}

export type OperationProvider =
  | "mise-tools"
  | "mise-bootstrap"
  | "winget"
  | "scoop"
  | "npm"
  | "verified-script"
  | "config-adapter"
  | "generated";

export interface Operation {
  id: string;
  provider: OperationProvider;
  resource_kind: string;
  resource_id: string;
  requested_state: string;
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
  package: string;
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
  operations: Operation[];
  configPath: string;
  configContents: string;
  change: FileChange;
  applyCommand: string;
}

export type BootstrapView =
  | { state: "converged" }
  | { state: "failed"; message: string }
  | { state: "skipped"; reason: string };

export interface ApplyResult {
  selection: string;
  results: OperationResult[];
  bootstrap: BootstrapView;
  followUps: string[];
  recoveryGuidance: string[];
  command: string;
}

export interface StatusResult {
  selection: string;
  satisfied: boolean;
  change: FileChange;
  drift: string | null;
  ledgerOwned: boolean;
  followUps: string[];
  native: { operationId: string; package: string; present: boolean; detail: string }[];
  command: string;
}

export interface RemoveResult {
  removed: string[];
  commands: string[];
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
  /** New enough for Powermise at all. */
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
  | { kind: "selfUpdate" };

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
  apply: (name: string, target: Platform) => call<ApplyResult>("apply_selection", { name, target }),
  status: (name: string, target: Platform) =>
    call<StatusResult>("selection_status", { name, target }),
  remove: (name: string) => call<RemoveResult>("remove_selection", { name }),
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

/** Subscribes to human-readable apply progress lines. */
export async function onApplyProgress(handler: (step: string) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<string>("apply-progress", (event) => handler(event.payload));
}

/** Matches the one naming rule Powermise enforces for setups. */
export function isValidSetupName(name: string): boolean {
  return /^[a-z0-9]+(-[a-z0-9]+)*$/.test(name);
}
