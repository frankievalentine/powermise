import { useMemo, useState } from "react";
import { Loader2, RotateCw } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { ExternalLinkButton } from "../../components/machine";
import catalogData from "../../data/macos-defaults.json";
import { prettyPath } from "../../mise";

type Value = boolean | number | string;

interface CatalogOption {
  label: string;
  value: Value;
  isDefault: boolean;
  note: string;
}

interface CatalogSetting {
  id: string;
  category: string;
  title: string;
  description: string;
  domain: string;
  key: string;
  type: "bool" | "int" | "float" | "string";
  relaunch: string | null;
  testedOn: string[];
  options: CatalogOption[];
  source: string;
}

const SETTINGS = (catalogData as { settings: CatalogSetting[] }).settings;
const KEYS = SETTINGS.map(({ domain, key }) => ({ domain, key }));
const CATEGORY_ORDER = [
  "Dock",
  "Finder",
  "Desktop",
  "Keyboard",
  "Trackpad",
  "Mouse",
  "Mission Control",
  "Menu Bar",
  "Screenshots",
];
const NOT_MANAGED = "__not_managed__";
const CUSTOM = "__custom__";

const id = (domain: string, key: string) => `${domain} ${key}`;

/** `defaults read` prints booleans as 1/0. */
function matches(current: string | null, value: Value): boolean {
  if (current === null) return false;
  if (typeof value === "boolean") return current === (value ? "1" : "0") || current === String(value);
  if (typeof value === "number") return Number(current) === value;
  return current === value;
}

function optionText(option: CatalogOption): string {
  const label = option.label === "true" ? "On" : option.label === "false" ? "Off" : option.label;
  return option.isDefault ? `${label} (macOS default)` : label;
}

function describeCurrent(setting: CatalogSetting, current: string | null): string {
  if (current === null) {
    const fallback = setting.options.find((option) => option.isDefault);
    return fallback ? `macOS default (${optionText(fallback).replace(" (macOS default)", "")})` : "macOS default";
  }
  const option = setting.options.find((candidate) => matches(current, candidate.value));
  if (option) return optionText(option).replace(" (macOS default)", "");
  if (setting.type === "bool") return current === "1" ? "On" : current === "0" ? "Off" : current;
  return current;
}

/** A catalog of macOS settings (from macos-defaults.com). Choices go into
 *  powermise's `conf.d/powermise-macos.toml`; mise applies them. */
export function MacosSettingsView({ context, query }: { context: AppContext; query: string }) {
  const { mise, run } = context;
  const [nonce, setNonce] = useState(0);
  const state = useAsync(() => api.macosSettings(KEYS), [mise.revision, nonce]);
  const [error, setError] = useState<string | null>(null);
  const miseTooOld = !!mise.info && !mise.info.recommendedMet;

  const managed = useMemo(
    () => new Map((state.data?.managed ?? []).map((entry) => [id(entry.domain, entry.key), entry.value])),
    [state.data],
  );
  const current = useMemo(
    () => new Map(KEYS.map((entry, index) => [id(entry.domain, entry.key), state.data?.current[index] ?? null])),
    [state.data],
  );

  const pending = SETTINGS.filter((setting) => {
    const value = managed.get(id(setting.domain, setting.key));
    return value !== undefined && !matches(current.get(id(setting.domain, setting.key)) ?? null, value);
  });
  const relaunch = [...new Set(pending.map((setting) => setting.relaunch).filter((app): app is string => !!app))];

  const choose = async (setting: CatalogSetting, value: Value | null) => {
    setError(null);
    try {
      await api.setMacosDefault(setting.domain, setting.key, value);
      setNonce((count) => count + 1);
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const visible = SETTINGS.filter(
      (setting) =>
        !needle ||
        `${setting.category} ${setting.title} ${setting.description} ${setting.domain} ${setting.key}`
          .toLowerCase()
          .includes(needle),
    );
    const byCategory = new Map<string, CatalogSetting[]>();
    for (const setting of visible) {
      byCategory.set(setting.category, [...(byCategory.get(setting.category) ?? []), setting]);
    }
    // System settings first, then apps.
    const rank = (category: string) => {
      const index = CATEGORY_ORDER.indexOf(category);
      return index < 0 ? CATEGORY_ORDER.length : index;
    };
    return [...byCategory.entries()].sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));
  }, [query]);

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">macOS Settings</h1>
            <p className="page-subtitle">
              Choose settings and mise keeps this Mac set that way. Your choices are saved in{" "}
              <span className="mono">{state.data ? prettyPath(state.data.fragmentPath) : "conf.d/powermise-macos.toml"}</span>
              ; nothing changes until you apply them. Settings from{" "}
              <ExternalLinkButton url="https://macos-defaults.com">macos-defaults.com</ExternalLinkButton> (MIT).
            </p>
          </div>
        </div>

        {miseTooOld && (
          <div className="callout warn">
            <span>
              Managing macOS settings needs mise {mise.info?.recommended} or newer, which reads powermise’s file from{" "}
              <code>conf.d</code>.{" "}
              <button className="link-button" onClick={() => context.navigate({ kind: "health" })}>
                Update mise in Health
              </button>
            </span>
          </div>
        )}
        {error && <div className="callout error selectable">{error}</div>}
        {state.error && <div className="callout error selectable">{state.error}</div>}

        {pending.length > 0 && (
          <div className="banner warn macos-apply">
            <span>
              {pending.length} {pending.length === 1 ? "setting isn’t" : "settings aren’t"} applied yet.
            </span>
            <button
              className="button primary"
              disabled={miseTooOld}
              onClick={() =>
                run({
                  action: { kind: "bootstrapApply", only: "macos-defaults" },
                  title: `Apply ${pending.length} macOS ${pending.length === 1 ? "setting" : "settings"}?`,
                  description: `mise writes them with \`defaults write\`.${relaunch.length ? ` ${relaunch.join(", ")} ${relaunch.length === 1 ? "needs" : "need"} to relaunch to show the change — you can do that next.` : ""}`,
                  verb: "Apply",
                })
              }
            >
              Apply…
            </button>
          </div>
        )}

        {state.loading && !state.data && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {pending.length === 0 && <RelaunchBar context={context} apps={appliedApps(managed, current)} />}

        {state.data &&
          groups.map(([category, settings]) => (
            <section className="section" key={category}>
              <h2 className="section-title">{category}</h2>
              <div className="list prose">
                {settings.map((setting) => (
                  <SettingRow
                    key={setting.id}
                    setting={setting}
                    managedValue={managed.get(id(setting.domain, setting.key))}
                    current={current.get(id(setting.domain, setting.key)) ?? null}
                    onChoose={(value) => void choose(setting, value)}
                  />
                ))}
              </div>
            </section>
          ))}
        {state.data && groups.length === 0 && (
          <div className="empty">
            <h3>No matches</h3>
            <p>Try a different search.</p>
          </div>
        )}
      </div>
    </div>
  );
}

