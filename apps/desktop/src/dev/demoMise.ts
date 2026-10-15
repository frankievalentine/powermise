// A stateful stand-in for mise used when the UI runs outside Tauri: design
// work, and the live demo on the marketing site. Installs, upgrades, and
// removals change this in-memory state and stream believable output, so the
// demo behaves like the real app without touching anyone's machine.
import registryEntries from "./registry.json";
import {
  demoHasMise,
  demoMachine,
  describeMachineAction,
  NOT_A_MACHINE_COMMAND,
  performMachineAction,
} from "./demoMachine";
import {
  demoProjects,
  describeProjectAction,
  NOT_A_PROJECT_COMMAND,
  performProjectAction,
} from "./demoProjects";
import type {
  ActivityEntry,
  AgentCli,
  Doctor,
  MiseAction,
  MiseInfo,
  OutdatedReport,
  OutputEvent,
  RegistryEntry,
  RemoteVersion,
  ToolInventory,
  ToolVersion,
} from "../api";

type Listener = (payload: unknown) => void;

/** A tiny event bus standing in for Tauri events. */
export const browserEvents = {
  listeners: new Map<string, Set<Listener>>(),
  on(event: string, listener: Listener) {
    const set = this.listeners.get(event) ?? new Set<Listener>();
    set.add(listener);
    this.listeners.set(event, set);
    return () => set.delete(listener);
  },
  emit(event: string, payload: unknown) {
    this.listeners.get(event)?.forEach((listener) => listener(payload));
  },
};

const HOME = "/Users/demo";
const GLOBAL_CONFIG = `${HOME}/.config/mise/config.toml`;
const DATA = `${HOME}/.local/share/mise/installs`;

const registry: RegistryEntry[] = (registryEntries as Partial<RegistryEntry>[]).map((entry) => ({
  short: entry.short!,
  backends: entry.backends ?? [],
  description: entry.description ?? null,
  aliases: entry.aliases ?? [],
}));

/** Latest version per demo tool; anything else gets a generated history. */
const LATEST: Record<string, string> = {
  node: "24.11.0",
  python: "3.14.0",
  go: "1.25.3",
  jq: "1.8.1",
  ripgrep: "15.1.0",
  terraform: "1.13.4",
  bun: "1.3.14",
  uv: "0.9.5",
  rust: "1.91.0",
  deno: "2.5.6",
  gh: "2.82.1",
  kubectl: "1.34.1",
};

/** What the demo machine has, keyed by tool: [version, global?] pairs. */
const machine = new Map<string, { version: string; global: boolean }[]>([
  ["node", [{ version: "22.20.0", global: true }, { version: "20.19.5", global: false }]],
  ["python", [{ version: "3.13.9", global: true }]],
  ["go", [{ version: "1.25.3", global: true }]],
  ["jq", [{ version: "1.7.1", global: true }]],
  ["ripgrep", [{ version: "15.1.0", global: true }]],
  ["terraform", [{ version: "1.12.2", global: true }]],
  ["bun", [{ version: "1.3.14", global: true }]],
]);

const selfUpdateAvailable = true;
const autoUpdate = new Set<string>();
let miseVersion = "2026.7.18";
const activityLog: ActivityEntry[] = [];
let nextJob = 0;

function latestOf(tool: string): string {
  return LATEST[tool] ?? "1.4.2";
}

