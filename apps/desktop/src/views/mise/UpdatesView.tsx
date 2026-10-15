import { CheckCircle2, Loader2 } from "lucide-react";
import type { AppContext } from "../../App";
import { homeFrom, sourceLabel, upgradeTarget } from "../../mise";
import { Monogram } from "../../components/ui";

export function UpdatesView({ context }: { context: AppContext }) {
  const { mise, env, navigate, run } = context;
  const home = homeFrom(env.miseConfigDir);
  const entries = Object.values(mise.outdated ?? {}).sort((a, b) => a.name.localeCompare(b.name));

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
                  title: `Upgrade ${entries.length} tools?`,
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

        {mise.outdated && entries.length === 0 && (
          <div className="empty">
            <CheckCircle2 size={28} color="var(--green)" />
            <h3>Everything is up to date</h3>
            <p>Powermise checks again whenever you open this page.</p>
          </div>
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
      </div>
    </div>
  );
}
