import { useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Eye,
  GitCompare,
  History,
  Loader2,
  MonitorCog,
  RefreshCw,
  RotateCcw,
  Save,
  Undo2,
} from "lucide-react";
import { api, type MachineStatus } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { ExternalLinkButton } from "../../components/machine";
import { Section } from "../../components/ui";

/** Titles for `mise bootstrap status` parts, and the `--only` name that
 *  applies just that part (where there is one). */
const PARTS: Record<string, { title: string; only?: string }> = {
  packages: { title: "System packages", only: "packages" },
  tools: { title: "Tools", only: "tools" },
  dotfiles: { title: "Dotfiles", only: "dotfiles" },
  repos: { title: "Git repositories", only: "repos" },
  files: { title: "Files", only: "files" },
  services: { title: "Services", only: "services" },
  user_services: { title: "User services", only: "services" },
  accounts: { title: "Users and groups", only: "accounts" },
  firewall: { title: "Firewall", only: "firewall" },
  compose: { title: "Docker Compose", only: "compose" },
  mise_shell_activate: { title: "Shell activation", only: "mise-shell-activate" },
  macos_defaults: { title: "macOS settings", only: "macos-defaults" },
  macos_login_items: { title: "Login items" },
  launchd: { title: "LaunchAgents", only: "macos-launchd-agents" },
  systemd: { title: "systemd units", only: "linux-systemd-units" },
  login_shell: { title: "Login shell", only: "user" },
  plugin_deps: { title: "Plugin dependencies", only: "plugins" },
  secrets: { title: "Secrets" },
};

const GOOD = [
  "installed",
  "applied",
  "present",
  "set",
  "ok",
  "linked",
  "current",
  "up-to-date",
  "up_to_date",
  "configured",
  "running",
  "enabled",
  "true",
];

const titleOf = (id: string) => PARTS[id]?.title ?? id.replace(/_/g, " ").replace(/^\w/, (letter) => letter.toUpperCase());

/** Machine setup: what the global config's `[bootstrap]` and `[dotfiles]`
 *  set up on this computer, previewed and applied with `mise bootstrap`. */
