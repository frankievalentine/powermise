import type {
  CatalogBundle,
  ConfigEntry,
  Harness,
  Platform,
  Tool,
  ToolProvider,
  ToolProviderKind,
} from "./api";

export interface CatalogIndex {
  revision: string;
  /** Every selectable harness (excludes the implicit `base`). */
  harnesses: Harness[];
  base: Harness | undefined;
  harnessById: Map<string, Harness>;
  tools: Tool[];
  toolById: Map<string, Tool>;
  integrations: ConfigEntry[];
  mcpServers: ConfigEntry[];
  configById: Map<string, ConfigEntry>;
  /** Tool by provider package key, e.g. `mise-tool:claude-code` or
   *  `bootstrap-brew-cask:claude`. */
  toolByPackage: Map<string, Tool>;
}

export function indexCatalog(bundle: CatalogBundle): CatalogIndex {
  const all = bundle.harnesses.harnesses;
  const tools = [...bundle.tools.tools, ...bundle.apps.apps];
  const integrations = bundle.integrations.integrations;
  const mcpServers = bundle.mcp.mcp_servers;
  return {
    revision: bundle.revision,
    harnesses: all.filter((harness) => harness.id !== "base"),
    base: all.find((harness) => harness.id === "base"),
    harnessById: new Map(all.map((harness) => [harness.id, harness])),
    tools,
    toolById: new Map(tools.map((tool) => [tool.id, tool])),
    integrations,
    mcpServers,
    configById: new Map([...integrations, ...mcpServers].map((entry) => [entry.id, entry])),
    toolByPackage: new Map(
      tools.flatMap((tool) =>
        tool.providers.map((provider) => [`${provider.kind}:${provider.package}`, tool] as const),
      ),
    ),
  };
}

const BOOTSTRAP_SOURCES: Record<string, ToolProviderKind> = {
  brew: "bootstrap-brew",
  "brew-cask": "bootstrap-brew-cask",
  apt: "bootstrap-apt",
};

/** A readable name for a plan operation ID such as `mise-tool:npm:@openai/codex`,
 *  `mise-bootstrap:brew-cask:claude`, `native-winget:Git.Git`, or
 *  `config-adapter:mcp-server:claude-context7`. */
export function operationTitle(index: CatalogIndex, operationId: string): string {
  const [kind, ...rest] = operationId.split(":");
  const resource = rest.join(":");
  let tool: Tool | undefined;
  if (kind === "mise-tool") {
    tool = index.toolByPackage.get(`mise-tool:${resource}`);
  } else if (kind === "mise-bootstrap") {
    const [source, ...packageParts] = resource.split(":");
    const providerKind = BOOTSTRAP_SOURCES[source];
    tool = providerKind && index.toolByPackage.get(`${providerKind}:${packageParts.join(":")}`);
  } else if (kind.startsWith("native-")) {
    tool = index.toolByPackage.get(`${kind.slice("native-".length)}:${resource}`);
  } else if (kind === "config-adapter") {
    const id = rest.slice(1).join(":");
    return index.configById.get(id)?.title ?? id;
  }
  return tool?.title ?? resource;
}

/** Mirrors the compiler's `chosen_provider` for browsing the catalog without
 *  compiling; setup previews use the compiler's own answer (`Preview.installs`). */
const PROVIDER_PRIORITY: ToolProviderKind[] = [
  "mise-tool",
  "bootstrap-apt",
  "bootstrap-brew",
  "bootstrap-brew-cask",
  "winget",
  "scoop",
];

export function providerFor(tool: Tool | undefined, target: Platform): ToolProvider | undefined {
  if (!tool) return undefined;
  return [...tool.providers]
    .filter((provider) => provider.supported_platforms.includes(target))
    .sort((a, b) => PROVIDER_PRIORITY.indexOf(a.kind) - PROVIDER_PRIORITY.indexOf(b.kind))[0];
}

export const PROVIDER_LABEL: Record<ToolProviderKind, string> = {
  "mise-tool": "mise",
  "bootstrap-apt": "apt",
  "bootstrap-brew": "Homebrew",
  "bootstrap-brew-cask": "Homebrew Cask",
  winget: "winget",
  scoop: "Scoop",
};

export const CATEGORY_LABEL: Record<string, string> = {
  terminal: "Terminal agent",
  ide: "Editor & IDE",
  gui: "Desktop app",
};

export function categoryLabel(category: string): string {
  return CATEGORY_LABEL[category] ?? category;
}

export function isAvailable(harness: Harness, target: Platform): boolean {
  return harness.supported_platforms.includes(target);
}

export function titleFor(index: CatalogIndex, id: string): string {
  return (
    index.toolById.get(id)?.title ??
    index.configById.get(id)?.title ??
    index.harnessById.get(id)?.title ??
    id
  );
}

/** Agent names for config targets such as `claude` or `codex`. */
export function targetTitle(index: CatalogIndex, configTarget: string): string {
  return index.harnessById.get(configTarget)?.title ?? configTarget;
}

const MONOGRAM_PALETTE: [string, string][] = [
  ["#ff9f0a", "#ff6b00"],
  ["#0a84ff", "#0060df"],
  ["#30d158", "#14a33d"],
  ["#bf5af2", "#8e3cc9"],
  ["#ff375f", "#d81e48"],
  ["#64d2ff", "#2a9fd6"],
  ["#5e5ce6", "#3f3dc2"],
  ["#ffd60a", "#e0a800"],
  ["#ac8e68", "#7d6547"],
  ["#40c8e0", "#1d97ad"],
];

export function monogramColors(id: string): [string, string] {
  let hash = 0;
  for (const character of id) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return MONOGRAM_PALETTE[hash % MONOGRAM_PALETTE.length];
}

export function monogramLetters(title: string): string {
  const words = title.split(/\s+/).filter(Boolean);
  if (words.length > 1) return (words[0][0] + words[1][0]).toUpperCase();
  return title.slice(0, 2).replace(/^./, (letter) => letter.toUpperCase());
}
