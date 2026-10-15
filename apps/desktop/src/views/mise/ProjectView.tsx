import { useMemo, useState } from "react";
import {
  AlertTriangle,
  Download,
  Eye,
  EyeOff,
  FolderOpen,
  Loader2,
  Play,
  Plus,
  ShieldAlert,
  ShieldCheck,
  SquareTerminal,
  Trash2,
} from "lucide-react";
import { api, type ConfigPreview, type ProjectOverview, type ToolVersion } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { homeFrom } from "../../mise";
import { CommandBlock, Monogram, Section, Segmented } from "../../components/ui";
import { Sheet } from "../../components/Sheet";
import { TaskRunSheet } from "../../components/TaskRunSheet";
import {
  ConfigSetSheet,
  DependencyTree,
  EnvironmentBanner,
  EnvironmentPicker,
  GeneratorsSection,
  IdiomaticCallout,
  LockfileSection,
  SecretsSection,
  VarsSection,
  dependencyNames,
} from "./ProjectExtras";

type Tab = "tools" | "tasks" | "env" | "config";

export function ProjectView({ context, path }: { context: AppContext; path: string }) {
  const { mise, env, run } = context;
  const [environment, setEnvironment] = useState<string | null>(null);
  const overview = useAsync(() => api.projectOverview(path, environment), [path, environment, mise.revision]);
  const trusted = useAsync(() => api.trustedFolders(), [mise.revision]);
  const [tab, setTab] = useState<Tab>("tools");
  const home = homeFrom(env.miseConfigDir);
  const data = overview.data;
  const pretty = (value: string) => (value.startsWith(home) ? `~${value.slice(home.length)}` : value);

  if (overview.error && !data) {
    return (
      <div className="content">
        <div className="page">
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{overview.error}</span>
          </div>
          <ForgetButton context={context} path={path} />
        </div>
      </div>
    );
  }
  if (!data) {
    return (
      <div className="empty">
        <Loader2 size={20} className="spin" />
      </div>
    );
  }

  // Untrust only what mise trusts by name; a folder can also be trusted
  // through a parent, which revoking here wouldn't change.
  const explicitlyTrusted =
    data.trust.trusted &&
    !!trusted.data?.trusted.some((entry) => entry.path === data.path || entry.path === data.trust.path);

  const missing = Object.entries(data.tools ?? {}).filter(([, versions]) =>
    versions.some((version) => version.source && !version.installed),
  );

  return (
    <div className="content">
      <div className="page">
        <div className="detail-hero">
          <Monogram id={data.name} title={data.name} size={56} />
          <div style={{ minWidth: 0 }}>
            <h1 className="page-title">{data.name}</h1>
            <p className="page-subtitle mono selectable" style={{ marginTop: 4 }}>
              {pretty(data.path)}
            </p>
          </div>
          <div className="detail-actions">
            <button className="button large" onClick={() => api.reveal(data.path)}>
              <FolderOpen size={14} /> Show
            </button>
            {data.trust.trusted && (
              <button
                className="button large"
                onClick={() => void api.openTerminal(data.path).catch((reason: Error) => window.alert(reason.message))}
                title="Open a terminal here with this project’s tools (mise en)"
              >
                <SquareTerminal size={14} /> Terminal
              </button>
            )}
            {explicitlyTrusted && (
              <button
                className="button large"
                onClick={() =>
                  run({
                    action: { kind: "untrust", path: data.trust.path },
                    title: `Stop trusting ${data.name}?`,
                    description:
                      "mise stops loading this folder’s config — its tools, environment, hooks, and tasks — until you trust it again.",
                    verb: "Untrust",
                    destructive: true,
                  })
                }
              >
                Untrust…
              </button>
            )}
            {missing.length > 0 && (
              <button
                className="button primary large"
                onClick={() =>
                  run({
                    action: { kind: "installProject", project: data.path },
                    title: `Install ${missing.length} missing ${missing.length === 1 ? "tool" : "tools"}?`,
                    description: `mise installs what this project asks for: ${missing.map(([name]) => name).join(", ")}.`,
                    verb: "Install",
                  })
                }
              >
                <Download size={14} /> Install Missing
              </button>
            )}
          </div>
        </div>

        {!data.trust.trusted ? (
          <TrustReview data={data} pretty={pretty} context={context} />
        ) : (
          <>
            <EnvironmentBanner environment={data.environment} />
            <IdiomaticCallout data={data} context={context} />
            {data.problems.length > 0 && (
              <div className="callout warn selectable" style={{ marginTop: 18 }}>
                <AlertTriangle size={14} />
                <div>
                  {data.problems.map((problem) => (
                    <div key={problem}>{problem}</div>
                  ))}
                </div>
              </div>
            )}
            <div className="project-tabs">
              <Segmented
                label="Project section"
                value={tab}
                onChange={setTab}
                options={[
                  { value: "tools", label: `Tools ${Object.keys(data.tools ?? {}).length}` },
                  { value: "tasks", label: `Tasks ${data.tasks?.length ?? 0}` },
                  { value: "env", label: `Environment ${data.env?.length ?? 0}` },
                  { value: "config", label: `Config files ${data.configFiles?.length ?? 0}` },
                ]}
              />
              <EnvironmentPicker data={data} value={data.environment} onChange={setEnvironment} />
            </div>
            {tab === "tools" && <ToolsTab data={data} context={context} pretty={pretty} />}
            {tab === "tasks" && <TasksTab data={data} context={context} pretty={pretty} />}
            {tab === "env" && <EnvTab data={data} context={context} pretty={pretty} />}
            {tab === "config" && <ConfigTab data={data} context={context} pretty={pretty} />}
          </>
        )}

        <ForgetButton context={context} path={path} />
      </div>
    </div>
  );
}

