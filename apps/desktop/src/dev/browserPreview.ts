// Browser-only stand-in for the Rust backend, used when the UI runs in a plain
// browser (`bun run dev` without Tauri) for design work and the marketing-site
// demo. AI Setups read the real catalog TOML files and mirror the compiler's
// rules; mise is simulated by `demoMise.ts`. Nothing here touches the machine.
// Never loaded inside Tauri.
import { parse } from "smol-toml";
import harnessesToml from "../../../../catalog/profiles/harnesses.toml?raw";
import toolsToml from "../../../../catalog/profiles/tools.toml?raw";
import appsToml from "../../../../catalog/profiles/apps.toml?raw";
import integrationsToml from "../../../../catalog/profiles/integrations.toml?raw";
import mcpToml from "../../../../catalog/profiles/mcp.toml?raw";
import { MCP_CLIENT_TITLE, MCP_CLIENT_UNSUPPORTED } from "../catalog";
import { demoMise, NOT_HANDLED, setBootstrapHook } from "./demoMise";
import { demoHasMise } from "./demoMachine";
export { browserEvents } from "./demoMise";
import type {
  McpClientId,
  McpTarget,
  BootstrapPlan,
  CatalogBundle,
  EnvironmentInfo,
  FileChange,
  Install,
  Platform,
  Preview,
  RemovedSetup,
  SavedSelection,
  SelectionDraft,
  SetupStatus,
  ToolProviderKind,
  WrittenSetup,
} from "../api";

function withDefaults<T>(entries: T[], defaults: Partial<T>): T[] {
  return entries.map((entry) => ({ ...defaults, ...entry }));
}

const harnessCatalog = parse(harnessesToml) as unknown as CatalogBundle["harnesses"] & {
  catalog_revision: string;
};
const catalog: CatalogBundle = {
  revision: harnessCatalog.catalog_revision,
  harnesses: {
    harnesses: withDefaults(harnessCatalog.harnesses, {
      desktop_tool_ids: [],
      desktop_required: false,
      extends: null,
    }),
  },
  tools: parse(toolsToml) as unknown as CatalogBundle["tools"],
  apps: parse(appsToml) as unknown as CatalogBundle["apps"],
  integrations: parse(integrationsToml) as unknown as CatalogBundle["integrations"],
  mcp: parse(mcpToml) as unknown as CatalogBundle["mcp"],
};

const HOME = "/Users/demo";
const MISE_CONFIG_DIR = `${HOME}/.config/mise`;
const fragmentPath = (name: string) => `${MISE_CONFIG_DIR}/conf.d/powermise-${name}.toml`;
const APPLY_COMMAND = "mise bootstrap --yes --only packages,tools";

interface DemoSetup {
  name: string;
  harnesses: string[];
  mcpServers: string[] | null;
  includeDesktopApps: boolean;
  /** Whether the demo applied its MCP servers. */
  mcpApplied?: boolean;
  /** The fragment mise currently reads, once applied. */
  applied: string | null;
  /** Keys the demo bootstrap has installed. */
  installed: Set<string>;
}

const setups: DemoSetup[] = [
  {
    name: "engineering-agents",
    harnesses: ["claude", "codex", "gemini"],
    mcpServers: null,
    includeDesktopApps: false,
    applied: null,
    installed: new Set(),
  },
];

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const PRIORITY: ToolProviderKind[] = [
  "mise-tool",
  "system",
  "bootstrap-brew",
  "bootstrap-brew-cask",
  "macos-app",
  "bootstrap-apt",
  "winget",
  "scoop",
];
const MANAGER: Partial<Record<ToolProviderKind, string>> = {
  "bootstrap-apt": "apt",
  "bootstrap-brew": "brew",
  "bootstrap-brew-cask": "brew-cask",
  "macos-app": "macos-app",
  winget: "winget",
  scoop: "scoop",
};

