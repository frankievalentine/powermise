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

/** The home directory: as reported, or else from the mise config dir (`~/.config/mise`). */
export function homeFrom(env: { home: string | null; miseConfigDir: string }): string {
  return env.home ?? env.miseConfigDir.replace(/[\\/]\.config[\\/]mise$/, "");
}

/** A path with the home directory shown as `~`: `/Users/me`, `/home/me`, or
 *  `C:\Users\me` (or the given home, when it's somewhere else). */
export function prettyPath(path: string, home?: string | null): string {
  if (home && path.startsWith(home)) return `~${path.slice(home.length)}`;
  return path.replace(/^(\/(Users|home)\/[^/]+|[A-Za-z]:\\Users\\[^\\]+)/, "~");
}

/** Short backend label from a registry backend like `aqua:jqlang/jq`. */
export function backendLabel(backend: string): string {
  return backend.split(":")[0];
}