type TabProps = { data: ProjectOverview; context: AppContext; pretty: (value: string) => string };

/** Before trusting, show what the config would do — read, never executed. */
function TrustReview({ data, pretty, context }: TabProps) {
  const risky = data.previews.some((preview) => preview.hooks.length > 0 || preview.envDirectives.length > 0);
  return (
    <>
      <div className="trust-panel">
        <ShieldAlert size={22} />
        <div>
          <h2>mise hasn’t trusted this folder yet</h2>
          <p>
            Until you trust it, mise won’t load its config — so nothing below has run. Review what trusting
            allows, then decide.{" "}
            {risky && "This config runs commands automatically, so read the highlighted parts closely."}
          </p>
        </div>
        <button
          className="button primary large"
          onClick={() =>
            context.run({
              action: { kind: "trustProject", project: data.path },
              title: `Trust ${data.name}?`,
              description:
                "mise will load this folder’s config, including its environment variables, hooks, and tasks. Only trust folders whose contents you’d run yourself.",
              verb: "Trust Folder",
            })
          }
        >
          <ShieldCheck size={14} /> Trust Folder…
        </button>
      </div>
      {data.previews.length === 0 && (
        <p className="section-note" style={{ marginTop: 14 }}>
          No config files found to preview. mise may be reading a file in a parent folder.
        </p>
      )}
      {data.previews.map((preview) => (
        <PreviewCard key={preview.path} preview={preview} pretty={pretty} />
      ))}
    </>
  );
}

function PreviewCard({ preview, pretty }: { preview: ConfigPreview; pretty: (value: string) => string }) {
  return (
    <Section title={pretty(preview.path)}>
      {preview.parseError && (
        <div className="callout warn selectable" style={{ marginBottom: 10 }}>
          <AlertTriangle size={14} />
          <span>Couldn’t read all of this file: {preview.parseError}</span>
        </div>
      )}
      <div className="list">
        {preview.hooks.map(([event, command]) => (
          <div className="list-row preview-row risky" key={`hook-${event}`}>
            <span className="badge orange">Runs on {event}</span>
            <code className="mono selectable">{command}</code>
          </div>
        ))}
        {preview.envDirectives.map((directive) => (
          <div className="list-row preview-row risky" key={directive}>
            <span className="badge orange">Loads</span>
            <code className="mono selectable">{directive}</code>
          </div>
        ))}
        {preview.tools.length > 0 && (
          <div className="list-row preview-row">
            <span className="badge">Tools</span>
            <span className="mono">{preview.tools.map(([name, version]) => `${name} ${version}`).join(" · ")}</span>
          </div>
        )}
        {preview.envNames.length > 0 && (
          <div className="list-row preview-row">
            <span className="badge">Sets</span>
            <span className="mono">{preview.envNames.join(", ")}</span>
          </div>
        )}
        {preview.tasks.map(([name, command]) => (
          <div className="list-row preview-row" key={`task-${name}`}>
            <span className="badge">Task {name}</span>
            <code className="mono selectable">{command}</code>
          </div>
        ))}
      </div>
    </Section>
  );
}

