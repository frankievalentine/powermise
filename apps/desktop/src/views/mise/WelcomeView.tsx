import type { ReactNode } from "react";
import { CheckCircle2, Circle, Loader2 } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { activationCommand } from "../../health";
import { useAsync } from "../../hooks";
import { ActivationSteps, ExternalLinkButton, GithubCard, ImportList } from "../../components/machine";
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
                <div className="welcome-install" key={installer.id}>
                  <CommandBlock command={installer.command} />
                  <button
                    className={`button large${index === 0 ? " primary" : ""}`}
                    onClick={() =>
                      run({
                        action: { kind: "installMise", installer: installer.id },
                        title: "Install mise?",
                        description: `powermise runs ${installer.label} to install mise. Nothing else on your computer changes.`,
                        verb: "Install mise",
                      })
                    }
                  >
                    Install with {installer.label}
                  </button>
                </div>
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

        <Step
          number={2}
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
            number={3}
            done={imports.data?.length === 0}
            title="Bring over versions you already have"
            note={
              imports.data?.length === 0
                ? "No versions from nvm, nodenv, pyenv, uv, or Homebrew found."
                : "These were installed by another version manager. mise can use them as they are, without downloading them again."
            }
          >
            {imports.data && <ImportList context={context} imports={imports.data} />}
          </Step>
        )}

        {hasMise && (
          <Step number={4} title="Sign in GitHub downloads (optional)" note="Avoids rate limits on busy days.">
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