function compile(draft: SelectionDraft, target: Platform) {
  const byId = new Map(catalog.harnesses.harnesses.map((harness) => [harness.id, harness]));
  const toolsById = new Map([...catalog.tools.tools, ...catalog.apps.apps].map((tool) => [tool.id, tool]));
  const ids = ["base", ...draft.harnesses.filter((id) => id !== "base")];
  const tools = new Set<string>();
  const desktop = new Set<string>();
  const unavailable = new Set<string>();
  const integrations = new Set<string>();
  const mcp = new Set<string>();
  const clients = new Set<McpClientId>();
  for (const id of ids) {
    const harness = byId.get(id);
    if (!harness) continue;
    if (!harness.supported_platforms.includes(target)) {
      throw new Error(`harness \`${id}\` does not support target \`${target}\``);
    }
    harness.required_tool_ids.forEach((tool) => tools.add(tool));
    if (draft.includeDesktopApps || harness.desktop_required) {
      for (const tool of harness.desktop_tool_ids) {
        const available = toolsById
          .get(tool)
          ?.providers.some((provider) => provider.supported_platforms.includes(target));
        if (available) {
          tools.add(tool);
          desktop.add(tool);
        } else unavailable.add(tool);
      }
    }
    harness.integration_ids.forEach((entry) => integrations.add(entry));
    if (!draft.mcpServers) harness.mcp_server_ids.forEach((entry) => mcp.add(entry));
    if (harness.mcp_client) clients.add(harness.mcp_client);
    if (harness.desktop_mcp_client && harness.desktop_tool_ids.some((tool) => desktop.has(tool))) {
      clients.add(harness.desktop_mcp_client);
    }
  }
  draft.mcpServers?.forEach((entry) => mcp.add(entry));

  const installs: Install[] = [];
  for (const id of [...tools].sort()) {
    const provider = [...(toolsById.get(id)?.providers ?? [])]
      .filter((entry) => entry.supported_platforms.includes(target) && (!entry.arch || entry.arch === "arm64"))
      .sort((a, b) => PRIORITY.indexOf(a.kind) - PRIORITY.indexOf(b.kind))[0];
    if (!provider) continue;
    const manager = MANAGER[provider.kind];
    const homebrew = provider.kind === "bootstrap-brew" || provider.kind === "bootstrap-brew-cask";
    if (provider.kind === "system") {
      installs.push({ toolId: id, kind: "system", key: provider.package, version: "" });
      continue;
    }
    installs.push({
      toolId: id,
      kind: provider.kind,
      key: manager ? `${manager}:${provider.package}` : provider.package,
      version: homebrew ? "latest" : provider.version,
    });
  }
  // MCP servers: their tools install with mise, and each agent gets a merge.
  const servers = [...mcp].sort().flatMap((id) => catalog.mcp.mcp_servers.find((server) => server.id === id) ?? []);
  const mcpTargets: McpTarget[] = [];
  const skipped: { id: string; reason: string }[] = [];
  if (clients.size) {
    for (const server of servers) {
      if (server.tool && server.version) {
        installs.push({ toolId: server.id, kind: "mise-tool", key: server.tool, version: server.version });
      }
    }
    for (const client of [...clients].sort()) {
      const path = MCP_PATHS[client]?.(target);
      if (!path) {
        skipped.push({ id: client, reason: `MCP servers for ${MCP_CLIENT_TITLE[client]}: ${MCP_CLIENT_UNSUPPORTED[client] ?? "not on this system"}` });
        continue;
      }
      const usable = servers.filter((server) => !(server.url && client === "claude-desktop"));
      if (usable.length) {
        mcpTargets.push({ client, title: MCP_CLIENT_TITLE[client], target: path, servers: usable.map((server) => server.id) });
      }
    }
  }
  return { ids, tools, desktop, unavailable, integrations, mcp, installs, mcpTargets, skipped };
}

/** Mirrors `McpClient::config_path` in powermise-domain. */
const MCP_PATHS: Partial<Record<McpClientId, (platform: Platform) => string | null>> = {
  "claude-code": () => "~/.claude.json",
  "claude-desktop": (platform) =>
    platform === "darwin"
      ? "~/Library/Application Support/Claude/claude_desktop_config.json"
      : platform === "win32"
        ? "~/AppData/Roaming/Claude/claude_desktop_config.json"
        : null,
  codex: () => "~/.codex/config.toml",
  "gemini-cli": () => "~/.gemini/settings.json",
  opencode: () => "~/.config/opencode/opencode.json",
  cursor: () => "~/.cursor/mcp.json",
  vscode: (platform) =>
    `${platform === "darwin" ? "~/Library/Application Support/Code/User" : platform === "win32" ? "~/AppData/Roaming/Code/User" : "~/.config/Code/User"}/mcp.json`,
  "copilot-cli": () => "~/.copilot/mcp-config.json",
  cline: (platform) =>
    `${platform === "darwin" ? "~/Library/Application Support/Code/User" : platform === "win32" ? "~/AppData/Roaming/Code/User" : "~/.config/Code/User"}/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`,
  antigravity: () => "~/.gemini/antigravity/mcp_config.json",
};

