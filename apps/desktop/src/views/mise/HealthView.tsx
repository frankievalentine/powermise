import { AlertTriangle, CheckCircle2, Info, Loader2, RefreshCw } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { CommandBlock, Section } from "../../components/ui";

/** The line that hooks mise into a shell, by shell name. */
function activationCommand(shell: string): { file: string; line: string } {
  if (shell.includes("bash")) return { file: "~/.bashrc", line: 'eval "$(mise activate bash)"' };
  if (shell.includes("fish")) return { file: "~/.config/fish/config.fish", line: "mise activate fish | source" };
  if (shell.includes("pwsh") || shell.includes("powershell"))
    return { file: "$PROFILE", line: "mise activate pwsh | Out-String | Invoke-Expression" };
  return { file: "~/.zshrc", line: 'eval "$(mise activate zsh)"' };
}

export function HealthView({ context }: { context: AppContext }) {
  const { mise, run } = context;
  const doctor = useAsync(() => api.doctor(), [mise.revision]);
  const info = mise.info;
  const problems = [...(doctor.data?.errors ?? []), ...(doctor.data?.warnings ?? [])];
  const shell = activationCommand(doctor.data?.shell?.name ?? "zsh");
  const healthy = !!doctor.data && problems.length === 0 && doctor.data.activated && !!info?.supported;

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
                : `${problems.length + (doctor.data.activated ? 0 : 1) + (info?.supported ? 0 : 1)} thing(s) need attention.`}
            </span>
          </div>
        )}

        <Section title="mise">
          <div className="list">
            <div className="list-row">
              <div className="row-main">
                <div className="row-title">Version</div>
                <div className="row-subtitle selectable">{info?.version ?? "Not found"}</div>
              </div>
              <div className="row-trailing">
                {info?.supported ? (
                  <span className="badge green">Supported</span>
                ) : (
                  <span className="badge orange">Needs {info?.minimum} or newer</span>
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
            {!doctor.data.activated && (
              <div style={{ marginTop: 10 }}>
                <p className="section-note" style={{ margin: "0 0 8px" }}>
                  Add this line to {shell.file}, then open a new terminal. Powermise doesn’t edit your
                  shell files for you.
                </p>
                <CommandBlock command={`echo '${shell.line}' >> ${shell.file}`} />
              </div>
            )}
            <div className="callout info" style={{ marginTop: 10 }}>
              <Info size={14} />
              <span>
                Powermise can’t see settings exported in your shell files (like <code>MISE_*</code>{" "}
                variables), because apps opened from the Dock don’t load them. Put mise settings in{" "}
                <code>~/.config/mise/config.toml</code> to share them with Powermise.
              </span>
            </div>
          </Section>
        )}

        {problems.length > 0 && (
          <Section title="Problems mise found">
            <div className="list">
              {problems.map((problem) => (
                <div className="list-row" key={problem}>
                  <AlertTriangle size={15} color="var(--orange)" />
                  <div className="row-main">
                    <div className="row-title selectable" style={{ fontWeight: 400, whiteSpace: "normal" }}>
                      {problem}
                    </div>
                  </div>
                </div>
              ))}
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

        <Section title="In the terminal">
          <CommandBlock command="mise doctor" />
        </Section>
      </div>
    </div>
  );
}
