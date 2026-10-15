import { Fragment, type ReactNode } from "react";
import { CheckCircle2, Circle, Loader2 } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { activationCommand } from "../../health";
import { useAsync } from "../../hooks";
import { ActivationSteps, ExternalLinkButton, GithubCard, SystemCheckRows, pendingImports } from "../../components/machine";
import { CommandBlock } from "../../components/ui";

export const WELCOMED_KEY = "powermise.welcomed";

/** Remembers that the person finished (or skipped) the welcome steps. */
export function markWelcomed() {
  try {
    localStorage.setItem(WELCOMED_KEY, "1");
  } catch {
    // Storage can be unavailable; the welcome page just shows again.
  }
}

export function hasBeenWelcomed(): boolean {
  try {
    return localStorage.getItem(WELCOMED_KEY) === "1";
  } catch {
    return false;
  }
}

/** First run: install mise, turn it on in the shell, bring over versions from
 *  other managers, and sign GitHub downloads in. Each step reflects what's
 *  already true, so it's also a checklist to come back to. */
/** Why the first install option is the one to pick. */
const RECOMMENDED: Partial<Record<string, string>> = {
  script: "Recommended by mise: its official build, which keeps itself up to date. No administrator password needed.",
  winget: "Recommended by mise on Windows.",
};

export function WelcomeView({ context }: { context: AppContext }) {
  const { env, mise, navigate, run } = context;
  const hasMise = !!env.misePath;
  const installers = useAsync(() => (hasMise ? Promise.resolve([]) : api.miseInstallers()), [hasMise]);
  const imports = useAsync(() => (hasMise ? api.detectImports() : Promise.resolve([])), [hasMise, mise.revision]);
  const activated = hasMise && !!mise.doctor?.activated;
  const shell = activationCommand(mise.doctor?.shell?.name, context.env.hostTarget, env.misePath);
  const version = mise.info?.version?.split(" ")[0];

  const finish = () => {
    markWelcomed();
    navigate({ kind: "installed" });
  };

  return (
    <div className="content">
      <div className="page welcome">
        <div className="page-header">
          <div>
            <h1 className="page-title">Welcome to powermise</h1>
            <p className="page-subtitle">
              An unofficial desktop companion for mise, the tool that installs and switches developer tools like
              Node.js, Python, and Go. A few steps and you’re set.
            </p>
          </div>
        </div>

        <Step
          number={1}
          done={hasMise}
          title={hasMise ? `mise ${version ?? ""} is installed` : "Install mise"}
          note={
            hasMise ? (
              <span className="mono">{env.misePath}</span>
            ) : (
              "powermise runs mise for everything it does, so mise comes first."
            )
          }
        >
          {!hasMise && (
            <>
              {installers.loading && (
                <p className="field-hint">
                  <Loader2 size={12} className="spin" /> Looking for a way to install it…
                </p>
              )}
              {(installers.data ?? []).map((installer, index) => (
                <Fragment key={installer.id}>
                  <div className="welcome-install">
                  <CommandBlock command={installer.command} />
                  <button
                    className={`button large${index === 0 ? " primary" : ""}`}
                    onClick={() =>
                      run({
                        action: { kind: "installMise", installer: installer.id },
                        title: "Install mise?",
                        description:
                          installer.id === "script"
                            ? "powermise runs mise’s official installer, which puts mise in ~/.local/bin — no administrator password needed. Nothing else on your computer changes."
                            : `powermise runs ${installer.label} to install mise. Nothing else on your computer changes.`,
                        verb: "Install mise",
                      })
                    }
                  >
                    Install with {installer.label}
                  </button>
                  </div>
                  {index === 0 && RECOMMENDED[installer.id] && (
                    <p className="field-hint" style={{ margin: "-2px 0 10px" }}>
                      {RECOMMENDED[installer.id]}
                    </p>
                  )}
                </Fragment>
              ))}

              {installers.data?.length === 0 && (
                <p className="section-note">
                  No package manager powermise can use was found.{" "}
                  <ExternalLinkButton url="https://mise.jdx.dev/getting-started.html">
                    Install mise by hand
                  </ExternalLinkButton>
                  , then reopen powermise.
                </p>
              )}
            </>
          )}
        </Step>

        {hasMise && mise.systemChecks && mise.systemChecks.length > 0 && (
          <Step
            number={2}
            done={mise.systemChecks.every((check) => check.ok)}
            title="Developer basics"
            note={
              mise.systemChecks.every((check) => check.ok)
                ? `${mise.systemChecks.map((check) => check.title).join(", ")}: all set.`
                : "mise’s Node, Python, Ruby, and Go come ready-built, but git, and anything that compiles, needs these."
            }
          >
            {mise.systemChecks.some((check) => !check.ok) && (
              <div className="list prose">
                <SystemCheckRows context={context} checks={mise.systemChecks} />
              </div>
            )}
          </Step>
        )}

        <Step
          number={hasMise && mise.systemChecks?.length ? 3 : 2}
          done={hasMise && activated}
          title="Turn mise on in your terminal"
          note={
            activated
              ? "Your shell switches tool versions automatically."
              : "So tools switch as you move between projects in the terminal."
          }
        >
          {hasMise && !activated && <ActivationSteps activation={shell} />}
        </Step>

        {hasMise && (
          <Step
            number={mise.systemChecks?.length ? 4 : 3}
            done={!!imports.data && pendingImports(context, imports.data).length === 0}
            title="Bring over versions you already have"
            note={
              imports.data && pendingImports(context, imports.data).length === 0
                ? imports.data.length
                  ? "mise has the versions your other version managers installed."
                  : "No versions from another version manager found."
                : "These were installed by another version manager. mise can use them as they are, without downloading them again."
            }
          >
            {imports.data && pendingImports(context, imports.data).length > 0 && (
              <button className="button" onClick={() => navigate({ kind: "import" })}>
                Review What to Bring Over
              </button>
            )}
          </Step>
        )}

        {hasMise && (
          <Step
            number={mise.systemChecks?.length ? 5 : 4}
            title="Sign in GitHub downloads (optional)" note="Avoids rate limits on busy days.">
            <GithubCard context={context} />
          </Step>
        )}

        <div className="welcome-foot">
          <button className="button primary large" onClick={finish} disabled={!hasMise}>
            {hasMise ? "Done" : "Install mise to continue"}
          </button>
          {!hasMise && (
            <button className="button large" onClick={() => navigate({ kind: "harnesses" })}>
              Browse AI Setups
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function Step({
  number,
  title,
  note,
  done,
  children,
}: {
  number: number;
  title: string;
  note?: ReactNode;
  done?: boolean;
  children?: ReactNode;
}) {
  return (
    <section className="welcome-step" aria-label={`Step ${number}`}>
      <div className="welcome-marker" aria-hidden>
        {done ? <CheckCircle2 size={20} className="outcome-icon succeeded" /> : <Circle size={20} />}
      </div>
      <div className="welcome-body">
        <h2 className="section-title">{title}</h2>
        {note && <p className="section-note">{note}</p>}
        {children}
      </div>
    </section>
  );
}