function fragment(name: string, installs: Install[], mcpTargets: McpTarget[] = []): string {
  const tools = installs.filter((install) => install.kind === "mise-tool");
  const packages = installs.filter((install) => install.kind !== "mise-tool" && install.kind !== "system");
  const quote = (key: string) => (/^[A-Za-z0-9_-]+$/.test(key) ? key : `"${key}"`);
  return [
    `# Generated by powermise for the AI setup "${name}". Do not edit:`,
    "# powermise rewrites this file when the setup changes. Remove the setup in",
    "# powermise (or delete this file) to remove it from mise.",
    "",
    ...(tools.length ? ["[tools]", ...tools.map((install) => `${quote(install.key)} = "${install.version}"`), ""] : []),
    ...(packages.length
      ? ["[bootstrap.packages]", ...packages.map((install) => `"${install.key}" = "${install.version}"`), ""]
      : []),
    ...mcpTargets.flatMap((entry) => [
      `[dotfiles."${entry.target}/powermise-${name}"]`,
      `source = "powermise-${name}.files/${entry.client}.${entry.client === "codex" ? "toml" : "json"}"`,
      "merge = true",
      "",
    ]),
  ].join("\n");
}

function change(path: string, current: string | null, desired: string): FileChange {
  if (current === desired) return { path, kind: "unchanged", diff: "" };
  if (current === null) {
    return { path, kind: "create", diff: [`--- /dev/null`, `+++ ${path}`, ...desired.split("\n").map((line) => `+${line}`)].join("\n") };
  }
  const before = current.split("\n");
  const after = desired.split("\n");
  const lines = [`--- ${path}`, `+++ ${path}`];
  for (const line of before) if (!after.includes(line)) lines.push(`-${line}`);
  for (const line of after) lines.push(before.includes(line) ? ` ${line}` : `+${line}`);
  return { path, kind: "update", diff: lines.join("\n") };
}

function preview(draft: SelectionDraft, target: Platform): Preview {
  const name = /^[a-z0-9]+(-[a-z0-9]+)*$/.test(draft.name) ? draft.name : "untitled";
  const compiled = compile(draft, target);
  const contents = fragment(name, compiled.installs, compiled.mcpTargets);
  const existing = setups.find((setup) => setup.name === name)?.applied ?? null;
  return {
    installs: compiled.installs,
    resolved: {
      harnessIds: compiled.ids,
      toolIds: [...compiled.tools].sort(),
      desktopAppIds: [...compiled.desktop],
      unavailableDesktopIds: [...compiled.unavailable],
      integrationIds: [...compiled.integrations],
      mcpServerIds: [...compiled.mcp],
    },
    fragmentPath: fragmentPath(name),
    fragmentContents: contents,
    change: change(fragmentPath(name), existing, contents),
    applyCommand: applyCommand(compiled.mcpTargets),
    skipped: compiled.skipped,
    homebrewSetup: null,
    mcp: compiled.mcpTargets,
  };
}

function saved(setup: DemoSetup): SavedSelection {
  return {
    name: setup.name,
    path: `${HOME}/.config/powermise/selections/${setup.name}.toml`,
    harnesses: setup.harnesses,
    mcpServers: setup.mcpServers,
    includeDesktopApps: setup.includeDesktopApps,
    fragmentPath: fragmentPath(setup.name),
    applied: setup.applied !== null,
  };
}

/** What the demo `mise bootstrap` installs: everything applied setups declare. */
function demoBootstrap(): string[] {
  const lines: string[] = [];
  for (const setup of setups.filter((entry) => entry.applied !== null)) {
    for (const install of compile(setup, "darwin").installs) {
      if (install.kind === "system" || setup.installed.has(install.key)) continue;
      setup.installed.add(install.key);
      lines.push(
        install.kind === "mise-tool"
          ? `mise ${install.key}@${install.version}  ✓ installed`
          : `==> Installing ${install.key.slice(install.key.indexOf(":") + 1)}  ✓`,
      );
    }
  }
  return lines.length ? lines : ["mise all packages and tools are installed"];
}
const demoTargets = (name: string) => {
  const setup = setups.find((entry) => entry.name === name);
  return setup ? compile(setup, "darwin").mcpTargets.map((entry) => entry.target) : [];
};
setBootstrapHook(
  demoBootstrap,
  (name) => {
    const setup = setups.find((entry) => entry.name === name);
    if (!setup) return [];
    setup.mcpApplied = true;
    return compile(setup, "darwin").mcpTargets.map(
      (entry) => `mise edits: applied ${entry.target} (merge:powermise-${name})`,
    );
  },
  demoTargets,
);