function SettingRow({
  setting,
  managedValue,
  current,
  onChoose,
}: {
  setting: CatalogSetting;
  managedValue: Value | undefined;
  current: string | null;
  onChoose: (value: Value | null) => void;
}) {
  const isManaged = managedValue !== undefined;
  const optionIndex = isManaged ? setting.options.findIndex((option) => option.value === managedValue) : -1;
  const [custom, setCustom] = useState(isManaged && optionIndex < 0 ? String(managedValue) : "");
  const [editingCustom, setEditingCustom] = useState(isManaged && optionIndex < 0);
  const selected = !isManaged ? NOT_MANAGED : optionIndex >= 0 ? String(optionIndex) : CUSTOM;
  const note = optionIndex >= 0 ? setting.options[optionIndex].note : "";
  const applied = isManaged && matches(current, managedValue);

  const parseCustom = (text: string): Value | null => {
    if (setting.type === "int") return /^-?\d+$/.test(text) ? Number(text) : null;
    if (setting.type === "float") return /^-?\d+(\.\d+)?$/.test(text) ? Number(text) : null;
    return text.length ? text : null;
  };

  return (
    <div className="list-row setting-row">
      <div className="row-main">
        <div className="row-title">
          {setting.title}
          {isManaged && <span className={`badge ${applied ? "green" : "orange"}`}>{applied ? "Applied" : "Not applied"}</span>}
        </div>
        <div className="row-subtitle">{setting.description}</div>
        <div className="row-subtitle setting-source">
          Now: {describeCurrent(setting, current)} · <span className="mono">{setting.domain} {setting.key}</span>
          {setting.testedOn.length > 0 && ` · tested on ${setting.testedOn.slice(-3).join(", ")}`}
        </div>
        {note && <div className="row-subtitle">{note}</div>}
      </div>
      <div className="row-trailing macos-control">
        <select
          className="select"
          value={selected}
          aria-label={setting.title}
          onChange={(event) => {
            const choice = event.target.value;
            if (choice === NOT_MANAGED) {
              setEditingCustom(false);
              onChoose(null);
            } else if (choice === CUSTOM) {
              setEditingCustom(true);
            } else {
              setEditingCustom(false);
              onChoose(setting.options[Number(choice)].value);
            }
          }}
        >
          <option value={NOT_MANAGED}>Leave as is</option>
          {setting.options.map((option, index) => (
            <option key={index} value={String(index)}>
              {optionText(option)}
            </option>
          ))}
          {setting.type !== "bool" && <option value={CUSTOM}>Custom…</option>}
        </select>
        {editingCustom && setting.type !== "bool" && (
          <form
            className="setting-edit"
            onSubmit={(event) => {
              event.preventDefault();
              const value = parseCustom(custom.trim());
              if (value !== null) onChoose(value);
            }}
          >
            <input
              className="text-field mono"
              value={custom}
              onChange={(event) => setCustom(event.target.value)}
              placeholder={setting.type === "string" ? "text" : "number"}
              inputMode={setting.type === "string" ? undefined : "decimal"}
              aria-label={`${setting.title} value`}
              style={{ width: 120 }}
            />
            <button className="button" type="submit" disabled={parseCustom(custom.trim()) === null}>
              Save
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

/** Apps to relaunch for the settings mise has applied. */
function appliedApps(managed: Map<string, Value>, current: Map<string, string | null>): string[] {
  return [
    ...new Set(
      SETTINGS.filter((setting) => {
        const value = managed.get(id(setting.domain, setting.key));
        return value !== undefined && matches(current.get(id(setting.domain, setting.key)) ?? null, value);
      })
        .map((setting) => setting.relaunch)
        .filter((app): app is string => !!app),
    ),
  ];
}

/** After applying, apps like the Dock and Finder only show a change once
 *  they relaunch. */
function RelaunchBar({ context, apps }: { context: AppContext; apps: string[] }) {
  if (!apps.length) return null;
  return (
    <div className="chip-row" style={{ marginTop: 12 }}>
      <span className="row-subtitle">Relaunch to see changes:</span>
      {apps.map((app) => (
        <button
          key={app}
          className="button"
          onClick={() =>
            context.run({
              action: { kind: "relaunchApp", app },
              title: `Relaunch ${app}?`,
              description: `${app} quits and macOS starts it again, so it picks up the new settings. Unsaved work in ${app} could be lost.`,
              verb: "Relaunch",
            })
          }
        >
          <RotateCw size={13} /> {app}
        </button>
      ))}
    </div>
  );
}
