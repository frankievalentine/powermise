import { useMemo, useState } from "react";
import { ArrowUpCircle, Compass, Loader2, Trash2 } from "lucide-react";
import type { AppContext } from "../../App";
import { homeFrom, isGlobalPath, sourceLabel, upgradeTarget } from "../../mise";
import { Monogram } from "../../components/ui";
import { CleanupSheet } from "../../components/machine";
import { MyToolsTabs } from "../../components/SectionTabs";

export function InstalledView({ context, query }: { context: AppContext; query: string }) {
  const { mise, env, navigate } = context;
  const home = homeFrom(env);

  const tools = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return Object.entries(mise.inventory ?? {})
      .filter(([name]) => !needle || name.toLowerCase().includes(needle))
      .map(([name, versions]) => {
        const global = versions.find(
          (version) => version.source && isGlobalPath(version.source.path, env.miseConfigDir),
        );
        const active = versions.find((version) => version.active) ?? global;
        return { name, versions, global, active };
      });
  }, [mise.inventory, query, env.miseConfigDir]);

  const inUse = tools.filter((tool) => tool.versions.some((version) => version.source));
  const unused = tools.filter((tool) => !tool.versions.some((version) => version.source));
  const outdatedCount = Object.keys(mise.outdated ?? {}).length;
  const [cleanupOpen, setCleanupOpen] = useState(false);

  const row = (tool: (typeof tools)[number]) => {
    const outdated = mise.outdated?.[tool.name];
    const description = mise.registryByName.get(tool.name)?.description;
    const shown = tool.active ?? tool.versions[tool.versions.length - 1];
    const missing = tool.versions.some((version) => version.source && !version.installed);
    return (
      <button
        key={tool.name}
        className="list-row interactive"
        onClick={() => navigate({ kind: "tool", name: tool.name })}
      >
        <Monogram id={tool.name} title={tool.name} size={30} />
        <div className="row-main">
          <div className="row-title">{tool.name}</div>
          <div className="row-subtitle">{description ?? sourceLabel(shown, env.miseConfigDir, home)}</div>
        </div>
        <div className="row-trailing">
          {missing && <span className="badge orange">Not installed</span>}
          {outdated && (
            <span className="badge accent" title={`Update available: ${upgradeTarget(outdated)}`}>
              <ArrowUpCircle size={11} /> {upgradeTarget(outdated)}
            </span>
          )}
          <span className="badge">{sourceLabel(shown, env.miseConfigDir, home)}</span>
          <span className="mono version-pill">{shown.version}</span>
          {tool.versions.length > 1 && (
            <span className="row-subtitle">+{tool.versions.length - 1}</span>
          )}
        </div>
      </button>
    );
  };

  return (
    <div className="content">
      <div className="page">
        <MyToolsTabs context={context} current="installed" />
        <div className="page-header">
          <div>
            <h1 className="page-title">My Tools</h1>
            <p className="page-subtitle">
              Every tool mise manages on this computer, the version you use, and where it’s set.
            </p>
          </div>
          <div className="detail-actions">
            {mise.inventory && (
              <button className="button large" onClick={() => setCleanupOpen(true)}>
                <Trash2 size={14} /> Clean Up…
              </button>
            )}
            {outdatedCount > 0 && (
              <button className="button large" onClick={() => navigate({ kind: "updates" })}>
                <ArrowUpCircle size={15} /> {outdatedCount} {outdatedCount === 1 ? "update" : "updates"}
              </button>
            )}
          </div>
        </div>

        {mise.error && <div className="callout error selectable">{mise.error}</div>}

        {!mise.inventory && !mise.error && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {mise.inventory && tools.length === 0 && (
          <div className="empty">
            <Compass size={28} color="var(--accent)" />
            <h3>{query ? "No matches" : "No tools yet"}</h3>
            <p>{query ? "Try a different search." : "Find something to install in Discover."}</p>
            {!query && (
              <button className="button primary large" onClick={() => navigate({ kind: "discover" })}>
                Open Discover
              </button>
            )}
          </div>
        )}

        {inUse.length > 0 && (
          <section className="section">
            <h2 className="section-title">In use</h2>
            <div className="list">{inUse.map(row)}</div>
          </section>
        )}
        {unused.length > 0 && (
          <section className="section">
            <h2 className="section-title">Installed, not in use</h2>
            <p className="section-note">
              Older versions no config file asks for. Remove them to free up space.
            </p>
            <div className="list">{unused.map(row)}</div>
          </section>
        )}
      </div>
      <CleanupSheet open={cleanupOpen} onDismiss={() => setCleanupOpen(false)} context={context} />
    </div>
  );
}
