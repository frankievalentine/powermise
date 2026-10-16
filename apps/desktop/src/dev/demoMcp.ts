// The browser demo's Harnesses › MCP Servers page: what's on, a few agents
// "set up" on the demo Mac, and servers someone added outside powermise.
// Mirrors src-tauri/src/mcp.rs. Nothing here touches the machine.
import { MCP_CLIENT_TITLE, MCP_CLIENT_UNSUPPORTED } from "../catalog";
import type { McpAgent, McpClientId, McpPanel, McpPanelView, McpPreview, McpServer, RemovedServer } from "../api";

const HOME = "/Users/demo";
const FRAGMENT = `${HOME}/.config/mise/conf.d/powermise-mcp.toml`;
const ORDER: McpClientId[] = [
  "claude-code",
  "claude-desktop",
  "codex",
  "gemini-cli",
  "opencode",
  "cursor",
  "vscode",
  "copilot-cli",
  "cline",
  "amp",
  "zed",
];
const DETECTED = new Set<McpClientId>(["claude-code", "claude-desktop", "codex", "cursor", "vscode"]);
/** Servers someone added to an agent by hand. */
const OUTSIDE: Partial<Record<McpClientId, string[]>> = { "claude-code": ["linear"], cursor: ["figma"] };

let saved: McpPanel = { servers: [{ id: "context7", clients: ["claude-code", "cursor"] }], custom: [] };
let applied = true;

/** `(setup name, agent, servers)` for applied demo setups. */
type SetupSource = { name: string; client: McpClientId; servers: string[] };

const takesRemote = (client: McpClientId) => client !== "claude-desktop" && client !== "zed";
const onFor = (panel: McpPanel, client: McpClientId) =>
  panel.servers.filter((entry) => entry.clients.includes(client)).map((entry) => entry.id);

export function mcpPanelView(paths: (client: McpClientId) => string | null, setups: SetupSource[]): McpPanelView {
  const agents: McpAgent[] = ORDER.map((id) => {
    const path = id === "zed" ? null : paths(id);
    const unavailable = MCP_CLIENT_UNSUPPORTED[id] ?? (path ? null : "Not available on this system");
    const fromPage = onFor(saved, id);
    const fromSetups = setups.filter((source) => source.client === id);
    const names = [...new Set([...(OUTSIDE[id] ?? []), ...(applied ? fromPage : []), ...fromSetups.flatMap((s) => s.servers)])];
    return {
      id,
      title: MCP_CLIENT_TITLE[id],
      path,
      unavailable,
      detected: DETECTED.has(id),
      takesRemote: takesRemote(id),
      configured: unavailable
        ? null
        : names.map((name) => ({
            name,
            from: [
              ...(fromPage.includes(name) ? ["mcp"] : []),
              ...fromSetups.filter((source) => source.servers.includes(name)).map((source) => source.name),
            ],
          })),
      state: fromPage.length ? (applied ? "applied" : "not applied") : null,
    };
  });
  return { panel: structuredClone(saved), agents, fragmentPath: FRAGMENT };
}

function entry(server: McpServer, client: McpClientId) {
  if (server.url) return client === "claude-code" ? { type: "http", url: server.url } : { url: server.url };
  const args = server.tool
    ? ["x", `${server.tool}@${server.version}`, "--", server.command ?? "", ...server.args]
    : server.args;
  const result: Record<string, unknown> = { command: server.tool ? `${HOME}/.local/bin/mise` : server.command, args };
  if (server.env && Object.keys(server.env).length) result.env = server.env;
  return result;
}

function files(panel: McpPanel, catalog: McpServer[], paths: (client: McpClientId) => string | null) {
  const all = [...catalog, ...panel.custom];
  return ORDER.filter((client) => client !== "zed" && paths(client))
    .map((client) => {
      const servers = onFor(panel, client)
        .map((id) => all.find((server) => server.id === id))
        .filter((server): server is McpServer => !!server && (!server.url || takesRemote(client)));
      const key = client === "codex" ? "mcp_servers" : client === "vscode" ? "servers" : client === "opencode" ? "mcp" : client === "amp" ? "amp.mcpServers" : "mcpServers";
      const contents =
        client === "codex"
          ? servers
              .map((server) => {
                const value = entry(server, client);
                return `[mcp_servers.${server.id}]\n${Object.entries(value)
                  .map(([name, item]) => `${name} = ${JSON.stringify(item)}`)
                  .join("\n")}\n`;
              })
              .join("\n")
          : `${JSON.stringify({ [key]: Object.fromEntries(servers.map((server) => [server.id, entry(server, client)])) }, null, 2)}\n`;
      return { client, servers, title: MCP_CLIENT_TITLE[client], target: paths(client)!, contents };
    })
    .filter((file) => file.servers.length > 0);
}

export function mcpPreview(
  panel: McpPanel,
  catalog: McpServer[],
  paths: (client: McpClientId) => string | null,
  setups: SetupSource[],
): McpPreview {
  const next = files(panel, catalog, paths);
  const removals = ORDER.flatMap((client) => {
    const kept = new Set([...onFor(panel, client), ...setups.filter((s) => s.client === client).flatMap((s) => s.servers)]);
    return onFor(saved, client)
      .filter((server) => !kept.has(server))
      .map((server) => ({ title: MCP_CLIENT_TITLE[client], server, path: paths(client) ?? "" }));
  });
  const tools = [
    ...new Map(
      [...catalog, ...panel.custom]
        .filter((server) => server.tool && panel.servers.some((entry) => entry.id === server.id && entry.clients.length))
        .map((server) => [server.tool!, `${server.tool}@${server.version}`]),
    ).values(),
  ];
  const command = [
    ...(tools.length ? [`mise install ${tools.join(" ")}`] : []),
    ...(next.length
      ? [`mise dotfiles apply --yes ${next.map((file) => file.target.replace("~", HOME)).join(" ")}`]
      : []),
  ].join(" && ");
  return {
    fragmentPath: FRAGMENT,
    fragment: next.length ? "# Written by powermise: the MCP servers turned on in Harnesses › MCP Servers.\n" : null,
    files: next.map(({ title, target, contents }) => ({ title, target, contents })),
    removals,
    skipped: [],
    command: command || null,
  };
}

export function writeMcp(panel: McpPanel): RemovedServer[] {
  const removed = ORDER.flatMap((client) =>
    onFor(saved, client)
      .filter((server) => !onFor(panel, client).includes(server))
      .map((server) => ({ client, server, path: "" })),
  );
  saved = structuredClone(panel);
  applied = false;
  return removed;
}

export function applyMcpCommand(catalog: McpServer[], paths: (client: McpClientId) => string | null): string {
  return mcpPreview(saved, catalog, paths, []).command ?? "";
}

export function applyMcp(): string[] {
  applied = true;
  return ["mise dotfiles: merged MCP servers into each agent’s settings"];
}

export function checkMcpServer(server: McpServer, catalog: McpServer[]) {
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(server.id)) {
    throw new Error(`\`${server.id}\` can't be a server name: use lowercase letters, numbers, hyphens, and underscores`);
  }
  if (catalog.some((entry) => entry.id === server.id)) throw new Error(`\`${server.id}\` is already a server powermise offers`);
  if (server.url && !/^https:\/\/\S+$|^http:\/\/(localhost|127\.0\.0\.1)/.test(server.url)) {
    throw new Error(`\`${server.url}\` isn't a server address: use https:// (or http:// on this computer)`);
  }
}