function inventory(): ToolInventory {
  const result: ToolInventory = {};
  for (const [tool, versions] of [...machine.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    result[tool] = versions.map(
      (entry): ToolVersion => ({
        version: entry.version,
        requested_version: entry.global ? entry.version : null,
        install_path: `${DATA}/${tool}/${entry.version}`,
        source: entry.global ? { type: "mise.toml", path: GLOBAL_CONFIG } : null,
        installed: true,
        active: entry.global,
      }),
    );
  }
  return result;
}

function outdated(): OutdatedReport {
  const report: OutdatedReport = {};
  for (const [tool, versions] of machine) {
    const global = versions.find((entry) => entry.global);
    const latest = latestOf(tool);
    if (global && global.version !== latest) {
      report[tool] = {
        name: tool,
        requested: global.version,
        current: global.version,
        bump: latest,
        latest,
        source: { type: "mise.toml", path: GLOBAL_CONFIG },
      };
    }
  }
  return report;
}

function remoteVersions(tool: string): RemoteVersion[] {
  const [major, minor, patch] = latestOf(tool).split(".").map(Number);
  const versions: string[] = [];
  for (let m = Math.max(0, minor - 3); m <= minor; m++) {
    const top = m === minor ? patch : 4;
    for (let p = 0; p <= top; p++) versions.push(`${major}.${m}.${p}`);
  }
  if (major > 1) versions.unshift(`${major - 1}.9.0`, `${major - 1}.10.1`);
  return versions.map((version) => ({ version, created_at: null, release_url: null }));
}

function doctor(): Doctor {
  return {
    version: `${miseVersion} macos-arm64`,
    activated: true,
    shims_on_path: true,
    self_update_available: selfUpdateAvailable,
    config_files: [GLOBAL_CONFIG, `${HOME}/code/web-app/mise.toml`],
    errors: [],
    warnings: [],
    shell: { name: "/bin/zsh", version: "zsh 5.9" },
  };
}

function describe(action: MiseAction): string {
  const project = describeProjectAction(action);
  if (project) return project;
  const machineAction = describeMachineAction(action);
  if (machineAction) return machineAction;
  switch (action.kind) {
    case "bootstrapPackagesAndTools":
      return "mise bootstrap --yes --only packages,tools";
    case "agentPlugin":
      return agentPluginCommand(action.agent, action.id, action.remove);
    case "skillsAdd":
      return `${action.telemetry ? "" : "DISABLE_TELEMETRY=1 "}mise x npm:skills@1.7.2 -- skills add ${action.source} -g -y ${action.agents
        .map((agent) => `-a ${agent}`)
        .join(" ")} ${action.skills.length ? action.skills.map((skill) => `--skill ${skill}`).join(" ") : "--skill '*'"}`;
    case "skillsRemove":
      return `${action.telemetry ? "" : "DISABLE_TELEMETRY=1 "}mise x npm:skills@1.7.2 -- skills remove ${action.name} -g -y`;
    case "skillsUpdate":
      return `${action.telemetry ? "" : "DISABLE_TELEMETRY=1 "}mise x npm:skills@1.7.2 -- skills update -g -y`;
    case "skillsSyncTools":
      return "mise skills sync --global";
    case "applySetup":
      return `mise bootstrap --yes --only packages,tools && mise dotfiles apply --yes ${applySetupTargets(action.name).join(" ")}`.trim();
    case "pruneTools":
      return "mise prune --tools --yes";
    case "useGlobal":
      return `mise use -g ${action.tool}@${action.version}`;
    case "unuseGlobal":
      return `mise unuse -g ${action.tool}`;
    case "uninstall":
      return `mise uninstall ${action.tool}@${action.version}`;
    case "upgrade":
      return action.tool ? `mise upgrade --global --bump ${action.tool}` : "mise upgrade --global --bump";
    case "upgradeProject":
      return `cd ${action.project.replace("/Users/demo", "~")} && mise upgrade --local --bump${action.tool ? ` ${action.tool}` : ""}`;
    case "selfUpdate":
      return "mise self-update --yes";
    case "updateMiseWithManager":
      return "brew upgrade mise";
    case "toolAutoUpdate":
      return `mise config set --global tools.${action.tool}.auto_update ${action.on}`;
    default:
      return "mise";
  }
}

function installLines(tool: string, version: string): string[] {
  const backend = registry.find((entry) => entry.short === tool)?.backends[0] ?? `core:${tool}`;
  return [
    `mise ${tool}@${version}  resolving ${backend}`,
    `mise ${tool}@${version}  download ${tool}-${version}-darwin-arm64.tar.gz`,
    `mise ${tool}@${version}  checksum verified`,
    `mise ${tool}@${version}  extract`,
    `mise ${tool}@${version}  ✓ installed`,
  ];
}

/** Applies an action to the demo machine and returns the output it "prints". */
function perform(action: MiseAction): string[] {
  const project = performProjectAction(action);
  if (project) return project;
  const machineAction = performMachineAction(action);
  if (machineAction) return machineAction;
  switch (action.kind) {
    case "bootstrapPackagesAndTools":
      return bootstrapHook();
    case "applySetup":
      return [...bootstrapHook(), ...applySetupHook(action.name)];
    case "agentPlugin":
      return pluginHook(action.agent, action.id, action.remove);
    case "skillsAdd":
    case "skillsRemove":
    case "skillsUpdate":
    case "skillsSyncTools":
      return skillsHook(action);
    case "pruneTools": {
      const unused = [...machine.entries()].flatMap(([tool, versions]) =>
        versions.filter((entry) => !entry.global).map((entry) => `${tool}@${entry.version}`),
      );
      for (const [tool, versions] of machine) machine.set(tool, versions.filter((entry) => entry.global));
      return unused.length ? unused.map((spec) => `mise ${spec}  ✓ pruned`) : ["mise nothing to prune"];
    }
    case "useGlobal": {
      const versions = (machine.get(action.tool) ?? []).map((entry) => ({ ...entry, global: false }));
      const existing = versions.find((entry) => entry.version === action.version);
      if (existing) existing.global = true;
      else versions.push({ version: action.version, global: true });
      machine.set(action.tool, versions);
      return [
        ...(existing ? [] : installLines(action.tool, action.version)),
        `mise ~/.config/mise/config.toml tools: ${action.tool}@${action.version}`,
      ];
    }
    case "unuseGlobal": {
      machine.delete(action.tool);
      return [`mise ~/.config/mise/config.toml removed: ${action.tool}`, `mise pruned ${action.tool}`];
    }
    case "uninstall": {
      const remaining = (machine.get(action.tool) ?? []).filter((entry) => entry.version !== action.version);
      if (remaining.length) machine.set(action.tool, remaining);
      else machine.delete(action.tool);
      return [`mise ${action.tool}@${action.version}  ✓ uninstalled`];
    }
    case "upgrade": {
      const report = outdated();
      const tools = action.tool ? [action.tool] : Object.keys(report);
      const lines: string[] = [];
      for (const tool of tools) {
        const target = report[tool]?.bump ?? report[tool]?.latest;
        if (!target) continue;
        lines.push(...installLines(tool, target));
        const versions = machine.get(tool) ?? [];
        machine.set(tool, [
          ...versions.filter((entry) => !entry.global),
          { version: target, global: true },
        ]);
        lines.push(`mise ~/.config/mise/config.toml tools: ${tool}@${target}`);
      }
      return lines.length ? lines : ["mise All tools are up to date"];
    }
    case "toolAutoUpdate":
      if (action.on) autoUpdate.add(action.tool);
      else autoUpdate.delete(action.tool);
      return [];
    case "selfUpdate":
      miseVersion = "2026.10.10";
      return ["Checking target-arch... mise-v2026.10.10-macos-arm64", "Downloading...", "Updated mise to 2026.10.10"];
    default:
      return [];
  }
}

function run(action: MiseAction): number {
  const id = ++nextJob;
  const entry: ActivityEntry = {
    id,
    command: describe(action),
    startedAt: Date.now(),
    finishedAt: null,
    outcome: null,
    error: null,
  };
  activityLog.push(entry);
  const lines = perform(action);
  lines.forEach((text, index) => {
    setTimeout(
      () => browserEvents.emit("mise-output", { jobId: id, stream: "stderr", text } satisfies OutputEvent),
      260 + index * 320,
    );
  });
  setTimeout(() => {
    entry.finishedAt = Date.now();
    entry.outcome = { success: true, code: 0, cancelled: false };
    browserEvents.emit("mise-finished", { ...entry });
  }, 420 + lines.length * 320);
  return id;
}

/** Handles mise commands for the browser stub; `undefined` means "not mine". */
export function demoMise(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case "mise_info":
      if (!demoHasMise) {
        return {
          path: null,
          version: null,
          latest: null,
          minimum: "2026.7.18",
          recommended: "2026.10.0",
          supported: false,
          recommendedMet: false,
          updateAvailable: false,
          updater: null,
        } satisfies MiseInfo;
      }
      return {
        path: "/Users/demo/.local/bin/mise",
        version: `${miseVersion} macos-arm64 (demo)`,
        latest: "2026.10.10",
        minimum: "2026.7.18",
        recommended: "2026.10.0",
        supported: true,
        recommendedMet: miseVersion.startsWith("2026.10"),
        updateAvailable: miseVersion !== "2026.10.10",
        updater: { kind: "unknown" },
      } satisfies MiseInfo;
    case "installed_tools":
      return demoHasMise ? inventory() : {};
    case "outdated_tools":
      return demoHasMise ? outdated() : {};
    case "registry":
      return registry;
    case "remote_versions":
      return remoteVersions(args.tool as string);
    case "doctor":
      return doctor();
    case "describe_action":
      return describe(args.action as MiseAction);
    case "run_action":
      return run(args.action as MiseAction);
    case "cancel_job":
      return undefined;
    case "activity":
      return [...activityLog].reverse();
    case "tool_info": {
      const tool = args.tool as string;
      const entry = registry.find((candidate) => candidate.short === tool);
      const versions = machine.get(tool) ?? [];
      return {
        backend: entry?.backends[0] ?? tool,
        description: entry?.description ?? null,
        url: tool.includes(":") ? null : `https://github.com/search?q=${tool}`,
        installed_versions: versions.map((version) => version.version),
        requested_versions: versions.filter((version) => version.global).map((version) => version.version),
        active_versions: versions.filter((version) => version.global).map((version) => version.version),
        config_source: versions.some((version) => version.global) ? { type: "mise.toml", path: GLOBAL_CONFIG } : null,
        tool_options: { os: null, depends: null, install_env: {}, auto_update: autoUpdate.has(tool) ? true : null },
        security: tool.startsWith("npm:")
          ? []
          : [
              { type: "checksum", algorithm: "sha256" },
              { type: "github_attestations", signer_workflow: `${tool}/.github/workflows/release.yml` },
            ],
      };
    }
    case "backends":
      return ["aqua", "asdf", "cargo", "conda", "core", "dotnet", "gem", "github", "gitlab", "go", "npm", "pypi", "spm", "http", "ubi", "vfox"];
    case "project_updates":
      return [
        {
          path: `${HOME}/code/web-app`,
          name: "web-app",
          outdated: {
            node: {
              name: "node",
              requested: "20",
              current: "20.19.5",
              bump: "24",
              latest: "24.11.0",
              release_url: null,
              source: { type: "mise.toml", path: `${HOME}/code/web-app/mise.toml` },
            },
          },
        },
      ];
    case "prunable_versions":
      return [...machine.entries()].flatMap(([tool, versions]) =>
        versions
          .filter((entry) => !entry.global)
          .map((entry) => ({
            tool,
            version: entry.version,
            installPath: `${DATA}/${tool}/${entry.version}`,
            bytes: 187_400_000,
          })),
      );
    default: {
      const machineResult = demoMachine(command, args);
      if (machineResult !== NOT_A_MACHINE_COMMAND) return machineResult;
      const result = demoProjects(command, args);
      return result === NOT_A_PROJECT_COMMAND ? NOT_HANDLED : result;
    }
  }
}

