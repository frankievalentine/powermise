// Browser-only stand-in for the Rust backend, used when the UI runs in a plain
// browser (`bun run dev` without Tauri) for design work and the marketing-site
// demo. AI Setups read the real catalog TOML files; mise is simulated by
// `demoMise.ts`. Nothing here touches the machine. Never loaded inside Tauri.
import { parse } from "smol-toml";
import harnessesToml from "../../../../catalog/profiles/harnesses.toml?raw";
import toolsToml from "../../../../catalog/profiles/tools.toml?raw";
import appsToml from "../../../../catalog/profiles/apps.toml?raw";
import integrationsToml from "../../../../catalog/profiles/integrations.toml?raw";
import mcpToml from "../../../../catalog/profiles/mcp.toml?raw";
import { demoMise, NOT_HANDLED } from "./demoMise";
export { browserEvents } from "./demoMise";
import type {
  ApplyResult,
  CatalogBundle,
  EnvironmentInfo,
  Operation,
  Platform,
  Preview,
  SavedSelection,
  SelectionDraft,
  StatusResult,
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

const MISE_CONFIG_DIR = "/Users/demo/.config/mise";

let selections: SavedSelection[] = [
  {
    name: "engineering-agents",
    path: "/Users/demo/.config/powermise/selections/engineering-agents.toml",
    harnesses: ["claude", "codex", "gemini"],
    includeDesktopApps: false,
    applied: true,
    lastRun: null,
  },
];

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function resolve(draft: SelectionDraft, target: Platform) {
  const byId = new Map(catalog.harnesses.harnesses.map((harness) => [harness.id, harness]));
  const toolsById = new Map(catalog.tools.tools.map((tool) => [tool.id, tool]));
  const ids = ["base", ...draft.harnesses.filter((id) => id !== "base")];
  const tools = new Set<string>();
  const desktop = new Set<string>();
  const unavailable = new Set<string>();
  const integrations = new Set<string>();
  const mcp = new Set<string>();
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
        } else {
          unavailable.add(tool);
        }
      }
    }
    harness.integration_ids.forEach((entry) => integrations.add(entry));
    harness.mcp_server_ids.forEach((entry) => mcp.add(entry));
  }
  return { ids, tools, desktop, unavailable, integrations, mcp, toolsById };
}

function preview(draft: SelectionDraft, target: Platform): Preview {
  const name = /^[a-z0-9]+(-[a-z0-9]+)*$/.test(draft.name) ? draft.name : "untitled";
  const resolved = resolve(draft, target);
  const operations: Operation[] = [];
  const installs: Preview["installs"] = [];
  const toolLines: string[] = [];
  const packageLines: string[] = [];
  for (const id of [...resolved.tools].sort()) {
    const provider = resolved.toolsById
      .get(id)
      ?.providers.find((entry) => entry.supported_platforms.includes(target));
    if (!provider) continue;
    installs.push({ toolId: id, kind: provider.kind, package: provider.package, version: provider.version });
    if (provider.kind === "mise-tool") {
      toolLines.push(`"${provider.package}" = "${provider.version}"`);
      operations.push(op(`mise-tool:${provider.package}`, "mise-tools", "tool", provider.package, provider.version));
    } else if (provider.kind === "winget" || provider.kind === "scoop") {
      operations.push(op(`native-${provider.kind}:${provider.package}`, provider.kind, "tool", provider.package, provider.version));
    } else {
      const source = provider.kind.replace("bootstrap-", "");
      packageLines.push(`"${source}:${provider.package}" = "${provider.version}"`);
      operations.push(op(`mise-bootstrap:${source}:${provider.package}`, "mise-bootstrap", "package", `${source}:${provider.package}`, provider.version));
    }
  }
  for (const id of resolved.integrations) {
    operations.push(op(`config-adapter:integration:${id}`, "config-adapter", "integration", id, "claude"));
  }
  for (const id of resolved.mcp) {
    operations.push(op(`config-adapter:mcp-server:${id}`, "config-adapter", "mcp-server", id, "claude"));
  }
  const contents = [
    `# Generated by powermise. Source selection: ${name}. Do not edit.`,
    "",
    "[tools]",
    ...toolLines,
    "",
    "[bootstrap.packages]",
    ...packageLines,
    "",
  ].join("\n");
  const path = `${MISE_CONFIG_DIR}/config.powermise-${name}.toml`;
  return {
    installs,
    resolved: {
      harnessIds: resolved.ids,
      toolIds: [...resolved.tools].sort(),
      desktopAppIds: [...resolved.desktop],
      unavailableDesktopIds: [...resolved.unavailable],
      appIds: [],
      integrationIds: [...resolved.integrations],
      mcpServerIds: [...resolved.mcp],
    },
    operations,
    configPath: path,
    configContents: contents,
    change: {
      path,
      kind: "create",
      diff: `--- ${path}\n+++ ${path}\n${contents
        .split("\n")
        .map((line) => `+${line}`)
        .join("\n")}`,
    },
    applyCommand: `powermise apply --name ${name} --target ${target} --yes`,
  };
}

