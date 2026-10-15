import { useMemo, useState } from "react";
import { AlertTriangle, Loader2, SquareTerminal } from "lucide-react";
import { api, type AgentCli, type AgentPlugin } from "../api";
import type { AppContext } from "../App";
import { AGENT_CLI_TITLE } from "../catalog";
import { useAsync } from "../hooks";
import { AgentsTabs } from "../components/SectionTabs";
import { Monogram, Section } from "../components/ui";

const AGENTS: AgentCli[] = ["claude", "codex", "gemini"];
/** Marketplace results shown before the search narrows them. */
const SHOWN = 60;

/** Plugins and extensions for each agent, installed and removed with the
 *  agent's own CLI, so they show up in the agent exactly as if you'd run it. */
export function PluginsView({ context, query }: { context: AppContext; query: string }) {
  const { catalog, mise, run } = context;
  const [agent, setAgent] = useState<AgentCli>("claude");
  const [source, setSource] = useState("");
  const report = useAsync(() => api.agentPlugins(agent), [agent, mise.revision]);
  const needle = query.trim().toLowerCase();
  const matches = (plugin: { name: string; description?: string | null }) =>
    !needle || plugin.name.toLowerCase().includes(needle) || (plugin.description ?? "").toLowerCase().includes(needle);

  const installed = report.data?.installed ?? [];
  const installedIds = new Set(installed.map((plugin) => plugin.id));
  const picks = catalog.integrations.filter((entry) => entry.agent === agent);
  const available = useMemo(
    () => (report.data?.available ?? []).filter(matches).slice(0, SHOWN),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [report.data, needle],
  );
  const title = AGENT_CLI_TITLE[agent];

  const install = (id: string, name: string, signsIn = false) => {
    if (signsIn) {
      void api.installPluginInTerminal(agent, id);
      return;
    }
    run({
      action: { kind: "agentPlugin", agent, id, remove: false },
      title: `Install ${name}?`,
      description:
        agent === "gemini"
          ? `${title} downloads the extension from ${id} and installs it for you. Extensions can run code, so only add ones you trust.`
          : `${title} installs the plugin from its marketplace for your user account.`,
      verb: "Install",
    });
  };
  const remove = (plugin: AgentPlugin) =>
    run({
      action: { kind: "agentPlugin", agent, id: plugin.id, remove: true },
      title: `Remove ${plugin.name}?`,
      description: `${title} uninstalls it. You can add it again any time.`,
      verb: "Remove",
      destructive: true,
    });
  const validSource = /^https:\/\/\S+$/.test(source.trim());

  return (
    <div className="content">
      <div className="page">
        <AgentsTabs context={context} current="integrations" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Agents</h1>
            <p className="page-subtitle">
              Plugins and extensions add commands, skills, and connections to an agent. powermise installs them with the
              agent’s own command, so they work exactly as if you’d added them yourself.
            </p>
          </div>
        </div>

        <div className="chip-row discover-chips" role="group" aria-label="Agent">
          {AGENTS.map((value) => (
            <button
              key={value}
              type="button"
              className={`chip-button${agent === value ? " selected" : ""}`}
              aria-pressed={agent === value}
              onClick={() => setAgent(value)}
            >
              {AGENT_CLI_TITLE[value]}
            </button>
          ))}
        </div>

        {report.loading && !report.data && (
          <p className="field-hint">
            <Loader2 size={12} className="spin" /> Asking {title}…
          </p>
        )}
        {report.data?.error && (
          <div className="callout warn">
            <AlertTriangle size={14} />
            <span>
              {report.data.error}. Add {title} with a setup in AI Agents, then come back here.
            </span>
          </div>
        )}

        {!report.data?.error && report.data && (
          <Section title="Installed" note={installed.length ? undefined : `No ${agent === "gemini" ? "extensions" : "plugins"} yet.`}>
            {installed.length > 0 && (
              <div className="list">
                {installed.map((plugin) => (
                  <PluginRow key={plugin.id} plugin={plugin}>
                    <button className="button" onClick={() => remove(plugin)}>
                      Remove…
                    </button>
                  </PluginRow>
                ))}
              </div>
            )}
          </Section>
        )}

        {picks.length > 0 && (
          <Section title="Suggested" note="These can also be part of a setup, so they install with it.">
            <div className="list">
              {picks.map((entry) => {
                const done = installedIds.has(entry.installed_name ?? entry.plugin);
                return (
                  <PluginRow
                    key={entry.id}
                    plugin={{ id: entry.plugin, name: entry.title, description: entry.description, installed: done }}
                  >
                    {done ? (
                      <span className="badge green">Installed</span>
                    ) : (
                      <button
                        className="button"
                        disabled={!!report.data?.error}
                        onClick={() => install(entry.plugin, entry.title, agent === "codex")}
                      >
                        {agent === "codex" && <SquareTerminal size={13} />} Install
                      </button>
                    )}
                  </PluginRow>
                );
              })}
            </div>
          </Section>
        )}

        {agent === "gemini" ? (
          <Section
            title="Add an extension"
            note="Paste a GitHub link to a Gemini CLI extension. Browse them at geminicli.com/extensions."
          >
            <div className="install-row">
              <input
                className="text-field mono"
                value={source}
                onChange={(event) => setSource(event.target.value)}
                placeholder="https://github.com/gemini-cli-extensions/security"
                aria-label="Extension link"
                spellCheck={false}
              />
              <button
                className="button primary"
                disabled={!validSource || !!report.data?.error}
                onClick={() => install(source.trim(), source.trim().split("/").pop() ?? "extension")}
              >
                Install
              </button>
            </div>
          </Section>
        ) : (
          report.data &&
          !report.data.error && (
            <Section
              title={needle ? `Results for “${query}”` : `Browse ${title} plugins`}
              note={`${(report.data.available.length ?? 0).toLocaleString()} in ${title}’s marketplaces${
                needle ? "" : `; search to find one`
              }.${agent === "codex" ? " Most sign in to a service as they install, so they open in your terminal." : ""}`}
            >
              <div className="list">
                {available.map((plugin) => (
                  <PluginRow key={plugin.id} plugin={plugin}>
                    <button className="button" onClick={() => install(plugin.id, plugin.name, plugin.signsInOnInstall)}>
                      {plugin.signsInOnInstall && <SquareTerminal size={13} />} Install
                    </button>
                  </PluginRow>
                ))}
              </div>
            </Section>
          )
        )}
      </div>
    </div>
  );
}

function PluginRow({
  plugin,
  children,
}: {
  plugin: Pick<AgentPlugin, "id" | "name" | "description"> & Partial<AgentPlugin>;
  children: React.ReactNode;
}) {
  return (
    <div className="list-row">
      <Monogram id={plugin.id} title={plugin.name} size={28} />
      <div className="row-main">
        <div className="row-title">
          {plugin.name}
          {plugin.installed && plugin.enabled === false && <span className="row-subtitle"> · turned off</span>}
        </div>
        <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
          {plugin.description ?? plugin.marketplace ?? plugin.id}
          {plugin.version ? ` · ${plugin.version}` : ""}
        </div>
      </div>
      <div className="row-trailing">{children}</div>
    </div>
  );
}