export function MachineView({ context }: { context: AppContext }) {
  const { mise, run } = context;
  const status = useAsync(() => api.machineStatus(), [mise.revision]);
  const data = status.data;

  const preview = (only: string | null, title: string) =>
    run({
      action: { kind: "bootstrapPreview", only },
      title: `Preview ${title}?`,
      description: "mise lists what it would change. Nothing on this computer changes.",
      verb: "Preview",
    });
  const apply = (only: string | null, title: string) =>
    run({
      action: { kind: "bootstrapApply", only },
      title: `Apply ${title}?`,
      description:
        "mise sets this computer up as your config describes: it can install packages, link dotfiles, and change settings. Preview first if you’re not sure. Some steps may need an administrator password and are best run from a terminal.",
      verb: "Apply",
    });

  const pending = (data?.sections ?? []).reduce(
    (count, section) => count + section.items.filter((item) => item.state && !GOOD.includes(item.state)).length,
    0,
  );

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Machine</h1>
            <p className="page-subtitle">
              What your mise config sets up on this computer beyond tools — system packages, dotfiles, repositories,
              and settings — from its <code>[bootstrap]</code> and <code>[dotfiles]</code> sections.{" "}
              <ExternalLinkButton url="https://mise.jdx.dev/cli/bootstrap.html">About bootstrap</ExternalLinkButton>
            </p>
          </div>
          {data && data.sections.length > 0 && (
            <div className="detail-actions">
              <button className="button large" onClick={() => preview(null, "everything")}>
                <Eye size={14} /> Preview
              </button>
              <button className="button primary large" onClick={() => apply(null, "everything")}>
                Apply All…
              </button>
            </div>
          )}
        </div>

        {context.env.hostTarget === "darwin" && (
          <button className="list-row interactive install-spec macos-entry" onClick={() => context.navigate({ kind: "macos-settings" })}>
            <MonitorCog size={18} />
            <div className="row-main">
              <div className="row-title">macOS Settings</div>
              <div className="row-subtitle">
                Dock, Finder, keyboard, trackpad, screenshots, and more — pick settings and mise keeps this Mac that way.
              </div>
            </div>
            <ChevronRight size={16} />
          </button>
        )}

        {status.error && <div className="callout error selectable">{status.error}</div>}
        {status.loading && !data && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {data && data.sections.length === 0 && (
          <div className="empty">
            <MonitorCog size={28} color="var(--accent)" />
            <h3>Nothing to set up yet</h3>
            <p>
              Your mise config has no <code>[bootstrap]</code> or <code>[dotfiles]</code> sections. AI Setups add system
              packages here.
            </p>
          </div>
        )}

        {data && data.sections.length > 0 && (
          <div className={`banner${pending ? " warn" : ""}`}>
            {pending ? <CircleAlert size={16} /> : <CheckCircle2 size={16} color="var(--green)" />}
            <span>
              {pending
                ? `${pending} ${pending === 1 ? "thing isn’t" : "things aren’t"} set up yet.`
                : "Everything your config describes is in place."}
            </span>
          </div>
        )}

        {data?.sections.map((section) => {
          const part = PARTS[section.id];
          const notReady = section.items.filter((item) => item.state && !GOOD.includes(item.state)).length;
          return (
            <section className="section" key={section.id}>
              <div className="section-heading-row">
                <h2 className="section-title">
                  {titleOf(section.id)}
                  {notReady > 0 && <span className="badge orange">{notReady} to apply</span>}
                </h2>
                {part?.only && (
                  <div className="detail-actions">
                    <button className="button" onClick={() => preview(part.only!, titleOf(section.id).toLowerCase())}>
                      Preview
                    </button>
                    {notReady > 0 && (
                      <button className="button" onClick={() => apply(part.only!, titleOf(section.id).toLowerCase())}>
                        Apply…
                      </button>
                    )}
                  </div>
                )}
              </div>
              <div className="list">
                {section.items.map((item, index) => (
                  <div className="list-row" key={`${item.name}-${index}`}>
                    <div className="row-main">
                      <div className="row-title mono selectable">{item.name}</div>
                      {item.detail && <div className="row-subtitle">{item.detail}</div>}
                    </div>
                    {item.state && (
                      <span className={`badge ${GOOD.includes(item.state) ? "green" : "orange"}`}>{item.state}</span>
                    )}
                  </div>
                ))}
              </div>
            </section>
          );
        })}

        {data && <DotfilesHistory data={data} context={context} />}
      </div>
    </div>
  );
}