function op(
  id: string,
  provider: Operation["provider"],
  resource_kind: string,
  resource_id: string,
  requested_state: string,
): Operation {
  return { id, provider, resource_kind, resource_id, requested_state };
}

export async function browserPreview<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  await sleep(command === "apply_selection" ? 2200 : command === "selection_status" ? 700 : 60);
  const mise = demoMise(command, args);
  if (mise !== NOT_HANDLED) return mise as T;
  switch (command) {
    case "get_environment":
      return {
        hostTarget: "darwin",
        powermiseDir: "/Users/demo/.config/powermise",
        miseConfigDir: MISE_CONFIG_DIR,
        misePath: "/Users/demo/.local/bin/mise",
        miseVersion: "2026.7.18 macos-arm64 (browser preview)",
      } satisfies EnvironmentInfo as T;
    case "get_catalog":
      return catalog as T;
    case "list_selections":
      return selections as T;
    case "save_selection": {
      const draft = args.draft as SelectionDraft;
      const entry: SavedSelection = {
        name: draft.name,
        path: `/Users/demo/.config/powermise/selections/${draft.name}.toml`,
        harnesses: draft.harnesses,
        includeDesktopApps: draft.includeDesktopApps,
        applied: false,
        lastRun: null,
      };
      selections = [...selections.filter((saved) => saved.name !== draft.name), entry].sort(
        (a, b) => a.name.localeCompare(b.name),
      );
      return entry.path as T;
    }
    case "preview_selection":
      return preview(args.draft as SelectionDraft, args.target as Platform) as T;
    case "apply_selection": {
      const name = args.name as string;
      const saved = selections.find((entry) => entry.name === name);
      const result = preview(
        { name, harnesses: saved?.harnesses ?? [], includeDesktopApps: !!saved?.includeDesktopApps },
        args.target as Platform,
      );
      const results = result.operations.map((operation) => ({
        operation_id: operation.id,
        outcome: operation.provider === "config-adapter" ? ("skipped" as const) : ("succeeded" as const),
        message:
          operation.provider === "config-adapter"
            ? `${operation.resource_id} is not configured yet; set it up manually`
            : "converged by real mise bootstrap",
      }));
      selections = selections.map((entry) =>
        entry.name === name
          ? { ...entry, applied: true, lastRun: { selection: name, platform: "darwin", operations: results } }
          : entry,
      );
      return {
        selection: name,
        results,
        bootstrap: { state: "converged" },
        followUps: [],
        recoveryGuidance: [],
        command: result.applyCommand,
      } satisfies ApplyResult as T;
    }
    case "selection_status":
      return {
        selection: args.name as string,
        satisfied: true,
        change: { path: "", kind: "unchanged", diff: "" },
        drift: null,
        ledgerOwned: true,
        followUps: [],
        native: [],
        command: `powermise status --name ${args.name} --target ${args.target}`,
      } satisfies StatusResult as T;
    case "remove_selection":
      selections = selections.filter((entry) => entry.name !== args.name);
      return { removed: [], commands: [`powermise uninstall --name ${args.name} --yes`] } as T;
    case "reveal_path":
      return undefined as T;
    default:
      throw new Error(`browser preview does not implement \`${command}\``);
  }
}
