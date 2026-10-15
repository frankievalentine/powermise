import { CheckCircle2, Folder, Loader2 } from "lucide-react";
import { api, type OutdatedTool } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { homeFrom, sourceLabel, upgradeTarget } from "../../mise";
import { Monogram, Switch } from "../../components/ui";

/** Global switches for automatic updates (mise 2026.10+), shown only when
 *  the installed mise has them. */
const AUTO_UPDATES: { key: string; title: string; description: string }[] = [
  {
    key: "tool_update.global_auto",
    title: "Keep global tools up to date",
    description: "mise upgrades the tools in your global config in the background, within the versions it asks for.",
  },
  {
    key: "self_update.auto",
    title: "Keep mise up to date",
    description: "mise updates itself in the background when a new release is out.",
  },
];

export function UpdatesView({ context }: { context: AppContext }) {
  const { mise, env, navigate, run } = context;
  const home = homeFrom(env.miseConfigDir);
  const entries = Object.values(mise.outdated ?? {}).sort((a, b) => a.name.localeCompare(b.name));
  const projectUpdates = useAsync(() => api.projectUpdates(), [mise.revision]);
  const settings = useAsync(() => api.settings(), [mise.revision]);
  const autoSettings = AUTO_UPDATES.flatMap((item) => {
    const setting = settings.data?.find((entry) => entry.key === item.key);
    return setting ? [{ ...item, on: setting.value === true }] : [];
  });
  const projects = projectUpdates.data ?? [];

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Updates</h1>
            <p className="page-subtitle">
              Newer versions of the tools you use. Upgrading installs the new version and updates
              the config file that asked for it.
            </p>
          </div>
          {entries.length > 1 && (
            <button
              className="button primary large"
              onClick={() =>
                run({
                  action: { kind: "upgrade", tool: null },
                  title: `Upgrade ${entries.length} global tools?`,
                  description: entries
                    .map((entry) => `${entry.name} → ${upgradeTarget(entry)}`)
                    .join(", "),
                  verb: "Upgrade All",
                })
              }
            >
              Upgrade All
            </button>
          )}
        </div>

        {!mise.outdated && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {mise.outdated && entries.length === 0 && projects.length === 0 && (
          <div className="empty">
            <CheckCircle2 size={28} color="var(--green)" />
            <h3>Everything is up to date</h3>
            <p>powermise checks again whenever you open this page.</p>
          </div>
        )}

        {entries.length > 0 && (projects.length > 0 || autoSettings.length > 0) && (
          <h2 className="section-title" style={{ marginTop: 6 }}>
            Global tools
          </h2>
        )}
        {entries.length > 0 && (
          <div className="list">
            {entries.map((entry) => {
              const target = upgradeTarget(entry);
              return (
                <div className="list-row" key={entry.name}>
                  <button
                    className="row-link"
                    onClick={() => navigate({ kind: "tool", name: entry.name })}
                    aria-label={`Open ${entry.name}`}
                  >
                    <Monogram id={entry.name} title={entry.name} size={30} />
                  </button>
                  <div className="row-main">
                    <div className="row-title">{entry.name}</div>
                    <div className="row-subtitle">
                      {entry.source
                        ? sourceLabel(
                            {
                              version: entry.current ?? "",
                              install_path: "",
                              installed: true,
                              active: true,
                              source: entry.source,
                            },
                            env.miseConfigDir,
                            home,
                          )
                        : "Installed"}
                      {entry.bump && entry.requested === entry.current && " · pinned — the config will be updated"}
                    </div>
                  </div>
                  <div className="row-trailing">
                    <span className="mono">{entry.current ?? "missing"}</span>
                    <span aria-hidden>→</span>
                    <span className="mono version-pill accent">{target}</span>
                    <button
                      className="button"
                      onClick={() =>
                        run({
                          action: { kind: "upgrade", tool: entry.name },
                          title: `Upgrade ${entry.name}?`,
                          description: `${entry.name} ${entry.current ?? ""} → ${target}.`,
                          verb: "Upgrade",
                        })
                      }
                    >
                      Upgrade
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        {projects.map((project) => (
          <section className="section" key={project.path}>
            <div className="section-heading-row">
              <h2 className="section-title">
                <Folder size={14} aria-hidden /> {project.name}
              </h2>
              {Object.keys(project.outdated).length > 1 && (
                <button
                  className="button"
                  onClick={() =>
                    run({
                      action: { kind: "upgradeProject", project: project.path, tool: null },
                      title: `Upgrade ${project.name}’s tools?`,
                      description: `mise upgrades what ${project.name}’s own config asks for and updates that config to match. Your global tools aren’t touched.`,
                      verb: "Upgrade All",
                    })
                  }
                >
                  Upgrade All
                </button>
              )}
            </div>
            <div className="list">
              {Object.values(project.outdated)
                .sort((a, b) => a.name.localeCompare(b.name))
                .map((entry) => (
                  <ProjectUpdateRow
                    key={entry.name}
                    entry={entry}
                    onUpgrade={() =>
                      run({
                        action: { kind: "upgradeProject", project: project.path, tool: entry.name },
                        title: `Upgrade ${entry.name} in ${project.name}?`,
                        description: `${entry.name} ${entry.current ?? ""} → ${upgradeTarget(entry)}, in ${project.name}’s config.`,
                        verb: "Upgrade",
                      })
                    }
                  />
                ))}
            </div>
          </section>
        ))}

        {settings.data && autoSettings.length === 0 && (
          <section className="section">
            <h2 className="section-title">Automatic updates</h2>
            <p className="section-note">
              Updating tools and mise in the background needs mise 2026.10 or newer.{" "}
              <button className="link-button" onClick={() => navigate({ kind: "health" })}>
                Update mise in Health
              </button>
            </p>
          </section>
        )}

        {autoSettings.length > 0 && (
          <section className="section">
            <h2 className="section-title">Automatic updates</h2>
            <div className="list prose">
              {autoSettings.map((item) => (
                <div className="list-row" key={item.key}>
                  <div className="row-main">
                    <div className="row-title">{item.title}</div>
                    <div className="row-subtitle">{item.description}</div>
                  </div>
                  <Switch
                    checked={item.on}
                    label={item.title}
                    onChange={(next) =>
                      run({
                        action: { kind: "setSetting", key: item.key, value: String(next) },
                        title: `${next ? "Turn on" : "Turn off"} “${item.title}”?`,
                        description: "mise writes this setting to your global config.",
                        verb: next ? "Turn On" : "Turn Off",
                      })
                    }
                  />
                </div>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function ProjectUpdateRow({ entry, onUpgrade }: { entry: OutdatedTool; onUpgrade: () => void }) {
  return (
    <div className="list-row">
      <Monogram id={entry.name} title={entry.name} size={30} />
      <div className="row-main">
        <div className="row-title">{entry.name}</div>
        {entry.bump && entry.requested === entry.current && (
          <div className="row-subtitle">Pinned — the project’s config will be updated</div>
        )}
      </div>
      <div className="row-trailing">
        <span className="mono">{entry.current ?? "missing"}</span>
        <span aria-hidden>→</span>
        <span className="mono version-pill accent">{upgradeTarget(entry)}</span>
        <button className="button" onClick={onUpgrade}>
          Upgrade
        </button>
      </div>
    </div>
  );
}
