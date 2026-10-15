import { useEffect, useRef, useState } from "react";
import { FolderPlus, Loader2, Minus, Plus, RotateCcw, X } from "lucide-react";
import { api, runQuietly, type MiseAction, type Setting } from "../../api";
import type { AppContext } from "../../App";
import { COMMON_SETTINGS, type CommonSetting } from "../../data/commonSettings";
import { Switch } from "../../components/ui";

type Pretty = (path: string) => string;

/** A setting's value as the text `mise settings set` takes. */
function asText(value: unknown): string {
  if (Array.isArray(value)) return value.map(String).join(",");
  return value === null || value === undefined ? "" : String(value);
}

function asList(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(String);
  return asText(value)
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

/** Who set a value, in words. */
export function sourceLabel(source: string | null, context: AppContext, pretty: Pretty): string {
  if (!source) return "Default";
  const globalConfig = context.env.miseConfigDir;
  if (source === `${globalConfig}/config.toml` || source === `${globalConfig}\\config.toml`) return "Changed by you";
  if (source.includes("conf.d/powermise-")) return "Set by an AI setup";
  const project = context.projects.find((entry) => source.startsWith(`${entry.path}/`));
  if (project) return `Set by the ${project.name} project`;
  if (!source.includes("/") && !source.includes("\\")) return `Set by ${source}`;
  return `Set in ${pretty(source)}`;
}

interface Feedback {
  state: "saving" | "saved" | "failed";
  command?: string;
  message?: string;
  undo?: MiseAction;
}

export function CommonSettings({
  context,
  settings,
  query,
  pretty,
}: {
  context: AppContext;
  settings: Setting[];
  query: string;
  pretty: Pretty;
}) {
  const byKey = new Map(settings.map((setting) => [setting.key, setting]));
  const needle = query.trim().toLowerCase();
  const groups = COMMON_SETTINGS.map((group) => ({
    ...group,
    settings: group.settings.filter(
      (setting) =>
        byKey.has(setting.key) &&
        (!needle ||
          `${setting.title} ${setting.description} ${setting.key} ${setting.keywords.join(" ")}`
            .toLowerCase()
            .includes(needle)),
    ),
  })).filter((group) => group.settings.length > 0);

  if (!groups.length) {
    return (
      <div className="empty">
        <h3>No matches</h3>
        <p>Try another word, or look in All settings.</p>
      </div>
    );
  }
  return (
    <>
      {groups.map((group) => (
        <section className="section" key={group.title}>
          <h2 className="section-title">{group.title}</h2>
          <div className="list prose">
            {group.settings.map((entry) => (
              <CommonRow key={entry.key} entry={entry} setting={byKey.get(entry.key)!} context={context} pretty={pretty} />
            ))}
          </div>
        </section>
      ))}
    </>
  );
}

function CommonRow({
  entry,
  setting,
  context,
  pretty,
}: {
  entry: CommonSetting;
  setting: Setting;
  context: AppContext;
  pretty: Pretty;
}) {
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const status = sourceLabel(setting.source, context, pretty);
  const changedByYou = status === "Changed by you";
  // A project, AI setup, or environment variable sets it: a global change
  // won't show there.
  const setElsewhere = !!setting.source && !changedByYou;

  /** Writes `next` (or resets with `null`), remembering how to undo it. */
  const save = async (next: string | null) => {
    const undo: MiseAction = setting.source
      ? { kind: "setSetting", key: setting.key, value: asText(setting.value) }
      : { kind: "unsetSetting", key: setting.key };
    await apply(next === null ? { kind: "unsetSetting", key: setting.key } : { kind: "setSetting", key: setting.key, value: next }, undo);
  };

  const apply = async (action: MiseAction, undo?: MiseAction) => {
    setFeedback({ state: "saving" });
    try {
      const result = await runQuietly(action);
      setFeedback(
        result.ok
          ? { state: "saved", command: result.command, undo }
          : { state: "failed", command: result.command, message: result.output.slice(-3).join(" ") || "mise couldn’t save it." },
      );
    } catch (reason) {
      setFeedback({ state: "failed", message: (reason as Error).message });
    }
  };

  return (
    <div className="list-row setting-row common-row">
      <div className="row-main">
        <div className="row-title">{entry.title}</div>
        <div className="row-subtitle">{entry.description}</div>
        <div className="row-subtitle setting-source">
          <span className={`badge${changedByYou ? " accent" : ""}`}>{status}</span>
          {changedByYou && (
            <button className="link-button" onClick={() => void save(null)}>
              <RotateCcw size={11} aria-hidden /> Reset
            </button>
          )}
          <span className="mono muted-key">{setting.key}</span>
        </div>
        {setElsewhere && (
          <div className="row-subtitle">Changing it here updates your own settings; {status.toLowerCase()} still wins where it applies.</div>
        )}
        {feedback && <FeedbackLine feedback={feedback} onUndo={(undo) => void apply(undo)} />}
      </div>
      <div className="row-trailing common-control">
        <Control entry={entry} setting={setting} context={context} pretty={pretty} onSave={(next) => void save(next)} busy={feedback?.state === "saving"} />
      </div>
    </div>
  );
}

function FeedbackLine({ feedback, onUndo }: { feedback: Feedback; onUndo: (undo: MiseAction) => void }) {
  if (feedback.state === "saving") {
    return (
      <div className="row-subtitle common-feedback">
        <Loader2 size={11} className="spin" /> Saving…
      </div>
    );
  }
  if (feedback.state === "failed") {
    return <div className="row-subtitle common-feedback error selectable">Couldn’t save: {feedback.message}</div>;
  }
  return (
    <div className="row-subtitle common-feedback">
      Saved with <code className="selectable">{feedback.command}</code>
      {feedback.undo && (
        <button className="link-button" onClick={() => onUndo(feedback.undo!)}>
          Undo
        </button>
      )}
    </div>
  );
}

function Control({
  entry,
  setting,
  context,
  pretty,
  onSave,
  busy,
}: {
  entry: CommonSetting;
  setting: Setting;
  context: AppContext;
  pretty: Pretty;
  onSave: (next: string | null) => void;
  busy: boolean;
}) {
  const control = entry.control;
  switch (control.kind) {
    case "switch": {
      const on = setting.value === true || setting.value === "true";
      return <Switch checked={on} label={entry.title} onChange={(next) => !busy && onSave(String(next))} />;
    }
    case "choice":
    case "duration": {
      const current = asText(setting.value);
      const options = control.options.some((option) => option.value === current)
        ? control.options
        : [...control.options, { value: current, label: `Custom (${current})` }];
      return (
        <select
          className="select"
          value={current}
          disabled={busy}
          aria-label={entry.title}
          onChange={(event) => onSave(event.target.value)}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      );
    }
    case "stepper":
      return <Stepper value={Number(setting.value) || control.min} min={control.min} max={control.max} unit={control.unit} busy={busy} onSave={onSave} label={entry.title} />;
    case "tools": {
      const selected = asList(setting.value);
      const installed = Object.keys(context.mise.inventory ?? {});
      const choices = [...new Set([...control.suggestions, ...installed])].filter((tool) => !selected.includes(tool)).sort();
      const write = (next: string[]) => onSave(next.length ? next.join(",") : null);
      return (
        <div className="list-editor">
          <div className="chip-row">
            {selected.length === 0 && <span className="row-subtitle">None</span>}
            {selected.map((tool) => (
              <span key={tool} className="badge chip">
                {tool}
                <button
                  className="chip-remove"
                  aria-label={`Remove ${tool}`}
                  disabled={busy}
                  onClick={() => write(selected.filter((item) => item !== tool))}
                >
                  <X size={10} />
                </button>
              </span>
            ))}
          </div>
          <select
            className="select"
            value=""
            disabled={busy || choices.length === 0}
            aria-label={`Add to ${entry.title}`}
            onChange={(event) => event.target.value && write([...selected, event.target.value])}
          >
            <option value="">Add a tool…</option>
            {choices.map((tool) => (
              <option key={tool} value={tool}>
                {tool}
              </option>
            ))}
          </select>
        </div>
      );
    }
    case "folders": {
      const selected = asList(setting.value);
      const write = (next: string[]) => onSave(next.length ? next.join(",") : null);
      return (
        <div className="list-editor">
          {selected.length === 0 && <span className="row-subtitle">None</span>}
          {selected.map((folder) => (
            <span key={folder} className="badge chip mono">
              {pretty(folder)}
              <button
                className="chip-remove"
                aria-label={`Remove ${folder}`}
                disabled={busy}
                onClick={() => write(selected.filter((item) => item !== folder))}
              >
                <X size={10} />
              </button>
            </span>
          ))}
          <button
            className="button"
            disabled={busy}
            onClick={async () => {
              const folder = await api.pickFolder().catch(() => null);
              if (folder && !selected.includes(folder)) write([...selected, folder]);
            }}
          >
            <FolderPlus size={13} /> Add Folder…
          </button>
        </div>
      );
    }
  }
}

/** − value + ; saves shortly after the last click, so several clicks are
 *  one change. */
function Stepper({
  value,
  min,
  max,
  unit,
  busy,
  onSave,
  label,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  busy: boolean;
  onSave: (next: string) => void;
  label: string;
}) {
  const [draft, setDraft] = useState(value);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => setDraft(value), [value]);
  const change = (next: number) => {
    const clamped = Math.min(max, Math.max(min, next));
    setDraft(clamped);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => clamped !== value && onSave(String(clamped)), 600);
  };
  return (
    <div className="stepper" role="group" aria-label={label}>
      <button className="icon-button" aria-label="Fewer" disabled={busy || draft <= min} onClick={() => change(draft - 1)}>
        <Minus size={13} />
      </button>
      <span className="stepper-value">
        {draft} <span className="row-subtitle">{unit}</span>
      </span>
      <button className="icon-button" aria-label="More" disabled={busy || draft >= max} onClick={() => change(draft + 1)}>
        <Plus size={13} />
      </button>
    </div>
  );
}
