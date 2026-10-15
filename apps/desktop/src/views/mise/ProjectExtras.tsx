// Project page extras: environments, idiomatic version files, task
// dependencies, [vars], the lockfile, config edits, and generated files.
import { useState } from "react";
import { FileCode2, Info, Loader2, Lock, Pencil, RefreshCw } from "lucide-react";
import { api, type Generator, type ProjectOverview, type Task } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { Section } from "../../components/ui";
import { Sheet } from "../../components/Sheet";

type Pretty = (value: string) => string;

// ---------------------------------------------------------------------------
// Environments (MISE_ENV)
// ---------------------------------------------------------------------------

export function EnvironmentPicker({
  data,
  value,
  onChange,
}: {
  data: ProjectOverview;
  value: string | null;
  onChange: (environment: string | null) => void;
}) {
  if (!data.environments.length) return null;
  return (
    <label className="environment-picker">
      <span>Environment</span>
      <select
        className="select"
        value={value ?? ""}
        onChange={(event) => onChange(event.target.value || null)}
        aria-label="Environment to preview"
      >
        <option value="">Default</option>
        {data.environments.map((name) => (
          <option key={name} value={name}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function EnvironmentBanner({ environment }: { environment: string | null }) {
  if (!environment) return null;
  return (
    <div className="callout info" style={{ marginTop: 16 }}>
      <Info size={14} />
      <span>
        Previewing <strong>{environment}</strong>: what mise loads with <code>MISE_ENV={environment}</code>, including{" "}
        <code>mise.{environment}.toml</code>. Changes you make still edit the default config files.
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Idiomatic version files (.nvmrc, .python-version, …)
// ---------------------------------------------------------------------------

export function IdiomaticCallout({ data, context }: { data: ProjectOverview; context: AppContext }) {
  const ignored = data.idiomatic.filter((file) => !file.enabled);
  if (!ignored.length) return null;
  return (
    <div className="list prose" style={{ marginTop: 18 }}>
      {ignored.map((file) => (
        <div className="list-row" key={file.file}>
          <Info size={16} color="var(--orange)" />
          <div className="row-main">
            <div className="row-title">
              <span className="mono">{file.file}</span> asks for {file.tool} {file.version}
            </div>
            <div className="row-subtitle">
              {file.disabledFile
                ? `mise reads ${file.tool}’s version files, but ${file.file} is turned off on its own. Turning it back on applies to every project.`
                : `mise ignores version files from other tools until you turn them on for ${file.tool}. Turning it on applies to every project.`}
            </div>
          </div>
          <button
            className="button"
            onClick={() => {
              if (file.disabledFile) {
                const rest = file.disabledFiles.filter((entry) => entry !== `${file.tool}:${file.file}`);
                context.run({
                  action: rest.length
                    ? { kind: "setSetting", key: "idiomatic_version_file_disable_files", value: rest.join(",") }
                    : { kind: "unsetSetting", key: "idiomatic_version_file_disable_files" },
                  title: `Read ${file.file} for ${file.tool} again?`,
                  description: `mise will read ${file.file} for ${file.tool} in every project, unless a mise config asks for a version.`,
                  verb: "Turn On",
                });
                return;
              }
              context.run({
                action: { kind: "addSetting", key: "idiomatic_version_file_enable_tools", value: file.tool },
                title: `Use ${file.file} files for ${file.tool}?`,
                description: `mise will read ${file.file} (and ${file.tool}’s other version files) in every project, unless a mise config asks for a version.`,
                verb: "Turn On",
              });
            }}
          >
            Use {file.file}
          </button>
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Task dependencies
// ---------------------------------------------------------------------------

/** The task names a `depends` entry refers to (`"build"`, `"build --x"`, `["build", "--x"]`). */
export function dependencyNames(task: Task): string[] {
  return task.depends
    .map((entry) => (Array.isArray(entry) ? entry[0] : entry.split(/\s+/)[0]))
    .filter((name): name is string => !!name);
}

export function DependencyTree({ tasks }: { tasks: Task[] }) {
  const byName = new Map(tasks.map((task) => [task.name, task]));
  const withDeps = tasks.filter((task) => task.depends.length > 0);
  if (!withDeps.length) return null;
  // Roots: tasks with dependencies that nothing else depends on.
  const dependedOn = new Set(tasks.flatMap(dependencyNames));
  const roots = withDeps.filter((task) => !dependedOn.has(task.name));
  const shown = roots.length ? roots : withDeps;

  const render = (name: string, trail: string[]): React.ReactNode => {
    const task = byName.get(name);
    const children = task ? dependencyNames(task) : [];
    const cycle = trail.includes(name);
    return (
      <li key={`${trail.join(">")}>${name}`}>
        <span className="mono">{name}</span>
        {cycle && <span className="badge orange">cycle</span>}
        {!task && <span className="row-subtitle"> (not defined here)</span>}
        {!cycle && children.length > 0 && <ul>{children.map((child) => render(child, [...trail, name]))}</ul>}
      </li>
    );
  };

  return (
    <Section title="What runs first" note="Running a task runs what it depends on first, top to bottom.">
      <ul className="dependency-tree">{shown.map((task) => render(task.name, []))}</ul>
    </Section>
  );
}

// ---------------------------------------------------------------------------
// [vars]
// ---------------------------------------------------------------------------

export function VarsSection({ data, context, pretty }: { data: ProjectOverview; context: AppContext; pretty: Pretty }) {
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  if (data.vars === null) {
    return (
      <Section title="Vars">
        <p className="section-note">
          Showing and saving a project’s <code>[vars]</code> needs mise 2026.10.6 or newer. Update mise from Health.
        </p>
      </Section>
    );
  }
  return (
    <Section
      title="Vars"
      note="Config values ([vars]) that templates and tasks use. Ones the config asks you for can be saved on this computer; values written in the config itself are fixed."
    >
      {data.vars.length === 0 ? (
        <p className="section-note">This project defines no vars.</p>
      ) : (
        <div className="list">
          {data.vars.map((variable) => (
            <div className="list-row" key={variable.name}>
              <div className="row-main">
                <div className="row-title">
                  <span className="mono">{variable.name}</span>
                  {variable.required && variable.value === null && <span className="badge orange"> Needs a value</span>}
                </div>
                <div className="row-subtitle">
                  {variable.prompt ?? (variable.saved ? "Saved on this computer" : variable.source ? pretty(variable.source) : "")}
                </div>
              </div>
              {editing === variable.name ? (
                <form
                  className="setting-edit"
                  onSubmit={(event) => {
                    event.preventDefault();
                    setEditing(null);
                    context.run({
                      action: { kind: "setVar", project: data.path, name: variable.name, value: draft },
                      title: `Save ${variable.name} on this computer?`,
                      description: "mise keeps it in its own state, outside every config file, for this project’s config to use.",
                      verb: "Save",
                    });
                  }}
                >
                  <input
                    className="text-field mono"
                    value={draft}
                    onChange={(event) => setDraft(event.target.value)}
                    aria-label={variable.name}
                    autoFocus
                  />
                  <button className="button" type="submit">
                    Save…
                  </button>
                </form>
              ) : (
                <div className="row-trailing">
                  {variable.value !== null && <span className="mono selectable">{variable.value}</span>}
                  {variable.settable && (
                    <button
                      className="icon-button"
                      aria-label={`Set ${variable.name} on this computer`}
                      title="Set on this computer"
                      onClick={() => {
                        setDraft(variable.value ?? "");
                        setEditing(variable.name);
                      }}
                    >
                      <Pencil size={13} />
                    </button>
                  )}
                  {variable.saved && (
                    <button
                      className="button"
                      onClick={() =>
                        context.run({
                          action: { kind: "unsetVar", project: data.path, name: variable.name },
                          title: `Forget the saved ${variable.name}?`,
                          description: "The config’s default (or its prompt) applies again.",
                          verb: "Forget",
                        })
                      }
                    >
                      Forget
                    </button>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </Section>
  );
}

export function SecretsSection({ data }: { data: ProjectOverview }) {
  if (!data.secrets) return null;
  return (
    <Section
      title="Secrets"
      note={`Names mise loads from ${data.secrets.source ?? "the project’s secrets source"}. Values stay there; powermise never reads them.`}
    >
      {data.secrets.keys.length ? (
        <div className="chip-row">
          {data.secrets.keys.map((key) => (
            <span key={key} className="badge mono">
              {key}
            </span>
          ))}
        </div>
      ) : (
        <p className="section-note">No secrets listed yet.</p>
      )}
    </Section>
  );
}

// ---------------------------------------------------------------------------
// Lockfile
// ---------------------------------------------------------------------------

export function LockfileSection({ data, context, pretty }: { data: ProjectOverview; context: AppContext; pretty: Pretty }) {
  const lock = () =>
    context.run({
      action: { kind: "lock", project: data.path },
      title: data.lockfile ? "Update the lockfile?" : "Create a lockfile?",
      description:
        "mise records the exact version and download checksums of each tool, so everyone who uses this project installs the same thing.",
      verb: data.lockfile ? "Update" : "Create",
    });
  return (
    <Section title="Lockfile">
      <div className="list prose">
        <div className="list-row">
          <Lock size={16} color="var(--text-tertiary)" />
          <div className="row-main">
            {data.lockfile ? (
              <>
                <div className="row-title mono">{pretty(data.lockfile.path)}</div>
                <div className="row-subtitle">
                  {data.lockfile.changes.length === 0
                    ? "Up to date with the config."
                    : `Out of date: ${data.lockfile.changes
                        .map(
                          (change) =>
                            `${change.name} ${change.old_versions.join(", ") || "(new)"} → ${change.new_versions.join(", ") || "(removed)"}`,
                        )
                        .join("; ")}`}
                </div>
              </>
            ) : (
              <>
                <div className="row-title">No lockfile</div>
                <div className="row-subtitle">
                  A lockfile pins exact versions and checksums, so everyone gets the same tools.
                </div>
              </>
            )}
          </div>
          {(!data.lockfile || data.lockfile.changes.length > 0) && (
            <button className="button" onClick={lock}>
              <RefreshCw size={13} /> {data.lockfile ? "Update" : "Create Lockfile"}
            </button>
          )}
        </div>
      </div>
    </Section>
  );
}

// ---------------------------------------------------------------------------
// Edit a config value (mise config set)
// ---------------------------------------------------------------------------

const VALUE_TYPES = [
  { value: "", label: "Automatic" },
  { value: "string", label: "Text" },
  { value: "integer", label: "Whole number" },
  { value: "float", label: "Number" },
  { value: "bool", label: "True / false" },
  { value: "list", label: "List" },
] as const;

export function ConfigSetSheet({
  file,
  data,
  context,
  onClose,
  pretty,
}: {
  file: string | null;
  data: ProjectOverview;
  context: AppContext;
  onClose: () => void;
  pretty: Pretty;
}) {
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [valueType, setValueType] = useState<(typeof VALUE_TYPES)[number]["value"]>("");
  const keyValid = /^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)*$/.test(key);
  const close = () => {
    setKey("");
    setValue("");
    setValueType("");
    onClose();
  };
  return (
    <Sheet open={!!file} onDismiss={close} labelledBy="config-set-title">
      <div className="sheet-head">
        <h2 id="config-set-title">Edit a value</h2>
        <p>
          Sets one value in <span className="mono">{file ? pretty(file) : ""}</span> with{" "}
          <span className="mono">mise config set</span>. Use dotted keys, like <code>tools.node</code>,{" "}
          <code>env.PORT</code>, or <code>settings.jobs</code>.
        </p>
      </div>
      <div className="sheet-body">
        <label className="task-field">
          <span className="task-field__label">Key</span>
          <input
            className="text-field mono"
            value={key}
            onChange={(event) => setKey(event.target.value.trim())}
            placeholder="tools.node"
            spellCheck={false}
          />
          {key && !keyValid && <span className="field-hint error">Use words separated by dots.</span>}
        </label>
        <label className="task-field" style={{ marginTop: 12 }}>
          <span className="task-field__label">Value</span>
          <input
            className="text-field mono"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            placeholder="22"
            spellCheck={false}
          />
        </label>
        <label className="task-field" style={{ marginTop: 12 }}>
          <span className="task-field__label">Type</span>
          <select
            className="select"
            value={valueType}
            onChange={(event) => setValueType(event.target.value as typeof valueType)}
          >
            {VALUE_TYPES.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={close}>
          Cancel
        </button>
        <button
          className="button primary large"
          disabled={!keyValid || !value || value.startsWith("-")}
          onClick={() => {
            const target = file!;
            const action = {
              kind: "configSet" as const,
              project: data.path,
              file: target,
              key,
              value,
              valueType: valueType || null,
            };
            close();
            context.run({
              action,
              title: `Set ${action.key}?`,
              description: `mise writes it to ${pretty(target)}.`,
              verb: "Set",
            });
          }}
        >
          Set…
        </button>
      </div>
    </Sheet>
  );
}

// ---------------------------------------------------------------------------
// Generated files (mise generate)
// ---------------------------------------------------------------------------

const GENERATORS: { id: Generator; title: string; description: string; file: string }[] = [
  {
    id: "github-action",
    title: "GitHub Actions workflow",
    description: "Runs a task in CI with the project's tools installed by mise.",
    file: ".github/workflows/",
  },
  {
    id: "git-pre-commit",
    title: "Git pre-commit hook",
    description: "Runs a task before every commit.",
    file: ".git/hooks/pre-commit",
  },
  {
    id: "devcontainer",
    title: "Dev container",
    description: "A container definition with mise, for Codespaces and editors that support dev containers.",
    file: ".devcontainer/devcontainer.json",
  },
];

export function GeneratorsSection({ data, context }: { data: ProjectOverview; context: AppContext }) {
  const [open, setOpen] = useState<Generator | null>(null);
  return (
    <Section title="Add to this project" note="Files mise can write for you. You see the file before it’s added.">
      <div className="list prose">
        {GENERATORS.map((generator) => {
          const needsGit = generator.id !== "devcontainer" && !data.isGitRepo;
          return (
            <div className="list-row" key={generator.id}>
              <FileCode2 size={16} color="var(--text-tertiary)" />
              <div className="row-main">
                <div className="row-title">{generator.title}</div>
                <div className="row-subtitle">
                  {needsGit ? "Needs a Git repository — this folder isn’t one." : generator.description}
                </div>
              </div>
              <button className="button" onClick={() => setOpen(generator.id)} disabled={needsGit}>
                Preview…
              </button>
            </div>
          );
        })}
      </div>
      <GeneratorSheet generator={open} data={data} context={context} onClose={() => setOpen(null)} />
    </Section>
  );
}

function GeneratorSheet({
  generator,
  data,
  context,
  onClose,
}: {
  generator: Generator | null;
  data: ProjectOverview;
  context: AppContext;
  onClose: () => void;
}) {
  const info = GENERATORS.find((entry) => entry.id === generator);
  const needsTask = generator === "github-action" || generator === "git-pre-commit";
  const tasks = data.tasks ?? [];
  const [task, setTask] = useState<string>("");
  const chosenTask = needsTask ? task || tasks[0]?.name || null : null;
  const preview = useAsync(
    () => (generator ? api.generatePreview(data.path, generator, chosenTask) : Promise.resolve("")),
    [generator, chosenTask, data.path],
  );
  return (
    <Sheet open={!!generator} onDismiss={onClose} labelledBy="generate-title">
      <div className="sheet-head">
        <h2 id="generate-title">Add a {info?.title.toLowerCase()}?</h2>
        <p>
          mise writes this to <span className="mono">{info?.file}</span> in {data.name}. Review it first; you can edit
          it afterwards like any file.
        </p>
      </div>
      <div className="sheet-body">
        {needsTask && (
          <label className="task-field" style={{ marginBottom: 12 }}>
            <span className="task-field__label">Task to run</span>
            <select className="select" value={chosenTask ?? ""} onChange={(event) => setTask(event.target.value)}>
              {tasks.map((entry) => (
                <option key={entry.name} value={entry.name}>
                  {entry.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {preview.loading && (
          <p className="field-hint">
            <Loader2 size={12} className="spin" /> Generating…
          </p>
        )}
        {preview.error && <div className="callout error selectable">{preview.error}</div>}
        {preview.data && <pre className="code-view selectable generated-file">{preview.data}</pre>}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onClose}>
          Cancel
        </button>
        <button
          className="button primary large"
          disabled={!preview.data || (needsTask && !chosenTask)}
          onClick={() => {
            onClose();
            context.run({
              action: { kind: "generate", project: data.path, generator: generator!, task: chosenTask },
              title: `Adding the ${info?.title.toLowerCase()}`,
              description: "",
              verb: "Add",
              autoStart: true,
            });
          }}
        >
          Add File
        </button>
      </div>
    </Sheet>
  );
}
