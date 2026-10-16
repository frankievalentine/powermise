import { useEffect, useState, useSyncExternalStore } from "react";
import { ArrowUpCircle, CheckCircle2, Loader2, RefreshCw } from "lucide-react";
import { api, inTauri, type AppUpdate } from "../api";
import { Section } from "./ui";

/** powermise's own updates (src-tauri/src/updater.rs), shared by the
 *  sidebar and Settings. */
interface UpdateState {
  update: AppUpdate | null;
  checked: boolean;
  checking: boolean;
  installing: boolean;
  error: string | null;
}

let state: UpdateState = { update: null, checked: false, checking: false, installing: false, error: null };
const listeners = new Set<() => void>();

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) listener();
}

function useUpdateState(): UpdateState {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => state,
  );
}

/** Looks for a newer powermise. In a browser there's nothing to update. */
export async function checkForAppUpdate() {
  if (state.checking) return;
  set({ checking: true, error: null });
  try {
    set({ update: inTauri ? await api.checkAppUpdate() : null, checked: true });
  } catch (reason) {
    set({ error: (reason as Error).message });
  } finally {
    set({ checking: false });
  }
}

/** Downloads and installs the update; the app restarts into it. */
async function installAppUpdate() {
  set({ installing: true, error: null });
  try {
    await api.installAppUpdate();
  } catch (reason) {
    set({ installing: false, error: (reason as Error).message });
  }
}

/** The sidebar's update button, when a newer powermise is out. */
export function SidebarUpdate() {
  const { update, installing, error } = useUpdateState();
  if (!update) return null;
  return (
    <button
      className="sidebar-update"
      onClick={() => void installAppUpdate()}
      disabled={installing}
      title={error ?? `Downloads powermise ${update.version} and restarts into it`}
    >
      {installing ? <Loader2 size={13} className="spin" /> : <ArrowUpCircle size={13} />}
      {installing ? `Updating to ${update.version}…` : `Restart to Update to ${update.version}`}
    </button>
  );
}

/** Settings: the version, and checking for a newer one. */
export function AboutPowermise() {
  const { update, checked, checking, installing, error } = useUpdateState();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    api.appVersion().then(setVersion, () => setVersion(null));
  }, []);
  return (
    <Section title="About powermise">
      <div className="list">
        <div className="list-row">
          <div className="row-main">
            <div className="row-title">powermise {version ?? ""}</div>
            <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
              An unofficial, independent companion for mise — not made by, affiliated with, or endorsed by the mise
              project. Report problems that only happen in powermise on its GitHub repository.
            </div>
            <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
              {error
                ? `Couldn’t check for updates: ${error}`
                : update
                  ? `powermise ${update.version} is available.`
                  : checked
                    ? "You have the latest version."
                    : "powermise checks for a newer version each time it opens."}
            </div>
          </div>
          <div className="row-trailing">
            {update ? (
              <button className="button primary" onClick={() => void installAppUpdate()} disabled={installing}>
                {installing ? <Loader2 size={14} className="spin" /> : <ArrowUpCircle size={14} />}
                {installing ? "Updating…" : "Restart to Update"}
              </button>
            ) : (
              <button className="button" onClick={() => void checkForAppUpdate()} disabled={checking}>
                {checking ? (
                  <Loader2 size={14} className="spin" />
                ) : checked && !error ? (
                  <CheckCircle2 size={14} />
                ) : (
                  <RefreshCw size={14} />
                )}
                Check for Updates
              </button>
            )}
          </div>
        </div>
      </div>
    </Section>
  );
}
