import type { OutdatedTool, ToolVersion } from "./api";

/** Where a tool version comes from, in words. */
export function sourceLabel(version: ToolVersion, miseConfigDir: string, home: string): string {
  const path = version.source?.path;
  if (!path) return "Installed, not in use";
  if (isGlobalPath(path, miseConfigDir)) return "Global";
  return path.startsWith(home) ? `~${path.slice(home.length)}` : path;
}

/** Whether a config path is the global mise config. */
export function isGlobalPath(path: string, miseConfigDir: string): boolean {
  return path.startsWith(`${miseConfigDir}/`) || path.startsWith(`${miseConfigDir}\\`);
}

/** The version an upgrade moves to: past a pin when there is one. */
export function upgradeTarget(entry: OutdatedTool): string {
  return entry.bump ?? entry.latest;
}

/** The home directory, derived from the mise config dir (`~/.config/mise`). */
export function homeFrom(miseConfigDir: string): string {
  return miseConfigDir.replace(/[\\/]\.config[\\/]mise$/, "");
}

/** Short backend label from a registry backend like `aqua:jqlang/jq`. */
export function backendLabel(backend: string): string {
  return backend.split(":")[0];
}