function applyCommand(targets: McpTarget[]): string {
  return targets.length
    ? `${APPLY_COMMAND} && mise dotfiles apply --yes ${targets.map((entry) => entry.target).join(" ")}`
    : APPLY_COMMAND;
}

export async function browserPreview<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await sleep(command === "setup_status" || command === "bootstrap_plan" ? 500 : 60);
  const mise = demoMise(command, args);
  if (mise !== NOT_HANDLED) return mise as T;
  switch (command) {
    case "get_environment":
      return {
        hostTarget: "darwin",
        home: HOME,
        powermiseDir: `${HOME}/.config/powermise`,
        miseConfigDir: MISE_CONFIG_DIR,
        misePath: demoHasMise ? `${HOME}/.local/bin/mise` : null,
      } satisfies EnvironmentInfo as T;
    case "get_catalog":
      return catalog as T;
    case "list_selections":
      return [...setups].sort((a, b) => a.name.localeCompare(b.name)).map(saved) as T;
    case "save_selection": {
      const draft = args.draft as SelectionDraft;
      const existing = setups.find((setup) => setup.name === draft.name);
      if (existing) {
        Object.assign(existing, {
          harnesses: draft.harnesses,
          mcpServers: draft.mcpServers,
          includeDesktopApps: draft.includeDesktopApps,
        });
      }
      else setups.push({ ...draft, applied: null, installed: new Set() });
      return `${HOME}/.config/powermise/selections/${draft.name}.toml` as T;
    }
    case "preview_selection":
      return preview(args.draft as SelectionDraft, args.target as Platform) as T;
    case "write_setup": {
      const setup = setups.find((entry) => entry.name === args.name);
      if (!setup) throw new Error(`setup \`${args.name}\` does not exist`);
      const next = preview(setup, args.target as Platform);
      setup.applied = next.fragmentContents;
      return { path: next.fragmentPath, change: next.change, removedServers: [] } satisfies WrittenSetup as T;
    }
    case "bootstrap_plan": {
      const pending = setups
        .filter((setup) => setup.applied !== null)
        .flatMap((setup) =>
          compile(setup, "darwin").installs.filter(
            (install) => install.kind !== "system" && !setup.installed.has(install.key),
          ),
        );
      const resources = [
        ...pending.map((install) => ({
          id: { kind: install.kind === "mise-tool" ? "tool" : "package", name: install.key },
          current: "missing",
          desired: install.version,
          action: "create",
        })),
        // Something from the person's own mise config, so the sheet shows it.
        { id: { kind: "package", name: "brew:wget" }, current: "missing", desired: "latest", action: "create" },
      ];
      return {
        resources,
        summary: { create: resources.length, update: 0, remove: 0, unchanged: 0, unknown: 0 },
      } satisfies BootstrapPlan as T;
    }
    case "setup_status": {
      const setup = setups.find((entry) => entry.name === args.name);
      if (!setup) throw new Error(`setup \`${args.name}\` does not exist`);
      const next = preview(setup, args.target as Platform);
      return {
        name: setup.name,
        fragmentPath: next.fragmentPath,
        change: next.change,
        applyCommand: next.applyCommand,
        mcp: next.mcp.map((entry) => ({ ...entry, state: setup.mcpApplied ? "applied" : "not applied" })),
        items: next.installs.map((install) => {
          const system = install.kind === "system";
          const installed = system || setup.installed.has(install.key);
          return {
            toolId: install.toolId,
            key: install.key,
            kind: install.kind,
            state: system ? "included with the system" : installed ? `installed ${install.version}` : "not installed",
            installed,
            overriddenBy: install.key === "jq" ? `${MISE_CONFIG_DIR}/config.toml` : null,
          };
        }),
      } satisfies SetupStatus as T;
    }
    case "remove_setup": {
      const name = args.name as string;
      const index = setups.findIndex((entry) => entry.name === name);
      if (index >= 0) setups.splice(index, 1);
      return {
        removed: [fragmentPath(name), `${HOME}/.config/powermise/selections/${name}.toml`],
        removedServers: [],
        cleanupCommand: "mise prune --tools",
      } satisfies RemovedSetup as T;
    }
    case "reveal_path":
      return undefined as T;
    default:
      throw new Error(`browser preview does not implement \`${command}\``);
  }
}
