import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, ExternalLink, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import {
  api,
  type McpAgent,
  type McpClientId,
  type McpPanel,
  type McpPreview,
  type McpServer,
} from "../api";
import type { AppContext } from "../App";
import { mcpRunsAs } from "../catalog";
import { canOpen } from "../components/machine";
import { HarnessesTabs } from "../components/SectionTabs";
import { Sheet } from "../components/Sheet";
import { CommandBlock, Monogram, Section, Segmented } from "../components/ui";
import { useAsync } from "../hooks";
import { prettyPath } from "../mise";

const same = (a: McpPanel, b: McpPanel) => JSON.stringify(normalize(a)) === JSON.stringify(normalize(b));

/** Drops servers that are off everywhere, and sorts, so equal pages compare equal. */
function normalize(panel: McpPanel): McpPanel {
  return {
    servers: panel.servers
      .filter((entry) => entry.clients.length > 0)
      .map((entry) => ({ id: entry.id, clients: [...entry.clients].sort() }))
      .sort((a, b) => a.id.localeCompare(b.id)),
    custom: [...panel.custom].sort((a, b) => a.id.localeCompare(b.id)),
  };
}

/** Harnesses › MCP Servers: turn servers on for each agent, see what each
 *  agent's config has now, and add servers of your own (src-tauri/src/mcp.rs). */