function ToolsTab({ data, context, pretty }: TabProps) {
  const [adding, setAdding] = useState(false);
  const inProject = (version: ToolVersion) => !!version.source && version.source.path.startsWith(`${data.path}/`);
  const rows = Object.entries(data.tools ?? {}).map(([name, versions]) => ({
    name,
    version: versions.find((version) => version.source) ?? versions[0],
  }));
  const local = rows.filter((row) => inProject(row.version));
  const inherited = rows.filter((row) => !inProject(row.version) && row.version.source);

  const table = (items: typeof rows) => (
    <div className="list">
      {items.map(({ name, version }) => (
        <div className="list-row" key={name}>
          <Monogram id={name} title={name} size={28} />
          <div className="row-main">
            <div className="row-title">{name}</div>
            <div className="row-subtitle">
              {version.source && pretty(version.source.path)}
              {version.requested_version && ` · asks for ${version.requested_version}`}
            </div>
          </div>
          <div className="row-trailing">
            {version.installed ? (
              <span className="badge green">Installed</span>
            ) : (
              <span className="badge orange">Not installed</span>
            )}
            <span className="mono version-pill">{version.version}</span>
          </div>
        </div>
      ))}
    </div>
  );

  return (
    <>
      <Section
        title="This project"
        note="Tools this folder asks for. Terminals inside it get these versions."
      >
        {local.length ? table(local) : <p className="section-note">This project doesn’t ask for any tools yet.</p>}
        <button className="button" style={{ marginTop: 10 }} onClick={() => setAdding(true)}>
          <Plus size={14} /> Add Tool
        </button>
      </Section>
      {inherited.length > 0 && (
        <Section title="From your global config" note="Used here too, unless this project asks for a different version.">
          {table(inherited)}
        </Section>
      )}
      <AddToolSheet open={adding} onClose={() => setAdding(false)} data={data} context={context} />
    </>
  );
}

function AddToolSheet({ open, onClose, data, context }: { open: boolean; onClose: () => void; data: ProjectOverview; context: AppContext }) {
  const [tool, setTool] = useState("");
  const [version, setVersion] = useState("latest");
  const names = useMemo(() => (context.mise.registry ?? []).map((entry) => entry.short), [context.mise.registry]);
  const valid = /^[A-Za-z0-9@:/._-]+$/.test(tool) && /^[A-Za-z0-9._-]+$/.test(version);
  return (
    <Sheet open={open} onDismiss={onClose} labelledBy="add-tool-title">
      <div className="sheet-head">
        <h2 id="add-tool-title">Add a tool to {data.name}</h2>
        <p>mise adds it to this project’s config and installs it. Other projects aren’t affected.</p>
      </div>
      <div className="sheet-body task-form">
        <label className="task-field">
          <span className="task-field__label">Tool</span>
          <input className="text-field mono" list="registry-tools" value={tool} onChange={(event) => setTool(event.target.value.trim())} placeholder="node" autoFocus spellCheck={false} />
          <datalist id="registry-tools">
            {names.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </label>
        <label className="task-field">
          <span className="task-field__label">Version</span>
          <input className="text-field mono" value={version} onChange={(event) => setVersion(event.target.value.trim())} spellCheck={false} />
          <span className="field-hint">A full version like 22.11.0, a prefix like 22, or latest.</span>
        </label>
        {tool && <CommandBlock command={`mise use ${tool}@${version}`} />}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onClose}>
          Cancel
        </button>
        <button
          className="button primary large"
          disabled={!valid}
          onClick={() => {
            onClose();
            context.run({
              action: { kind: "useInProject", project: data.path, tool, version },
              title: `Add ${tool}`,
              description: "",
              verb: "Add",
              autoStart: true,
            });
          }}
        >
          Add Tool
        </button>
      </div>
    </Sheet>
  );
}