function DotfilesHistory({ data, context }: { data: MachineStatus; context: AppContext }) {
  const [url, setUrl] = useState("");
  if (!data.supportsHistory) {
    return (
      <Section title="Dotfile history">
        <p className="section-note">
          Saving checkpoints of your dotfiles and syncing them between computers needs mise 2026.10 or newer. Update
          mise from Health.
        </p>
      </Section>
    );
  }
  const history = data.history;
  if (!history) return null;
  const validUrl = /^(https:\/\/|git@|ssh:\/\/)\S+$/.test(url);
  return (
    <Section
      title="Dotfile history"
      note="mise keeps checkpoints of the files it tracks, so you can roll back — and sync them to your other computers through a Git repository."
    >
      <div className="list prose">
        <div className="list-row">
          <History size={16} color="var(--text-tertiary)" />
          <div className="row-main">
            <div className="row-title">
              {history.checkpoints} {history.checkpoints === 1 ? "checkpoint" : "checkpoints"} · {history.trackedFiles}{" "}
              tracked {history.trackedFiles === 1 ? "file" : "files"}
            </div>
            <div className="row-subtitle">
              {history.unsaved ? "There are unsaved changes." : "Nothing changed since the latest checkpoint."}
              {history.pendingOperations > 0 && ` ${history.pendingOperations} interrupted operation(s) to recover.`}
            </div>
          </div>
          <button
            className="button"
            onClick={() =>
              context.run({
                action: { kind: "dotfilesSave" },
                title: "Save a checkpoint?",
                description: "mise saves the current version of every tracked file.",
                verb: "Save",
              })
            }
          >
            <Save size={13} /> Save Checkpoint
          </button>
        </div>
        <div className="list-row">
          <RefreshCw size={16} color="var(--text-tertiary)" />
          <div className="row-main">
            <div className="row-title">{data.origin ? "Syncing with" : "Not syncing"}</div>
            <div className="row-subtitle mono selectable">
              {data.origin ?? "Connect a Git repository to share checkpoints with your other computers."}
            </div>
          </div>
          {data.origin ? (
            <button
              className="button"
              onClick={() =>
                context.run({
                  action: { kind: "dotfilesSync" },
                  title: "Sync dotfiles?",
                  description: "mise pushes your checkpoints and fetches changes from your other computers.",
                  verb: "Sync",
                })
              }
            >
              Sync
            </button>
          ) : (
            <form
              className="setting-edit"
              onSubmit={(event) => {
                event.preventDefault();
                if (!validUrl) return;
                context.run({
                  action: { kind: "dotfilesOriginSet", url },
                  title: "Connect this repository?",
                  description: "mise records it as the origin your dotfile checkpoints sync with.",
                  verb: "Connect",
                });
              }}
            >
              <input
                className="text-field mono"
                value={url}
                onChange={(event) => setUrl(event.target.value.trim())}
                placeholder="git@github.com:you/dotfiles.git"
                aria-label="Repository URL"
                spellCheck={false}
              />
              <button className="button" type="submit" disabled={!validUrl}>
                Connect…
              </button>
            </form>
          )}
        </div>
      </div>

      {data.checkpoints.length > 0 && (
        <>
          <div className="section-heading-row" style={{ marginTop: 18 }}>
            <h3 className="section-title" style={{ fontSize: 13 }}>
              Recent checkpoints
            </h3>
            <div className="detail-actions">
              <button
                className="button"
                onClick={() =>
                  context.run({
                    action: { kind: "dotfilesUndo" },
                    title: "Undo the latest change?",
                    description: "mise reverses the most recent operation on your tracked files.",
                    verb: "Undo",
                  })
                }
              >
                <Undo2 size={13} /> Undo Latest
              </button>
              <button
                className="button"
                onClick={() =>
                  void api.resolveDotfileConflicts().catch((reason: Error) => window.alert(reason.message))
                }
                title="Opens mise dotfiles conflicts in your terminal app"
              >
                <GitCompare size={13} /> Resolve Conflicts
              </button>
            </div>
          </div>
          <div className="list">
            {data.checkpoints.map((checkpoint) => (
              <div className="list-row" key={checkpoint.id}>
                <span className="config-rank mono">{checkpoint.id}</span>
                <div className="row-main">
                  <div className="row-title">{checkpoint.summary ?? checkpoint.trigger ?? `Checkpoint ${checkpoint.id}`}</div>
                  <div className="row-subtitle">
                    {[
                      checkpoint.createdAt && new Date(checkpoint.createdAt).toLocaleString(),
                      checkpoint.trigger,
                      checkpoint.machine && `on ${checkpoint.machine}`,
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </div>
                </div>
                <button
                  className="button"
                  onClick={() =>
                    context.run({
                      action: { kind: "dotfilesRollback", checkpoint: checkpoint.id, dryRun: true },
                      title: `Preview rolling back to checkpoint ${checkpoint.id}?`,
                      description: "mise lists what would change. Nothing changes yet.",
                      verb: "Preview",
                    })
                  }
                >
                  Preview
                </button>
                <button
                  className="button"
                  onClick={() =>
                    context.run({
                      action: { kind: "dotfilesRollback", checkpoint: checkpoint.id, dryRun: false },
                      title: `Roll back to checkpoint ${checkpoint.id}?`,
                      description:
                        "mise returns every tracked file to this checkpoint. It saves the current versions first, so you can undo it.",
                      verb: "Roll Back",
                      destructive: true,
                    })
                  }
                >
                  <RotateCcw size={13} /> Roll Back…
                </button>
              </div>
            ))}
          </div>
        </>
      )}
    </Section>
  );
}
