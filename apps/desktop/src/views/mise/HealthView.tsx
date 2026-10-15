import { useState } from "react";
import { AlertTriangle, CheckCircle2, Info, Loader2, RefreshCw } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { activationCommand, healthSummary } from "../../health";
import { useAsync } from "../../hooks";
import { usePlatform } from "../../platform";
import { CleanupSheet, GithubCard, ImportList, formatBytes } from "../../components/machine";
import { CommandBlock, Section } from "../../components/ui";

/** The line that hooks mise into a shell, by shell name. */
export function HealthView({ context, section }: { context: AppContext; section?: "github" | "imports" }) {
  const { mise, run } = context;
  // Opened from Home for one section: bring it into view once it renders.
  const scrollTo = (element: HTMLDivElement | null) => element?.scrollIntoView({ block: "start" });
  const platform = usePlatform();
  const launchedFrom =
    platform === "darwin" ? "the Dock or Finder" : platform === "win32" ? "the Start menu" : "your app launcher";
  // Shared with the sidebar badge, which reloads it after every change.
  const [checking, setChecking] = useState(false);
  const doctor = {
    data: mise.doctor,
    error: mise.doctorError,
    loading: checking || (!mise.doctor && !mise.doctorError),
    reload: async () => {
      setChecking(true);
      await mise.reloadDoctor();
      setChecking(false);
    },
  };
  const info = mise.info;
  const problems = [...(doctor.data?.errors ?? []), ...(doctor.data?.warnings ?? [])];
  const shell = activationCommand(doctor.data?.shell?.name ?? "zsh");
  const summary = healthSummary(info, doctor.data);
  const imports = useAsync(() => api.detectImports(), [mise.revision]);
  const prunable = useAsync(() => api.prunableVersions(), [mise.revision]);
  const disk = useAsync(() => api.diskUsage(), [mise.revision]);
  const trust = useAsync(() => api.trustedFolders(), [mise.revision]);
  const plugins = useAsync(() => api.plugins(), [mise.revision]);
  const pretty = (path: string) => path.replace(/^\/(Users|home)\/[^/]+/, "~");
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const healthy = !!doctor.data && summary.count === 0;

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Health</h1>
            <p className="page-subtitle">How mise is set up on this computer, and anything that needs attention.</p>
          </div>
          <button className="button large" onClick={doctor.reload} disabled={doctor.loading}>
            {doctor.loading ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} Check Again
          </button>
        </div>

        {doctor.error && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{doctor.error}</span>
          </div>
        )}

        {doctor.data && (
          <div className={`banner${healthy ? "" : " warn"}`}>
            {healthy ? <CheckCircle2 size={16} color="var(--green)" /> : <AlertTriangle size={16} />}
            <span>
              {healthy
                ? "mise is installed, up to date enough, and hooked into your shell."
                : `${summary.count} ${summary.count === 1 ? "thing needs" : "things need"} attention.`}
            </span>
          </div>
        )}

        {doctor.data && !healthy && (
          <Section title="Needs attention">
            <div className="list prose">
              {info && !info.recommendedMet && (
                <div className="list-row">
                  <AlertTriangle size={16} color={info.supported ? "var(--orange)" : "var(--red)"} />
                  <div className="row-main">
                    <div className="row-title">
                      {info.supported ? "A newer mise is available" : "mise is too old for powermise"}
                    </div>
                    <div className="row-subtitle">
                      {info.supported
                        ? `You have ${info.version?.split(" ")[0]}. Updating to ${info.recommended} or newer turns on AI Setups, macOS settings, automatic updates, and more.`
                        : `powermise needs mise ${info.minimum} or newer.`}
                    </div>
                  </div>
                  <button
                    className="button primary"
                    onClick={() =>
                      run({
                        action: { kind: "selfUpdate" },
                        title: `Update mise${info.latest ? ` to ${info.latest}` : ""}?`,
                        description: "mise downloads its newest release and replaces itself. Your tools and settings stay as they are.",
                        verb: "Update mise",
                      })
                    }
                  >
                    Update mise
                  </button>
                </div>
              )}
              {!doctor.data.activated && (
                <div className="list-row setting-row">
                  <AlertTriangle size={16} color="var(--orange)" />
                  <div className="row-main">
                    <div className="row-title">mise isn’t turned on in your terminal</div>
                    <div className="row-subtitle">
                      Terminals won’t switch tool versions as you move between projects. Run this once, then open a
                      new terminal window (powermise doesn’t edit your shell files for you):
                    </div>
                    <div style={{ marginTop: 8 }}>
                      <CommandBlock command={`echo '${shell.line}' >> ${shell.file}`} />
                    </div>
                  </div>
                </div>
              )}
              {problems.map((problem) => (
                <div className="list-row" key={problem}>
                  <AlertTriangle size={16} color="var(--orange)" />
                  <div className="row-main">
                    <div className="row-title">mise reported a problem</div>
                    <div className="row-subtitle selectable">{problem}</div>
                  </div>
                </div>
              ))}
            </div>
          </Section>
        )}

        <details className="more-details" open={!!section || undefined}>
          <summary>More details</summary>

          <Section title="mise">
            <div className="list">
              <div className="list-row">
                <div className="row-main">
                  <div className="row-title">Version</div>
                  <div className="row-subtitle selectable">{info?.version ?? "Not found"}</div>
                </div>
                <div className="row-trailing">
                  {!info?.supported ? (
                    <span className="badge red">Needs {info?.minimum} or newer</span>
                  ) : info.recommendedMet ? (
                    <span className="badge green">Up to date</span>
                  ) : (
                    <span
                      className="badge orange"
                      title={`powermise works with this version. Some features need ${info.recommended} or newer.`}
                    >
                      Update recommended
                    </span>
                  )}
                  {doctor.data?.self_update_available && (
                    <button
                      className="button"
                      onClick={() =>
                        run({
                          action: { kind: "selfUpdate" },
                          title: `Update mise${info?.latest ? ` to ${info.latest}` : ""}?`,
                          description: "mise downloads its newest release and replaces itself. Your tools and settings stay as they are.",
                          verb: "Update mise",
                        })
                      }
                    >
                      Update{info?.latest ? ` to ${info.latest}` : ""}
                    </button>
                  )}
                </div>
              </div>
              {info?.supported && !info.recommendedMet && (
                <div className="list-row">
                  <Info size={15} color="var(--orange)" />
                  <div className="row-main">
                    <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                      powermise works with your mise, but is designed around {info.recommended} or newer.
                      Updating turns on project-vs-global updates, automatic updates, and more.
                    </div>
                  </div>
                </div>
              )}
              <div className="list-row">
                <div className="row-main">
                  <div className="row-title">Location</div>
                  <div className="row-subtitle selectable mono">{info?.path ?? "—"}</div>
                </div>
              </div>
            </div>
          </Section>

          {doctor.data && (
            <Section title="Your shell">
              <div className="list">
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Activated in your shell</div>
                    <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                      {doctor.data.activated
                        ? "Terminals switch tool versions automatically as you move between projects."
                        : "Without this, terminals won’t pick up the tools mise installs."}
                    </div>
                  </div>
                  <div className="row-trailing">
                    {doctor.data.activated ? (
                      <span className="badge green">Yes</span>
                    ) : (
                      <span className="badge orange">No</span>
                    )}
                  </div>
                </div>
              </div>
              {doctor.data.shims_on_path && (
                <div className="list prose" style={{ marginTop: 10 }}>
                  <div className="list-row">
                    <div className="row-main">
                      <div className="row-title">Shims are on your PATH</div>
                      <div className="row-subtitle">
                        {doctor.data.activated
                          ? "Shims are small stand-in commands that call the right version. With mise activated you don’t need them, but they help editors and scripts that don’t run your shell."
                          : "Shims are small stand-in commands that call the right version, so tools work even where mise isn’t activated — editors, scripts, and apps. If a tool’s command is missing, rebuild them."}
                      </div>
                    </div>
                    <button
                      className="button"
                      onClick={() =>
                        run({
                          action: { kind: "reshim" },
                          title: "Rebuild shims?",
                          description: "mise recreates a shim for every command your installed tools provide.",
                          verb: "Rebuild",
                        })
                      }
                    >
                      Rebuild Shims
                    </button>
                  </div>
                </div>
              )}
              {!doctor.data.activated && (
                <div style={{ marginTop: 10 }}>
                  <p className="section-note" style={{ margin: "0 0 8px" }}>
                    Add this line to {shell.file}, then open a new terminal. powermise doesn’t edit your
                    shell files for you.
                  </p>
                  <CommandBlock command={`echo '${shell.line}' >> ${shell.file}`} />
                </div>
              )}
              <div className="callout info" style={{ marginTop: 10 }}>
                <Info size={14} />
                <span>
                  powermise can’t see settings exported in your shell files (like <code>MISE_*</code>{" "}
                  variables), because apps opened from {launchedFrom} don’t load them. Put mise settings in{" "}
                  <code>~/.config/mise/config.toml</code> to share them with powermise.
                </span>
              </div>
            </Section>
          )}


          {doctor.data && doctor.data.config_files.length > 0 && (
            <Section title="Config files mise reads">
              <div className="list">
                {doctor.data.config_files.map((file) => (
                  <div className="list-row" key={file}>
                    <div className="row-main">
                      <div className="row-title mono selectable">{file}</div>
                    </div>
                    <button className="button" onClick={() => api.reveal(file)}>
                      Show
                    </button>
                  </div>
                ))}
              </div>
            </Section>
          )}

          <div id="health-github" ref={section === "github" ? scrollTo : undefined} />
          <Section title="GitHub downloads">
            <GithubCard context={context} />
          </Section>

          <div id="health-imports" ref={section === "imports" ? scrollTo : undefined} />
          {imports.data && imports.data.length > 0 && (
            <Section
              title="Versions from other tools"
              note="Installed by another version manager. mise can use them without downloading them again."
            >
              <ImportList context={context} imports={imports.data} />
            </Section>
          )}

          {trust.data && (trust.data.trusted.length > 0 || trust.data.ignored.length > 0) && (
            <Section
              title="Trusted folders"
              note="mise runs hooks, tasks, and environment from these folders’ configs. Revoke trust for any you no longer use."
            >
              <div className="list">
                {trust.data.trusted.map((entry) => (
                  <div className="list-row" key={entry.path}>
                    <div className="row-main">
                      <div className="row-title mono selectable">{pretty(entry.path)}</div>
                      {!entry.exists && <div className="row-subtitle">No longer exists</div>}
                    </div>
                    <button
                      className="button"
                      onClick={() =>
                        run({
                          action: { kind: "untrust", path: entry.path },
                          title: `Stop trusting ${pretty(entry.path)}?`,
                          description:
                            "mise stops loading this config’s hooks, tasks, and environment until you trust it again.",
                          verb: "Revoke",
                          destructive: true,
                        })
                      }
                    >
                      Revoke
                    </button>
                  </div>
                ))}
                {trust.data.ignored.map((path) => (
                  <div className="list-row" key={`ignored:${path}`}>
                    <div className="row-main">
                      <div className="row-title mono selectable">{pretty(path)}</div>
                      <div className="row-subtitle">Ignored — mise skips this config entirely</div>
                    </div>
                  </div>
                ))}
              </div>
            </Section>
          )}

          {plugins.data && plugins.data.length > 0 && (
            <Section
              title="Plugins"
              note="Legacy asdf and vfox plugins. Most tools no longer need one; mise’s registry and backends cover them."
            >
              <div className="list">
                {plugins.data.map((plugin) => (
                  <div className="list-row" key={plugin.name}>
                    <div className="row-main">
                      <div className="row-title mono">{plugin.name}</div>
                      {plugin.url && <div className="row-subtitle mono selectable">{plugin.url}</div>}
                    </div>
                    {plugin.revision && <span className="row-subtitle mono">{plugin.revision}</span>}
                  </div>
                ))}
              </div>
            </Section>
          )}

          <Section title="Disk space">
            <div className="list">
              {disk.data && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Installed tools</div>
                    <div className="row-subtitle mono">
                      {formatBytes(disk.data.installsBytes)} · {pretty(disk.data.installsDir)}
                    </div>
                  </div>
                  <button className="button" onClick={() => api.reveal(disk.data!.installsDir)}>
                    Show
                  </button>
                </div>
              )}
              {disk.data && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Download cache</div>
                    <div className="row-subtitle mono">
                      {formatBytes(disk.data.cacheBytes)} · {pretty(disk.data.cacheDir)}
                    </div>
                  </div>
                  <button
                    className="button"
                    disabled={!disk.data.cacheBytes}
                    onClick={() =>
                      run({
                        action: { kind: "cachePrune" },
                        title: "Remove old cache files?",
                        description: "mise deletes cached downloads and version lists it hasn’t used recently.",
                        verb: "Remove",
                      })
                    }
                  >
                    Remove Old
                  </button>
                  <button
                    className="button"
                    disabled={!disk.data.cacheBytes}
                    onClick={() =>
                      run({
                        action: { kind: "cacheClear" },
                        title: "Clear the whole cache?",
                        description:
                          "mise deletes every cached download and version list. Installed tools aren’t affected; mise downloads again when it needs to.",
                        verb: "Clear Cache",
                        destructive: true,
                      })
                    }
                  >
                    Clear All
                  </button>
                </div>
              )}
              <div className="list-row">
                <div className="row-main">
                  <div className="row-title">Unused tool versions</div>
                  <div className="row-subtitle">
                    {prunable.data
                      ? prunable.data.length
                        ? `${prunable.data.length} ${prunable.data.length === 1 ? "version" : "versions"} no config uses · ${formatBytes(prunable.data.reduce((sum, entry) => sum + entry.bytes, 0))}`
                        : "Every installed version is in use."
                      : "Checking…"}
                  </div>
                </div>
                <button className="button" onClick={() => setCleanupOpen(true)} disabled={!prunable.data?.length}>
                  Clean Up…
                </button>
              </div>
            </div>
          </Section>

          <Section title="In the terminal">
            <CommandBlock command="mise doctor" />
          </Section>

          <Section title="About powermise">
            <p className="section-note" style={{ margin: 0, maxWidth: 620 }}>
              powermise is an unofficial, independent companion for mise. It isn’t made by, affiliated with,
              or endorsed by the mise project. If something only goes wrong in powermise, report it on the
              powermise GitHub repository rather than to mise.
            </p>
          </Section>
        </details>
      </div>
      <CleanupSheet open={cleanupOpen} onDismiss={() => setCleanupOpen(false)} context={context} />
    </div>
  );
}
