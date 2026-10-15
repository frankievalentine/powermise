import type {
  CatalogBundle,
  Integration,
  AgentCli,
  Harness,
  McpClientId,
  McpServer,
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
  integrations: Integration[];
  mcpServers: McpServer[];
  mcpById: Map<string, McpServer>;
  configById: Map<string, Integration>;
}

export function indexCatalog(bundle: CatalogBundle): CatalogIndex {
  const all = bundle.harnesses.harnesses;
  // Apps that are also tools (vscode, zed, cursor) are listed once, as tools.
  const toolIds = new Set(bundle.tools.tools.map((tool) => tool.id));
  const tools = [...bundle.tools.tools, ...bundle.apps.apps.filter((app) => !toolIds.has(app.id))];
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
    mcpById: new Map(mcpServers.map((server) => [server.id, server])),
    configById: new Map(integrations.map((entry) => [entry.id, entry])),
  };
}

/** Mirrors the compiler's `chosen_provider` for browsing the catalog without
 *  compiling; setup previews use the compiler's own answer (`Preview.installs`). */
const PROVIDER_PRIORITY: ToolProviderKind[] = [
  "mise-tool",
  "system",
  "bootstrap-brew",
  "bootstrap-brew-cask",
  "macos-app",
  "bootstrap-apt",
  "winget",
  "scoop",
];

export function providerFor(tool: Tool | undefined, target: Platform): ToolProvider | undefined {
  if (!tool) return undefined;
  return [...tool.providers]
    // Browsing assumes an Apple Silicon Mac for per-architecture downloads.
    .filter((provider) => provider.supported_platforms.includes(target) && (!provider.arch || provider.arch === "arm64"))
    .sort((a, b) => PROVIDER_PRIORITY.indexOf(a.kind) - PROVIDER_PRIORITY.indexOf(b.kind))[0];
}

export const PROVIDER_LABEL: Record<ToolProviderKind, string> = {
  "mise-tool": "mise",
  system: "Comes with the system",
  "bootstrap-apt": "apt",
  "bootstrap-brew": "Homebrew · via mise",
  "bootstrap-brew-cask": "Mac app · Homebrew Cask",
  "macos-app": "Mac app · vendor download",
  winget: "winget",
  scoop: "Scoop",
};

/** Short labels for tight spaces, like the Agent Tools table. */
export const PROVIDER_SHORT: Record<ToolProviderKind, string> = {
  "mise-tool": "mise",
  system: "Built in",
  "bootstrap-apt": "apt",
  "bootstrap-brew": "Homebrew",
  "bootstrap-brew-cask": "Mac app",
  "macos-app": "Mac app",
  winget: "winget",
  scoop: "Scoop",
};

/** What each install source means, for tooltips. */
export const PROVIDER_HINT: Record<ToolProviderKind, string> = {
  "mise-tool": "mise downloads and manages this version itself.",
  system: "Already part of the operating system, so nothing is installed.",
  "bootstrap-apt": "Installed with apt, which needs an administrator password.",
  "bootstrap-brew":
    "A Homebrew package. mise downloads and installs it itself — you don’t need Homebrew installed.",
  "bootstrap-brew-cask":
    "A Mac app from Homebrew Cask, installed into Applications by mise — no Homebrew needed.",
  "macos-app":
    "A Mac app downloaded from its maker, checked against a published checksum, and installed into Applications by mise.",
  winget: "Installed with Windows’ winget package manager.",
  scoop: "Installed with Scoop.",
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
    index.mcpById.get(id)?.title ??
    index.harnessById.get(id)?.title ??
    id
  );
}

export const MCP_CLIENT_TITLE: Record<McpClientId, string> = {
  "claude-code": "Claude Code",
  "claude-desktop": "Claude desktop app",
  codex: "Codex",
  "gemini-cli": "Gemini CLI",
  opencode: "OpenCode",
  cursor: "Cursor",
  vscode: "VS Code",
  "copilot-cli": "Copilot CLI",
  cline: "Cline",
  antigravity: "Antigravity",
  zed: "Zed",
};

/** Agents powermise can't add MCP servers to, and why. */
export const MCP_CLIENT_UNSUPPORTED: Partial<Record<McpClientId, string>> = {
  zed: "Zed’s settings file can hold comments, which mise won’t merge into. Add servers in Zed’s Agent Panel settings.",
};

/** The harness's desktop MCP config, when its desktop app installs on
 *  `platform` (Claude's desktop app isn't on Linux, nor VS Code). */
export function desktopMcpClient(index: CatalogIndex, harness: Harness, platform: Platform): McpClientId | null {
  if (!harness.desktop_mcp_client) return null;
  const installable = harness.desktop_tool_ids.some((id) => providerFor(index.toolById.get(id), platform));
  return installable ? harness.desktop_mcp_client : null;
}

/** How an MCP server runs, in words. */
export function mcpRunsAs(server: McpServer): string {
  if (server.url) return "Remote server · nothing to install";
  const backend = server.tool?.split(":")[0];
  const kind = backend === "npm" ? "npm package" : backend === "pipx" ? "Python package" : `${backend} tool`;
  return `${kind} · mise installs and runs it`;
}

export const AGENT_CLI_TITLE: Record<AgentCli, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
};

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
