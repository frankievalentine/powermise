import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { inTauri } from "../api";
import { applyPlatform, UI_PLATFORMS, usePlatform } from "../platform";
import { Segmented } from "./ui";

/** The window frame above the app for the current platform.
 *
 *  - macOS: the system draws traffic lights over the sidebar (Tauri overlay
 *    title bar); a browser preview draws stand-ins.
 *  - Windows: our own title bar with caption buttons on the right, over Mica.
 *  - Linux: the system's own frame; a browser preview draws a stand-in. */
export function WindowChrome() {
  const platform = usePlatform();
  if (platform === "win32") return <WindowsTitleBar />;
  if (!inTauri && platform === "linux") return <FakeHeaderBar />;
  if (!inTauri && platform === "darwin") return <FakeTrafficLights />;
  return null;
}

async function tauriWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

function WindowsTitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!inTauri) return;
    let stop: (() => void) | undefined;
    tauriWindow().then(async (window) => {
      setMaximized(await window.isMaximized());
      stop = await window.onResized(async () => setMaximized(await window.isMaximized()));
    });
    return () => stop?.();
  }, []);

  const act = (action: "minimize" | "toggleMaximize" | "close") => {
    if (inTauri) tauriWindow().then((window) => window[action]());
  };

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar__title" data-tauri-drag-region>
        <span className="titlebar__icon" aria-hidden />
        <span data-tauri-drag-region>Powermise</span>
      </div>
      <button className="caption-button" onClick={() => act("minimize")} aria-label="Minimize">
        <Minus size={14} strokeWidth={1.5} />
      </button>
      <button
        className="caption-button"
        onClick={() => act("toggleMaximize")}
        aria-label={maximized ? "Restore" : "Maximize"}
      >
        {maximized ? <Copy size={12} strokeWidth={1.5} /> : <Square size={11} strokeWidth={1.5} />}
      </button>
      <button className="caption-button close" onClick={() => act("close")} aria-label="Close">
        <X size={15} strokeWidth={1.5} />
      </button>
    </header>
  );
}

function FakeHeaderBar() {
  return (
    <header className="fake-headerbar" aria-hidden>
      Powermise
      <span className="fake-headerbar__close">
        <X size={13} />
      </span>
    </header>
  );
}

function FakeTrafficLights() {
  return (
    <div className="fake-traffic-lights" aria-hidden>
      <span />
      <span />
      <span />
    </div>
  );
}

/** Browser-preview control for seeing each platform's chrome. Hidden in
 *  Tauri (the platform is real) and in the site demo (the page drives it). */
export function PlatformSwitcher() {
  const platform = usePlatform();
  if (inTauri || document.body.classList.contains("embedded")) return null;
  return (
    <div className="platform-switcher">
      Preview as
      <Segmented
        label="Preview platform"
        value={platform}
        options={UI_PLATFORMS}
        onChange={(next) => applyPlatform(next, true)}
      />
    </div>
  );
}