export const NOT_HANDLED = Symbol("not handled");

/** Set by the browser stub so a demo bootstrap installs the applied setups. */
let bootstrapHook: () => string[] = () => ["mise all tools are installed"];
/** Runs a demo skills command; returns the output lines. */
let skillsHook: (action: MiseAction) => string[] = () => [];
export function setSkillsHook(hook: typeof skillsHook) {
  skillsHook = hook;
}

/** Installs or removes a demo agent plugin; returns the output lines. */
let pluginHook: (agent: AgentCli, id: string, remove: boolean) => string[] = () => [];
export function setPluginHook(hook: typeof pluginHook) {
  pluginHook = hook;
}

function agentPluginCommand(agent: AgentCli, id: string, remove: boolean): string {
  if (agent === "claude") return `mise x -- claude plugin ${remove ? "uninstall" : "install"} ${id} --scope user`;
  if (agent === "codex") return `mise x -- codex plugin ${remove ? "remove" : "add"} ${id}`;
  return remove ? `mise x -- gemini extensions uninstall ${id}` : `mise x -- gemini extensions install ${id} --consent`;
}

/** Applies a setup's MCP servers in the demo; returns the output lines. */
let applySetupHook: (name: string) => string[] = () => [];
/** The agent config files a demo setup merges MCP servers into. */
let applySetupTargets: (name: string) => string[] = () => [];
export function setBootstrapHook(
  hook: () => string[],
  apply?: (name: string) => string[],
  targets?: (name: string) => string[],
) {
  bootstrapHook = hook;
  if (apply) applySetupHook = apply;
  if (targets) applySetupTargets = targets;
}
