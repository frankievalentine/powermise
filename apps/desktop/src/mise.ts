import type { OutdatedTool, Platform, ToolVersion } from "./api";
import registryOs from "./data/registry-os.json";

/** mise's OS names → powermise platforms (WSL runs Linux tools). */
const MISE_OS: Record<string, Platform[]> = { linux: ["linux", "wsl"], macos: ["darwin"], windows: ["win32"] };
const LIMITED = (registryOs as { os: Record<string, string[]> }).os;

/** The platforms a registry tool installs on, or `null` when it has no limit.
 *  From mise's registry `os` lists (scripts/registry-os.py); mise skips the
 *  tool anywhere else. */
export function toolPlatforms(short: string): Platform[] | null {
  const systems = LIMITED[short];
  return systems ? systems.flatMap((system) => MISE_OS[system] ?? []) : null;
}

/** Whether mise installs this registry tool on `platform`. */
export function runsOn(short: string, platform: Platform): boolean {
  return toolPlatforms(short)?.includes(platform) ?? true;
}

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

/** mise's core tools: languages whose installers are built in
 *  (https://mise.jdx.dev/core-tools.html). */
export const CORE_TOOLS = ["bun", "deno", "dotnet", "elixir", "erlang", "go", "java", "node", "python", "ruby", "rust", "swift", "zig"];

/** Short backend label from a registry backend like `aqua:jqlang/jq`. */
export function backendLabel(backend: string): string {
  const name = backend.split(":")[0];
  // Core tools' installers are part of mise itself.
  return name === "core" ? "built in" : name;
}
