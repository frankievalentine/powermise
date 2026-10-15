// Which operating system's conventions the UI follows. The content is the
// same everywhere; window chrome, materials, shortcut labels, and dialog
// button order adapt. Set on <html data-platform> before the first render.
import { useEffect, useState } from "react";

export type UiPlatform = "darwin" | "win32" | "linux";

export const UI_PLATFORMS: { value: UiPlatform; label: string }[] = [
  { value: "darwin", label: "macOS" },
  { value: "win32", label: "Windows" },
  { value: "linux", label: "Linux" },
];

const STORAGE_KEY = "powermise:platform";
const CHANGE_EVENT = "powermise:platform-change";

function isPlatform(value: unknown): value is UiPlatform {
  return value === "darwin" || value === "win32" || value === "linux";
}

/** The webview's own platform, from its user agent (WKWebView, WebView2, and
 *  WebKitGTK all identify their OS). */
function fromUserAgent(): UiPlatform {
  const agent = navigator.userAgent;
  if (/Mac/i.test(agent)) return "darwin";
  if (/Windows/i.test(agent)) return "win32";
  return "linux";
}

/** In Tauri, always the real platform. In a browser preview, `?platform=` or
 *  the last choice from the switcher can override it. */
export function detectPlatform(inTauri: boolean): UiPlatform {
  if (!inTauri) {
    const query = new URLSearchParams(window.location.search).get("platform");
    if (isPlatform(query)) return query;
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (isPlatform(stored)) return stored;
    } catch {
      // Storage can be unavailable (privacy modes); fall through.
    }
  }
  return fromUserAgent();
}

export function currentPlatform(): UiPlatform {
  const value = document.documentElement.dataset.platform;
  return isPlatform(value) ? value : fromUserAgent();
}

export function applyPlatform(platform: UiPlatform, remember = false) {
  document.documentElement.dataset.platform = platform;
  if (remember) {
    try {
      localStorage.setItem(STORAGE_KEY, platform);
    } catch {
      // Not remembering is fine.
    }
  }
  window.dispatchEvent(new CustomEvent(CHANGE_EVENT, { detail: platform }));
}

export function setPlatformFromMessage(value: unknown) {
  if (isPlatform(value)) applyPlatform(value);
}

export function usePlatform(): UiPlatform {
  const [platform, setPlatform] = useState(currentPlatform);
  useEffect(() => {
    const onChange = () => setPlatform(currentPlatform());
    window.addEventListener(CHANGE_EVENT, onChange);
    return () => window.removeEventListener(CHANGE_EVENT, onChange);
  }, []);
  return platform;
}

/** A keyboard shortcut as each platform writes it: ⌘F on macOS, Ctrl+F elsewhere. */
export function shortcutLabel(platform: UiPlatform, key: string): string {
  return platform === "darwin" ? `⌘${key.toUpperCase()}` : `Ctrl+${key.toUpperCase()}`;
}

/** Whether a keyboard event uses the platform's primary modifier. */
export function hasPrimaryModifier(event: KeyboardEvent, platform: UiPlatform): boolean {
  return platform === "darwin" ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