function TasksTab({ data, context, pretty }: TabProps) {
  const [running, setRunning] = useState<string | null>(null);
  const tasks = data.tasks ?? [];
  return (
    <Section title="Tasks" note="Commands this project defines. Running one shows its output live; Stop ends it.">
      {tasks.length === 0 ? (
        <p className="section-note">No tasks defined here.</p>
      ) : (
        <div className="list">
          {tasks.map((task) => (
            <div className="list-row" key={task.name}>
              <div className="row-main">
                <div className="row-title mono">{task.name}</div>
                <div className="row-subtitle">
                  {task.description || task.run[0]} · {pretty(task.source)}
                  {task.depends.length > 0 && ` · runs ${dependencyNames(task).join(", ")} first`}
                </div>
              </div>
              {task.sources.length > 0 && !task.interactive && (
                <button
                  className="button"
                  title={`Rerun when ${task.sources.join(", ")} change`}
                  onClick={() =>
                    // `mise watch` runs watchexec, which may not be installed yet.
                    context.mise.inventory && !context.mise.inventory.watchexec
                      ? context.run({
                          action: { kind: "useGlobal", tool: "watchexec", version: "latest" },
                          title: "Install watchexec first?",
                          description:
                            "mise watch uses watchexec to notice file changes. mise installs it as a global tool; then click Watch again.",
                          verb: "Install",
                        })
                      : context.run({
                          action: { kind: "watch", project: data.path, task: task.name },
                          title: `Watch ${task.name}?`,
                          description: `mise runs ${task.name} now and again whenever ${task.sources.join(", ")} change, until you stop it.`,
                          verb: "Watch",
                        })
                  }
                >
                  <Eye size={13} /> Watch
                </button>
              )}
              {task.interactive ? (
                <button
                  className="button"
                  title="This task needs a real terminal"
                  onClick={() =>
                    void api.openTerminal(data.path, task.name).catch((reason: Error) => window.alert(reason.message))
                  }
                >
                  <SquareTerminal size={13} /> Run in Terminal
                </button>
              ) : (
                <button className="button" onClick={() => setRunning(task.name)}>
                  <Play size={13} /> Run
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      <DependencyTree tasks={tasks} />
      <TaskRunSheet
        project={data.path}
        task={running}
        onClose={() => setRunning(null)}
        onRun={(args) => {
          const task = running!;
          setRunning(null);
          context.run({
            action: { kind: "runTask", project: data.path, task, args },
            title: `Running ${task}`,
            description: "",
            verb: "Run",
            autoStart: true,
          });
        }}
      />
    </Section>
  );
}

function EnvTab({ data, context, pretty }: TabProps) {
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [target, setTarget] = useState<"shared" | "local">("shared");
  const [targetTouched, setTargetTouched] = useState(false);
  // Secret-looking names default to the machine-local file.
  const secretName = /SECRET|TOKEN|PASSWORD|PASSWD|API_?KEY|PRIVATE|CREDENTIAL|_KEY$/i.test(key);
  const effectiveTarget = targetTouched ? target : secretName ? "local" : "shared";
  const targetFile = `${data.path}/${effectiveTarget === "local" ? "mise.local.toml" : "mise.toml"}`;
  const vars = data.env ?? [];
  const keyValid = /^[A-Za-z_][A-Za-z0-9_]*$/.test(key);
  const fromProject = (source: string | null) => !!source && source.startsWith(`${data.path}/`);

  return (
    <>
      <Section
        title="Environment variables"
        note="What mise sets inside this folder. Secrets stay hidden until you reveal them."
      >
        {vars.length === 0 ? (
          <p className="section-note">mise doesn’t set any variables here.</p>
        ) : (
          <div className="list">
            {vars.map((variable) => {
              const shown = !variable.secret || revealed[variable.name];
              return (
                <div className="list-row" key={variable.name}>
                  <div className="row-main">
                    <div className="row-title mono">{variable.name}</div>
                    <div className="row-subtitle">{variable.source ? pretty(variable.source) : "set by mise"}</div>
                  </div>
                  <div className="row-trailing">
                    <span className={`mono env-value${shown ? " selectable" : ""}`}>{shown ? variable.value : "••••••••"}</span>
                    {variable.secret && (
                      <button
                        className="icon-button"
                        aria-label={shown ? `Hide ${variable.name}` : `Reveal ${variable.name}`}
                        onClick={() => setRevealed((current) => ({ ...current, [variable.name]: !current[variable.name] }))}
                      >
                        {shown ? <EyeOff size={14} /> : <Eye size={14} />}
                      </button>
                    )}
                    {fromProject(variable.source) && (
                      <button
                        className="icon-button"
                        aria-label={`Remove ${variable.name}`}
                        onClick={() =>
                          context.run({
                            action: { kind: "unsetEnv", project: data.path, key: variable.name, file: variable.source },
                            title: `Remove ${variable.name}?`,
                            description: `mise removes it from ${pretty(variable.source!)}.`,
                            verb: "Remove",
                            destructive: true,
                          })
                        }
                      >
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Section>
      <Section title="Add a variable" note="The value won’t appear in the activity log.">
        <div className="env-target">
          <span className="row-subtitle">Save to</span>
          <Segmented
            label="Save to"
            value={effectiveTarget}
            onChange={(next) => {
              setTarget(next);
              setTargetTouched(true);
            }}
            options={[
              { value: "shared", label: "mise.toml — shared with the project" },
              { value: "local", label: "mise.local.toml — this machine only" },
            ]}
          />
        </div>
        <div className="env-form">
          <input className="text-field mono" placeholder="NAME" value={key} onChange={(event) => setKey(event.target.value.toUpperCase())} aria-label="Variable name" spellCheck={false} />
          <input className="text-field mono" placeholder="value" value={value} onChange={(event) => setValue(event.target.value)} aria-label="Variable value" spellCheck={false} />
          <button
            className="button large"
            disabled={!keyValid}
            onClick={() => {
              context.run({
                action: { kind: "setEnv", project: data.path, key, value, file: targetFile },
                title: `Set ${key}`,
                description: "",
                verb: "Set",
                autoStart: true,
              });
              setKey("");
              setValue("");
              setTargetTouched(false);
            }}
          >
            Set
          </button>
        </div>
        {key && !keyValid && <p className="field-hint error">Use letters, numbers, and underscores, not starting with a number.</p>}
      </Section>
      <VarsSection data={data} context={context} pretty={pretty} />
      <SecretsSection data={data} />
    </>
  );
}

function ConfigTab({ data, context, pretty }: TabProps) {
  const files = data.configFiles ?? [];
  const [editing, setEditing] = useState<string | null>(null);
  return (
    <>
      <Section
        title="Config files"
        note="The files mise reads here, most specific first. When two ask for the same tool, the one higher in this list wins."
      >
        <div className="list">
          {files.map((file, index) => {
            const global = !file.path.startsWith(`${data.path}/`);
            return (
              <div className="list-row" key={file.path}>
                <span className="config-rank mono">{index + 1}</span>
                <div className="row-main">
                  <div className="row-title mono selectable">{pretty(file.path)}</div>
                  <div className="row-subtitle">
                    {file.path.endsWith(".local.toml")
                      ? "Local overrides — usually not committed"
                      : global
                        ? "Outside this project"
                        : file.path.endsWith(".tool-versions")
                          ? "asdf-style version file"
                          : "Project config"}
                    {file.tools.length > 0 && ` · ${file.tools.join(", ")}`}
                  </div>
                </div>
                {!global && file.path.endsWith(".toml") && (
                  <button className="button" onClick={() => setEditing(file.path)}>
                    Edit a Value…
                  </button>
                )}
                <button className="icon-button" aria-label="Show in folder" onClick={() => api.reveal(file.path)}>
                  <FolderOpen size={14} />
                </button>
              </div>
            );
          })}
        </div>
      </Section>
      <button
        className="button"
        style={{ marginTop: 10 }}
        onClick={() =>
          context.run({
            action: { kind: "formatConfig", project: data.path },
            title: "Tidy up the config files’ formatting?",
            description: "mise rewrites this project’s mise config files in its standard TOML layout. The settings don’t change.",
            verb: "Format",
          })
        }
      >
        Format Config Files…
      </button>
      <LockfileSection data={data} context={context} pretty={pretty} />
      <GeneratorsSection data={data} context={context} />
      <ConfigSetSheet file={editing} data={data} context={context} onClose={() => setEditing(null)} pretty={pretty} />
    </>
  );
}

function ForgetButton({ context, path }: { context: AppContext; path: string }) {
  return (
    <Section title="Remove from powermise">
      <p className="section-note">Removes this folder from the sidebar. Nothing on disk changes.</p>
      <button
        className="button destructive"
        onClick={async () => {
          await api.forgetProject(path);
          await context.refreshProjects();
          context.navigate({ kind: "installed" });
        }}
      >
        <Trash2 size={14} /> Remove from List
      </button>
    </Section>
  );
}

