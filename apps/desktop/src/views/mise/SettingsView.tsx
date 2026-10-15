import { useMemo, useState } from "react";
import { Loader2, RotateCcw } from "lucide-react";
import { api, type Setting } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { ExternalLinkButton } from "../../components/machine";
import { Segmented, Switch } from "../../components/ui";

const GROUP_TITLE: Record<string, string> = {
  "": "General",
  github: "GitHub",
  gitlab: "GitLab",
  npm: "npm",
  pypi: "PyPI",
  oci: "OCI",
  spm: "Swift packages",
  otel: "OpenTelemetry",
  hook_env: "Shell hooks",
  self_update: "Updating mise",
  status: "Status messages",
  dotnet: ".NET",
};

const groupOf = (key: string) => (key.includes(".") ? key.split(".")[0] : "");
const groupTitle = (group: string) =>
  GROUP_TITLE[group] ?? group.charAt(0).toUpperCase() + group.slice(1).replace(/_/g, " ");

/** Settings mise can set from a single text value. Arrays and tables are
 *  edited in the config file instead. */
const editable = (setting: Setting) => !setting.type.includes("array") && !setting.type.includes("object");

const asText = (value: unknown) =>
  value === null || value === undefined ? "" : typeof value === "string" ? value : JSON.stringify(value);

type Filter = "all" | "changed";

export function SettingsView({ context, query }: { context: AppContext; query: string }) {
  const { mise, env } = context;
  const settings = useAsync(() => api.settings(), [mise.revision]);
  const [filter, setFilter] = useState<Filter>("all");

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const visible = (settings.data ?? []).filter(
      (setting) =>
        (filter === "all" || setting.source) &&
        // Deprecated settings only show when something still sets them.
        (!setting.deprecated || setting.source) &&
        (!needle ||
          setting.key.toLowerCase().includes(needle) ||
          setting.description.toLowerCase().includes(needle)),
    );
    const byGroup = new Map<string, Setting[]>();
    for (const setting of visible) {
      const group = groupOf(setting.key);
      byGroup.set(group, [...(byGroup.get(group) ?? []), setting]);
    }
    return [...byGroup.entries()].sort(([a], [b]) => (a === "" ? -1 : b === "" ? 1 : a.localeCompare(b)));
  }, [settings.data, filter, query]);

  const changedCount = (settings.data ?? []).filter((setting) => setting.source).length;

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Settings</h1>
            <p className="page-subtitle">
              How mise behaves everywhere on this computer. Changes go in your global mise config with{" "}
              <span className="mono">mise settings set</span>.{" "}
              <ExternalLinkButton url="https://mise.jdx.dev/configuration/settings.html">All settings</ExternalLinkButton>
            </p>
          </div>
          <Segmented
            label="Show"
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: "All" },
              { value: "changed", label: `Changed${changedCount ? ` (${changedCount})` : ""}` },
            ]}
          />
        </div>

        {settings.error && <div className="callout error selectable">{settings.error}</div>}
        {settings.loading && !settings.data && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}
        {settings.data && groups.length === 0 && (
          <div className="empty">
            <h3>{query ? "No matches" : "Nothing changed yet"}</h3>
            <p>{query ? "Try a different search." : "Every setting is at mise’s default."}</p>
          </div>
        )}

        {groups.map(([group, entries]) => (
          <section className="section" key={group || "general"}>
            <h2 className="section-title">{groupTitle(group)}</h2>
            <div className="list">
              {entries.map((setting) => (
                // Keyed by value too, so an edit resets once mise reports the change.
                <SettingRow key={`${setting.key}=${asText(setting.value)}`} setting={setting} context={context} globalDir={env.miseConfigDir} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

function SettingRow({ setting, context, globalDir }: { setting: Setting; context: AppContext; globalDir: string }) {
  const { run } = context;
  const original = asText(setting.value);
  const [draft, setDraft] = useState(original);
  // `mise settings set/unset` edit the global config.toml — not conf.d
  // fragments or other files, which are only shown.
  const inGlobal =
    setting.source === `${globalDir}/config.toml` || setting.source === `${globalDir}\\config.toml`;
  const elsewhere = !!setting.source && !inGlobal;
  const pretty = (path: string) => path.replace(/^\/(Users|home)\/[^/]+/, "~");
  const name = setting.key.includes(".") ? setting.key.slice(setting.key.indexOf(".") + 1) : setting.key;

  const set = (value: string) =>
    run({
      action: { kind: "setSetting", key: setting.key, value },
      title: `Change ${setting.key}?`,
      description: elsewhere
        ? `This writes it to your global mise config, but ${pretty(setting.source!)} also sets it and takes precedence there.`
        : "mise writes this to your global config.",
      verb: "Change",
    });

  let control: React.ReactNode;
  if (setting.type === "boolean") {
    control = (
      <Switch checked={setting.value === true} onChange={(next) => set(String(next))} label={setting.key} />
    );
  } else if (editable(setting)) {
    control = (
      <form
        className="setting-edit"
        onSubmit={(event) => {
          event.preventDefault();
          if (draft !== original) set(draft);
        }}
      >
        <input
          className="text-field mono"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          inputMode={setting.type === "number" ? "numeric" : undefined}
          placeholder="Not set"
          aria-label={setting.key}
          spellCheck={false}
        />
        {draft !== original && (
          <button className="button" type="submit">
            Save…
          </button>
        )}
      </form>
    );
  } else {
    control = (
      <span className="mono row-subtitle" title="Edit lists in your mise config file">
        {original || "[]"}
      </span>
    );
  }

  return (
    <div className="list-row setting-row">
      <div className="row-main">
        <div className="row-title">
          <span className="mono">{name}</span>
          {setting.deprecated && <span className="badge orange">Deprecated</span>}
        </div>
        <div className="row-subtitle">{setting.description}</div>
        {setting.source && (
          <div className="row-subtitle setting-source">
            Set in {pretty(setting.source)}
            {inGlobal && (
              <button
                className="link-button"
                onClick={() =>
                  run({
                    action: { kind: "unsetSetting", key: setting.key },
                    title: `Reset ${setting.key}?`,
                    description: "mise removes it from your global config, so the default applies again.",
                    verb: "Reset",
                  })
                }
              >
                <RotateCcw size={11} aria-hidden /> Reset to default
              </button>
            )}
          </div>
        )}
      </div>
      <div className="row-trailing">{control}</div>
    </div>
  );
}
