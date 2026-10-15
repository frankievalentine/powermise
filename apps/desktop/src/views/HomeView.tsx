import { useState, type ReactNode } from "react";
import {
  ArrowRight,
  CheckCircle2,
  Circle,
  Compass,
  Cpu,
  Folder,
  HeartPulse,
  History,
  Layers,
  X,
} from "lucide-react";
import { api } from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import { healthSummary } from "../health";

const CHECKLIST_HIDDEN = "powermise.checklistHidden";

function readHidden(): boolean {
  try {
    return localStorage.getItem(CHECKLIST_HIDDEN) === "1";
  } catch {
    return false;
  }
}

/** Home: what to do next, and how things stand. */
export function HomeView({ context }: { context: AppContext }) {
  const { env, mise, projects, selections, navigate, addProject } = context;
  const [hidden, setHidden] = useState(readHidden);
  const github = useAsync(() => (env.misePath ? api.githubStatus() : Promise.resolve(null)), [mise.revision, env.misePath]);
  const imports = useAsync(() => (env.misePath ? api.detectImports() : Promise.resolve([])), [mise.revision, env.misePath]);
  const activity = useAsync(() => api.activity(), [mise.revision]);

  const installed = Object.keys(mise.inventory ?? {}).length;
  const updates = Object.keys(mise.outdated ?? {}).length;
  const health = healthSummary(mise.info, mise.doctor);

  const steps: Step[] = [
    {
      id: "terminal",
      title: "Turn mise on in your terminal",
      detail: "So terminals switch tool versions as you move between projects.",
      done: !!mise.doctor?.activated,
      action: { label: "Show me how", run: () => navigate({ kind: "health" }) },
    },
    {
      id: "tool",
      title: "Install your first tool",
      detail: "Node.js, Python, Go, and 1,000+ more — pick one in Discover.",
      done: installed > 0,
      action: { label: "Open Discover", run: () => navigate({ kind: "discover" }) },
    },
    {
      id: "project",
      title: "Add a project folder",
      detail: "See and install the tools a project asks for, run its tasks, and manage its settings.",
      done: projects.length > 0,
      action: { label: "Add a Folder…", run: () => void addProject() },
    },
    ...(imports.data && imports.data.length > 0
      ? [
          {
            id: "imports",
            title: "Bring over versions you already have",
            detail: `${imports.data.map((entry) => entry.source).join(", ")} installed versions mise can use without downloading again.`,
            done: false,
            action: { label: "Bring Over", run: () => navigate({ kind: "health", section: "imports" }) },
          },
        ]
      : []),
    {
      id: "github",
      title: "Sign in GitHub downloads",
      detail: "Avoids GitHub's download limits when installing tools.",
      done: !!github.data?.token.token,
      optional: true,
      action: { label: "Set Up", run: () => navigate({ kind: "health", section: "github" }) },
    },
    {
      id: "agents",
      title: "Set up AI coding agents",
      detail: "Claude, Codex, Gemini, and others, with everything they need.",
      done: selections.length > 0,
      optional: true,
      action: { label: "Browse Agents", run: () => navigate({ kind: "harnesses" }) },
    },
  ];
  const remaining = steps.filter((step) => !step.done && !step.optional).length;
  const showChecklist = !hidden && steps.some((step) => !step.done);

  const hide = () => {
    setHidden(true);
    try {
      localStorage.setItem(CHECKLIST_HIDDEN, "1");
    } catch {
      // Hiding is a convenience; the checklist just shows again.
    }
  };

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Home</h1>
            <p className="page-subtitle">How mise is set up on this computer, and what to do next.</p>
          </div>
        </div>

        {!env.misePath && (
          <div className="callout warn">
            <span>
              mise isn’t installed yet.{" "}
              <button className="link-button" onClick={() => navigate({ kind: "welcome" })}>
                Install mise
              </button>
            </span>
          </div>
        )}

        {showChecklist && (
          <section className="section home-checklist">
            <div className="section-heading-row">
              <h2 className="section-title">
                Get started
                {remaining > 0 && <span className="badge">{remaining} to go</span>}
              </h2>
              <button className="icon-button" onClick={hide} aria-label="Hide the checklist" title="Hide">
                <X size={14} />
              </button>
            </div>
            <div className="list prose">
              {steps.map((step) => (
                <div className={`list-row${step.done ? " done" : ""}`} key={step.id}>
                  {step.done ? (
                    <CheckCircle2 size={18} className="outcome-icon succeeded" />
                  ) : (
                    <Circle size={18} color="var(--text-tertiary)" />
                  )}
                  <div className="row-main">
                    <div className="row-title">
                      {step.title}
                      {step.optional && !step.done && <span className="row-subtitle"> · optional</span>}
                    </div>
                    {!step.done && <div className="row-subtitle">{step.detail}</div>}
                  </div>
                  {!step.done && (
                    <button className="button" onClick={step.action.run}>
                      {step.action.label}
                    </button>
                  )}
                </div>
              ))}
            </div>
          </section>
        )}

        <section className="section">
          <h2 className="section-title">At a glance</h2>
          <div className="grid home-glance">
            <Glance
              icon={<Layers size={18} />}
              title="My Tools"
              value={installed ? `${installed} installed` : "None yet"}
              note={updates ? `${updates} ${updates === 1 ? "update" : "updates"} ready` : "Everything up to date"}
              attention={updates > 0}
              onClick={() => navigate({ kind: updates ? "updates" : "installed" })}
            />
            <Glance
              icon={<HeartPulse size={18} />}
              title="Health"
              value={health.count ? `${health.count} to look at` : "All good"}
              note={mise.info?.version ? `mise ${mise.info.version.split(" ")[0]}` : "mise not found"}
              attention={health.count > 0}
              onClick={() => navigate({ kind: "health" })}
            />
            <Glance
              icon={<Folder size={18} />}
              title="Projects"
              value={projects.length ? `${projects.length} ${projects.length === 1 ? "folder" : "folders"}` : "None yet"}
              note={projects[0] ? `Last opened: ${projects[0].name}` : "Add a project folder"}
              onClick={() => (projects[0] ? navigate({ kind: "project", path: projects[0].path }) : void addProject())}
            />
            <Glance
              icon={<Cpu size={18} />}
              title="AI Agents"
              value={selections.length ? `${selections.length} ${selections.length === 1 ? "setup" : "setups"}` : "Not set up"}
              note={`${selections.filter((selection) => selection.applied).length} applied`}
              onClick={() =>
                navigate(selections[0] ? { kind: "setup", name: selections[0].name } : { kind: "harnesses" })
              }
            />
          </div>
        </section>

        <section className="section">
          <div className="section-heading-row">
            <h2 className="section-title">Recent changes</h2>
            {(activity.data?.length ?? 0) > 0 && (
              <button className="link-button" onClick={() => navigate({ kind: "activity" })}>
                <History size={12} aria-hidden /> All history
              </button>
            )}
          </div>
          {activity.data && activity.data.length > 0 ? (
            <div className="list">
              {activity.data.slice(0, 5).map((entry) => (
                <div className="list-row" key={entry.id}>
                  {entry.outcome?.success ? (
                    <CheckCircle2 size={15} className="outcome-icon succeeded" />
                  ) : (
                    <Circle size={15} color={entry.finishedAt ? "var(--red)" : "var(--text-tertiary)"} />
                  )}
                  <div className="row-main">
                    <div className="row-title mono">{entry.command}</div>
                  </div>
                  <span className="row-subtitle">{new Date(entry.startedAt).toLocaleString()}</span>
                </div>
              ))}
            </div>
          ) : (
            <p className="section-note">
              Nothing yet. Installs, updates, and other changes show up here.{" "}
              <button className="link-button" onClick={() => navigate({ kind: "discover" })}>
                <Compass size={12} aria-hidden /> Find a tool
              </button>
            </p>
          )}
        </section>
      </div>
    </div>
  );
}

interface Step {
  id: string;
  title: string;
  detail: string;
  done: boolean;
  optional?: boolean;
  action: { label: string; run: () => void };
}

function Glance({
  icon,
  title,
  value,
  note,
  attention,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  value: string;
  note: string;
  attention?: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`card glance${attention ? " attention" : ""}`} onClick={onClick}>
      <div className="glance-head">
        {icon}
        <span>{title}</span>
        <ArrowRight size={14} className="glance-arrow" />
      </div>
      <div className="glance-value">{value}</div>
      <div className="row-subtitle">{note}</div>
    </button>
  );
}