export function McpServersView({ context, query }: { context: AppContext; query: string }) {
  const { catalog, env, mise, run } = context;
  const view = useAsync(() => api.mcpPanel(), [mise.revision]);
  const [draft, setDraft] = useState<McpPanel | null>(null);
  const [showAll, setShowAll] = useState(false);
  const [review, setReview] = useState<McpPreview | null>(null);
  const [editing, setEditing] = useState<McpServer | "new" | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const saved = view.data?.panel;
  useEffect(() => {
    if (saved) setDraft(normalize(saved));
  }, [saved]);
  const panel = draft ?? (saved ? normalize(saved) : null);
  const dirty = !!saved && !!panel && !same(saved, panel);

  const agents = view.data?.agents ?? [];
  const usable = agents.filter((agent) => !agent.unavailable);
  const isOn = (server: string, client: McpClientId) =>
    !!panel?.servers.find((entry) => entry.id === server)?.clients.includes(client);
  // Agents set up on this computer, plus any a server is already on for.
  const shown = usable.filter(
    (agent) => showAll || agent.detected || panel?.servers.some((entry) => entry.clients.includes(agent.id)),
  );
  const hidden = usable.length - shown.length;

  const servers = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const all = [...catalog.mcpServers, ...(panel?.custom ?? [])];
    return all.filter(
      (server) =>
        !needle || server.title.toLowerCase().includes(needle) || server.description.toLowerCase().includes(needle),
    );
  }, [catalog.mcpServers, panel?.custom, query]);
  const custom = new Set(panel?.custom.map((server) => server.id));

  const update = (change: (panel: McpPanel) => McpPanel) => setDraft((current) => (current ? normalize(change(current)) : current));
  const toggle = (server: string, client: McpClientId) =>
    update((current) => {
      const entry = current.servers.find((candidate) => candidate.id === server);
      const clients = entry?.clients.includes(client)
        ? entry.clients.filter((id) => id !== client)
        : [...(entry?.clients ?? []), client];
      return {
        ...current,
        servers: [...current.servers.filter((candidate) => candidate.id !== server), { id: server, clients }],
      };
    });
  const saveServer = (server: McpServer, clients: McpClientId[]) =>
    update((current) => ({
      custom: [...current.custom.filter((entry) => entry.id !== server.id), server],
      servers: [...current.servers.filter((entry) => entry.id !== server.id), { id: server.id, clients }],
    }));
  const removeServer = (id: string) =>
    update((current) => ({
      custom: current.custom.filter((entry) => entry.id !== id),
      servers: current.servers.filter((entry) => entry.id !== id),
    }));

  const openReview = async () => {
    if (!panel) return;
    setError(null);
    setBusy(true);
    try {
      setReview(await api.mcpPreview(panel));
    } catch (reason) {
      setError((reason as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const apply = async () => {
    if (!panel || !review) return;
    setBusy(true);
    setError(null);
    try {
      await api.writeMcp(panel);
      const command = review.command;
      setReview(null);
      if (command) {
        run({ action: { kind: "applyMcp" }, title: "Applying MCP servers", description: "", verb: "Apply", autoStart: true });
      }
      view.reload();
    } catch (reason) {
      setError((reason as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const configuredAgents = agents.filter(
    (agent) => agent.detected || (agent.configured?.length ?? 0) > 0 || agent.state,
  );

  return (
    <div className="content">
      <div className="page">
        <HarnessesTabs context={context} current="mcp" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Harnesses</h1>
            <p className="page-subtitle">
              MCP servers give an agent extra abilities, like reading current library docs or using a browser. Turn
              one on for the agents you use: mise installs it and adds it to each agent’s own config.
            </p>
          </div>
        </div>

        {error && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}
        {view.error && !view.data && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{view.error}</span>
          </div>
        )}
        <section className="section">
          <div className="section-heading-row">
            <h2 className="section-title">Servers</h2>
            <div className="detail-actions">
              {hidden > 0 && (
                <button className="button" onClick={() => setShowAll(true)}>
                  Show {hidden} More {hidden === 1 ? "Agent" : "Agents"}
                </button>
              )}
              <button className="button" onClick={() => setEditing("new")} disabled={!panel}>
                <Plus size={13} /> Add Server…
              </button>
            </div>
          </div>
          {view.loading && !view.data ? (
            <p className="field-hint">
              <Loader2 size={12} className="spin" /> Reading your agents’ settings…
            </p>
          ) : shown.length === 0 ? (
            <p className="section-note">
              No agents with MCP settings are set up here yet.{" "}
              <button className="link-button" onClick={() => setShowAll(true)}>
                Show every agent
              </button>
            </p>
          ) : null}
          <div className="list">
            {servers.map((server) => (
              <div className="list-row mcp-server-row" key={server.id}>
                <Monogram id={server.id} title={server.title} size={28} />
                <div className="row-main">
                  <div className="row-title">
                    {canOpen(server.homepage) ? (
                      <button
                        className="title-link"
                        onClick={() => void api.openUrl(server.homepage!)}
                        title={`Open ${server.title}’s page`}
                      >
                        {server.title}
                        <ExternalLink size={11} aria-hidden />
                      </button>
                    ) : (
                      server.title
                    )}
                    {custom.has(server.id) && <span className="badge">Added by you</span>}
                  </div>
                  <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                    {server.description}
                  </div>
                  <div className="row-subtitle">
                    {mcpRunsAs(server)}
                    {server.tool && (
                      <>
                        {" · "}
                        <span className="mono">
                          {server.tool}@{server.version}
                        </span>
                      </>
                    )}
                  </div>
                  {shown.length > 0 && (
                    <div className="chip-row mcp-agent-chips" role="group" aria-label={`Agents ${server.title} is on for`}>
                      {shown.map((agent) => {
                        const blocked = !!server.url && !agent.takesRemote;
                        const on = isOn(server.id, agent.id);
                        return (
                          <button
                            key={agent.id}
                            type="button"
                            className={`chip-button${on ? " selected" : ""}`}
                            aria-pressed={on}
                            aria-label={agent.title}
                            disabled={blocked && !on}
                            title={blocked ? `${agent.title} only runs servers itself, not remote ones` : undefined}
                            onClick={() => toggle(server.id, agent.id)}
                          >
                            {agent.title}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
                <div className="row-trailing">
                  {custom.has(server.id) ? (
                    <>
                      <button className="icon-button" aria-label={`Edit ${server.title}`} title="Edit" onClick={() => setEditing(server)}>
                        <Pencil size={14} />
                      </button>
                      <button
                        className="icon-button"
                        aria-label={`Remove ${server.title}`}
                        title="Remove"
                        onClick={() => removeServer(server.id)}
                      >
                        <Trash2 size={14} />
                      </button>
                    </>
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        </section>

        {dirty && (
          <div className="banner mcp-pending">
            <span>Your changes aren’t applied yet.</span>
            <div className="detail-actions">
              <button className="button" onClick={() => saved && setDraft(normalize(saved))} disabled={busy}>
                Discard
              </button>
              <button className="button primary" onClick={() => void openReview()} disabled={busy}>
                {busy && <Loader2 size={13} className="spin" />} Review and Apply…
              </button>
            </div>
          </div>
        )}

        {configuredAgents.length > 0 && (
          <Section
            title="In each agent’s settings now"
            note="What each agent’s own config file lists, including servers you or the agent added. Setups add servers too: one stays in an agent’s settings while anything in powermise still adds it."
          >
            <div className="list">
              {configuredAgents.map((agent) => (
                <AgentRow key={agent.id} agent={agent} home={env.home} />
              ))}
            </div>
          </Section>
        )}

        {view.data && (
          <p className="section-note">
            powermise keeps what’s on in{" "}
            <span className="mono selectable">{prettyPath(view.data.fragmentPath, env.home)}</span>, which mise reads
            like any of your config files. Each agent’s settings change only with a mise dotfiles merge, which owns just
            these servers and leaves everything else in the file alone.
          </p>
        )}
      </div>

      <ReviewSheet
        preview={review}
        home={env.home}
        busy={busy}
        onCancel={() => setReview(null)}
        onApply={() => void apply()}
      />
      <ServerSheet
        editing={editing}
        agents={usable}
        defaultClients={shown.filter((agent) => agent.detected).map((agent) => agent.id)}
        clientsOf={(id) => panel?.servers.find((entry) => entry.id === id)?.clients ?? []}
        taken={(id) => catalog.mcpServers.some((server) => server.id === id) || (editing === "new" && custom.has(id))}
        onCancel={() => setEditing(null)}
        onSave={(server, clients) => {
          saveServer(server, clients);
          setEditing(null);
        }}
      />
    </div>
  );
}

function AgentRow({ agent, home }: { agent: McpAgent; home: string | null }) {
  const sourceLabel = (from: string[]) =>
    from.map((source) => (source === "mcp" ? "this page" : `setup ${source}`)).join(", ");
  return (
    <div className="list-row">
      <div className="row-main">
        <div className="row-title">{agent.title}</div>
        <div className="row-subtitle mono">{agent.path ? prettyPath(agent.path, home) : ""}</div>
        <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
          {agent.unavailable ??
            (agent.configured === null
              ? "powermise can’t read this file (it may have comments), so it can’t list or change its servers."
              : agent.configured.length === 0
                ? "No MCP servers yet."
                : null)}
        </div>
        {agent.configured && agent.configured.length > 0 && (
          <div className="chip-row mcp-agent-chips">
            {agent.configured.map((server) => (
              <span
                key={server.name}
                className={`badge${server.from.length ? " green" : ""}`}
                title={server.from.length ? `Added by powermise: ${sourceLabel(server.from)}` : "Added outside powermise"}
              >
                {server.name}
              </span>
            ))}
          </div>
        )}
      </div>
      <div className="row-trailing">
        {agent.state && agent.state !== "applied" && (
          <span className="badge orange" title={`mise reports this page’s servers here as “${agent.state}”`}>
            {agent.state === "not applied" ? "Not applied" : "Out of date"}
          </span>
        )}
      </div>
    </div>
  );
}

function ReviewSheet({
  preview,
  home,
  busy,
  onCancel,
  onApply,
}: {
  preview: McpPreview | null;
  home: string | null;
  busy: boolean;
  onCancel: () => void;
  onApply: () => void;
}) {
  return (
    <Sheet open={!!preview} onDismiss={onCancel} labelledBy="mcp-review-title">
      <div className="sheet-head">
        <h2 id="mcp-review-title">Apply MCP server changes?</h2>
        <p>
          {preview?.fragment
            ? `powermise writes ${prettyPath(preview.fragmentPath, home)}, and mise adds the servers to each agent’s settings.`
            : `Nothing is on anymore, so powermise deletes ${preview ? prettyPath(preview.fragmentPath, home) : ""}.`}
        </p>
      </div>
      <div className="sheet-body">
        {preview?.files.map((file) => (
          <div key={file.target}>
            <div className="side-heading" style={{ marginTop: 0 }}>
              {file.title} · {prettyPath(file.target, home)}
            </div>
            <pre className="code-view selectable">{file.contents}</pre>
          </div>
        ))}
        {preview && preview.removals.length > 0 && (
          <>
            <div className="side-heading">Taken out of agents’ settings</div>
            <ul className="plain-list">
              {preview.removals.map((removal) => (
                <li key={`${removal.path}/${removal.server}`}>
                  <span className="mono">{removal.server}</span> from {removal.title}
                </li>
              ))}
            </ul>
          </>
        )}
        {preview && preview.skipped.length > 0 && (
          <>
            <div className="side-heading">Left out</div>
            <ul className="plain-list">
              {preview.skipped.map((skip) => (
                <li key={`${skip.id}/${skip.reason}`}>{skip.reason}</li>
              ))}
            </ul>
          </>
        )}
        {preview?.command && (
          <>
            <div className="side-heading">Then mise runs</div>
            <CommandBlock command={preview.command} />
          </>
        )}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onCancel}>
          Cancel
        </button>
        <button className="button primary large" onClick={onApply} disabled={busy}>
          {busy && <Loader2 size={14} className="spin" />} Apply
        </button>
      </div>
    </Sheet>
  );
}

type Kind = "npm" | "pipx" | "mise" | "command" | "remote";

const KINDS: { value: Kind; label: string }[] = [
  { value: "npm", label: "npm" },
  { value: "pipx", label: "Python" },
  { value: "mise", label: "mise Tool" },
  { value: "command", label: "Command" },
  { value: "remote", label: "Remote" },
];

const PACKAGE_HINT: Record<Kind, string> = {
  npm: "@modelcontextprotocol/server-github",
  pipx: "mcp-server-fetch",
  mise: "github:owner/repo",
  command: "",
  remote: "",
};

function kindOf(server: McpServer): Kind {
  if (server.url) return "remote";
  if (!server.tool) return "command";
  if (server.tool.startsWith("npm:")) return "npm";
  if (server.tool.startsWith("pipx:")) return "pipx";
  return "mise";
}

/** The command a package most likely installs: its name without a scope. */
const commandFor = (pkg: string) => pkg.split("/").pop()?.split("[")[0] ?? "";

/** Adds or edits a server of your own. */
function ServerSheet({
  editing,
  agents,
  defaultClients,
  clientsOf,
  taken,
  onCancel,
  onSave,
}: {
  editing: McpServer | "new" | null;
  agents: McpAgent[];
  defaultClients: McpClientId[];
  clientsOf: (id: string) => McpClientId[];
  taken: (id: string) => boolean;
  onCancel: () => void;
  onSave: (server: McpServer, clients: McpClientId[]) => void;
}) {
  const [kind, setKind] = useState<Kind>("npm");
  const [name, setName] = useState("");
  const [pkg, setPkg] = useState("");
  const [version, setVersion] = useState("latest");
  const [command, setCommand] = useState("");
  const [commandTouched, setCommandTouched] = useState(false);
  const [args, setArgs] = useState("");
  const [url, setUrl] = useState("");
  const [env, setEnv] = useState<[string, string][]>([]);
  const [clients, setClients] = useState<McpClientId[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (!editing) return;
    setError(null);
    if (editing === "new") {
      setKind("npm");
      setName("");
      setPkg("");
      setVersion("latest");
      setCommand("");
      setCommandTouched(false);
      setArgs("");
      setUrl("");
      setEnv([]);
      setClients(defaultClients);
      return;
    }
    const current = kindOf(editing);
    setKind(current);
    setName(editing.id);
    setPkg(editing.tool?.replace(/^(npm|pipx):/, "") ?? "");
    setVersion(editing.version ?? "latest");
    setCommand(editing.command ?? "");
    setCommandTouched(true);
    setArgs(editing.args.join(" "));
    setUrl(editing.url ?? "");
    setEnv(Object.entries(editing.env ?? {}));
    setClients(clientsOf(editing.id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing]);

  const packaged = kind === "npm" || kind === "pipx" || kind === "mise";
  const server = (): McpServer => {
    const tool = kind === "npm" ? `npm:${pkg.trim()}` : kind === "pipx" ? `pipx:${pkg.trim()}` : pkg.trim();
    const id = name.trim();
    return {
      id,
      title: id,
      description:
        kind === "remote"
          ? `A remote server at ${url.trim()}.`
          : packaged
            ? `Added by you: ${tool}.`
            : `Added by you: runs ${command.trim()}.`,
      homepage: null,
      tool: packaged ? tool : null,
      version: packaged ? version.trim() || "latest" : null,
      command: kind === "remote" ? null : command.trim(),
      args: kind === "remote" ? [] : args.split(/\s+/).filter(Boolean),
      url: kind === "remote" ? url.trim() : null,
      env: kind === "remote" ? {} : Object.fromEntries(env.filter(([key]) => key.trim()).map(([key, value]) => [key.trim(), value])),
    };
  };
  const save = async () => {
    const next = server();
    if (editing === "new" && taken(next.id)) {
      setError(`There’s already a server named “${next.id}”.`);
      return;
    }
    setChecking(true);
    setError(null);
    try {
      await api.checkMcpServer(next);
      onSave(next, clients.filter((client) => kind !== "remote" || agents.find((agent) => agent.id === client)?.takesRemote));
    } catch (reason) {
      setError((reason as Error).message);
    } finally {
      setChecking(false);
    }
  };
  const ready = name.trim() && (kind === "remote" ? url.trim() : command.trim() && (!packaged || pkg.trim()));

  return (
    <Sheet open={!!editing} onDismiss={onCancel} labelledBy="mcp-server-title">
      <div className="sheet-head">
        <h2 id="mcp-server-title">{editing === "new" ? "Add a server" : `Edit ${name}`}</h2>
        <p>
          A server from npm or Python runs through mise, which installs it the first time. A command runs as it is; a
          remote server is just an address.
        </p>
      </div>
      <div className="sheet-body">
        <Segmented label="How it runs" value={kind} options={KINDS} onChange={setKind} />
        <label className="task-field" style={{ marginTop: 14 }}>
          <span className="task-field__label">Name</span>
          <input
            className="text-field mono"
            value={name}
            disabled={editing !== "new"}
            onChange={(event) => setName(event.target.value.toLowerCase().replace(/\s+/g, "-"))}
            placeholder="github"
            spellCheck={false}
          />
          <span className="field-hint">How agents list it: lowercase letters, numbers, and hyphens.</span>
        </label>
        {packaged && (
          <div className="mcp-form-pair">
            <label className="task-field">
              <span className="task-field__label">{kind === "mise" ? "Tool" : "Package"}</span>
              <input
                className="text-field mono"
                value={pkg}
                onChange={(event) => {
                  setPkg(event.target.value);
                  if (!commandTouched) setCommand(commandFor(event.target.value.trim()));
                }}
                placeholder={PACKAGE_HINT[kind]}
                spellCheck={false}
              />
            </label>
            <label className="task-field">
              <span className="task-field__label">Version</span>
              <input
                className="text-field mono"
                value={version}
                onChange={(event) => setVersion(event.target.value.trim())}
                placeholder="latest"
                spellCheck={false}
              />
            </label>
          </div>
        )}
        {kind !== "remote" ? (
          <>
            <label className="task-field" style={{ marginTop: 12 }}>
              <span className="task-field__label">Command</span>
              <input
                className="text-field mono"
                value={command}
                onChange={(event) => {
                  setCommand(event.target.value);
                  setCommandTouched(true);
                }}
                placeholder={packaged ? "server-github" : "docker"}
                spellCheck={false}
              />
              {packaged && <span className="field-hint">The program the package installs.</span>}
            </label>
            <label className="task-field" style={{ marginTop: 12 }}>
              <span className="task-field__label">Arguments</span>
              <input
                className="text-field mono"
                value={args}
                onChange={(event) => setArgs(event.target.value)}
                placeholder={packaged ? "Optional, separated by spaces" : "run -i --rm example/mcp"}
                spellCheck={false}
              />
            </label>
            <div className="task-field" style={{ marginTop: 12 }}>
              <span className="task-field__label">Environment variables</span>
              {env.map(([key, value], index) => (
                <div className="mcp-env-row" key={index}>
                  <input
                    className="text-field mono"
                    value={key}
                    aria-label="Variable name"
                    placeholder="LOG_LEVEL"
                    spellCheck={false}
                    onChange={(event) =>
                      setEnv(env.map((entry, at) => (at === index ? [event.target.value.trim(), entry[1]] : entry)))
                    }
                  />
                  <input
                    className="text-field mono"
                    value={value}
                    aria-label="Value"
                    placeholder="info"
                    spellCheck={false}
                    onChange={(event) =>
                      setEnv(env.map((entry, at) => (at === index ? [entry[0], event.target.value] : entry)))
                    }
                  />
                  <button
                    className="icon-button"
                    aria-label="Remove variable"
                    onClick={() => setEnv(env.filter((_, at) => at !== index))}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
              <div>
                <button className="button" onClick={() => setEnv([...env, ["", ""]])}>
                  <Plus size={13} /> Add Variable
                </button>
              </div>
              <span className="field-hint">
                Not for API keys or tokens: these are saved in plain files powermise keeps. Add keys in the agent’s own
                settings.
              </span>
            </div>
          </>
        ) : (
          <label className="task-field" style={{ marginTop: 12 }}>
            <span className="task-field__label">Address</span>
            <input
              className="text-field mono"
              value={url}
              onChange={(event) => setUrl(event.target.value.trim())}
              placeholder="https://mcp.example.com/mcp"
              spellCheck={false}
            />
          </label>
        )}
        <div className="task-field" style={{ marginTop: 12 }}>
          <span className="task-field__label">Turn on for</span>
          <div className="chip-row" role="group" aria-label="Agents">
            {agents.map((agent) => {
              const blocked = kind === "remote" && !agent.takesRemote;
              const on = clients.includes(agent.id) && !blocked;
              return (
                <button
                  key={agent.id}
                  type="button"
                  className={`chip-button${on ? " selected" : ""}`}
                  aria-pressed={on}
                  disabled={blocked}
                  onClick={() =>
                    setClients(on ? clients.filter((id) => id !== agent.id) : [...clients, agent.id])
                  }
                >
                  {agent.title}
                </button>
              );
            })}
          </div>
        </div>
        {error && (
          <div className="callout error selectable" style={{ marginTop: 12 }}>
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onCancel}>
          Cancel
        </button>
        <button className="button primary large" onClick={() => void save()} disabled={!ready || checking}>
          {checking && <Loader2 size={14} className="spin" />} {editing === "new" ? "Add" : "Save"}
        </button>
      </div>
    </Sheet>
  );
}
