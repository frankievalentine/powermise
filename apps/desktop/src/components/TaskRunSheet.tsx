import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Loader2, Play } from "lucide-react";
import { api, type TaskDetails, type TaskInput } from "../api";
import { Sheet } from "./Sheet";
import { Checkbox, CommandBlock } from "./ui";

/** A form for a task's arguments and flags, generated from its usage spec.
 *  Shows the exact `mise run` command as you fill it in, then runs it. */
export function TaskRunSheet({
  project,
  task,
  onClose,
  onRun,
}: {
  project: string;
  task: string | null;
  onClose: () => void;
  onRun: (args: string[]) => void;
}) {
  const [details, setDetails] = useState<TaskDetails | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [switches, setSwitches] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (!task) return;
    setDetails(null);
    setError(null);
    setValues({});
    setSwitches({});
    api
      .taskDetails(project, task)
      .then((loaded) => {
        setDetails(loaded);
        const defaults: Record<string, string> = {};
        for (const input of [...loaded.args, ...loaded.flags]) {
          if (input.default) defaults[input.name] = input.default;
        }
        setValues(defaults);
      })
      .catch((reason: Error) => setError(reason.message));
  }, [project, task]);

  const args = useMemo(() => {
    if (!details) return [];
    const result: string[] = [];
    for (const input of details.args) {
      const value = values[input.name]?.trim();
      if (value) result.push(value);
    }
    for (const flag of details.flags) {
      const name = flagName(flag);
      if (flag.takesValue) {
        const value = values[flag.name]?.trim();
        if (value) result.push(name, value);
      } else if (switches[flag.name]) {
        result.push(name);
      }
    }
    return result;
  }, [details, values, switches]);

  const missing = details?.args.filter((input) => input.required && !values[input.name]?.trim()) ?? [];
  const command = `mise run ${task ?? ""}${args.length ? ` -- ${args.map(quote).join(" ")}` : ""}`;

  return (
    <Sheet open={!!task} onDismiss={onClose} labelledBy="task-title">
      <div className="sheet-head">
        <h2 id="task-title">
          Run <span className="mono">{task}</span>
        </h2>
        {details?.description && <p>{details.description}</p>}
      </div>
      <div className="sheet-body">
        {!details && !error && (
          <div className="empty" style={{ padding: 20 }}>
            <Loader2 size={18} className="spin" />
          </div>
        )}
        {error && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}
        {details && (
          <div className="task-form">
            {details.depends.length > 0 && (
              <p className="field-hint">Runs first: {details.depends.join(", ")}</p>
            )}
            {details.args.length === 0 && details.flags.length === 0 && (
              <p className="field-hint">This task doesn’t take any arguments.</p>
            )}
            {details.args.map((input) => (
              <Field key={input.name} input={input} value={values[input.name] ?? ""} onChange={(value) => setValues((current) => ({ ...current, [input.name]: value }))} />
            ))}
            {details.flags.map((flag) =>
              flag.takesValue ? (
                <Field key={flag.name} input={flag} value={values[flag.name] ?? ""} onChange={(value) => setValues((current) => ({ ...current, [flag.name]: value }))} />
              ) : (
                <button
                  key={flag.name}
                  type="button"
                  className="task-switch"
                  role="checkbox"
                  aria-checked={!!switches[flag.name]}
                  onClick={() => setSwitches((current) => ({ ...current, [flag.name]: !current[flag.name] }))}
                >
                  <Checkbox checked={!!switches[flag.name]} />
                  <span>
                    <span className="mono">{flagName(flag)}</span>
                    {flag.help && <span className="row-subtitle"> — {flag.help}</span>}
                  </span>
                </button>
              ),
            )}
            <CommandBlock command={command} />
          </div>
        )}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onClose}>
          Cancel
        </button>
        <button
          className="button primary large"
          disabled={!details || missing.length > 0}
          title={missing.length ? `Fill in ${missing.map((input) => input.name).join(", ")}` : undefined}
          onClick={() => onRun(args)}
        >
          <Play size={14} /> Run
        </button>
      </div>
    </Sheet>
  );
}

function Field({ input, value, onChange }: { input: TaskInput; value: string; onChange: (value: string) => void }) {
  const id = `task-input-${input.name}`;
  return (
    <label className="task-field" htmlFor={id}>
      <span className="task-field__label">
        <span className="mono">{input.usage || input.name}</span>
        {input.required && <span className="badge">Required</span>}
      </span>
      {input.choices.length > 0 ? (
        <select id={id} className="select" value={value} onChange={(event) => onChange(event.target.value)}>
          {!input.required && <option value="">—</option>}
          {input.required && !value && <option value="">Choose…</option>}
          {input.choices.map((choice) => (
            <option key={choice} value={choice}>
              {choice}
            </option>
          ))}
        </select>
      ) : (
        <input id={id} className="text-field mono" value={value} onChange={(event) => onChange(event.target.value)} spellCheck={false} />
      )}
      {input.help && <span className="field-hint">{input.help}</span>}
    </label>
  );
}

/** `--level <n>` → `--level`; `-v --verbose` → `--verbose`. */
function flagName(flag: TaskInput): string {
  const words = flag.usage.split(/\s+/).filter((word) => word.startsWith("-"));
  return words.find((word) => word.startsWith("--")) ?? words[0] ?? `--${flag.name}`;
}

function quote(argument: string): string {
  return /[\s'"]/.test(argument) ? `'${argument.replace(/'/g, `'\\''`)}'` : argument;
}
