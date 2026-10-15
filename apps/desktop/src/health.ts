import type { Doctor, MiseInfo, Platform, SystemCheck } from "./api";

export interface HealthSummary {
  /** Things that need attention: mise's own errors and warnings, an inactive
   *  shell, and a mise older than recommended. */
  count: number;
  /** `error` when something is broken (mise too old to support, or doctor
   *  errors); `warning` for everything else. */
  severity: "error" | "warning" | null;
}

/** What the Health screen reports, summarized for the sidebar badge. */
export function healthSummary(
  info: MiseInfo | null,
  doctor: Doctor | null,
  shellConflicts = 0,
  systemIssues = 0,
): HealthSummary {
  const errors = (doctor?.errors.length ?? 0) + (info && !info.supported ? 1 : 0);
  const warnings =
    (doctor?.warnings.length ?? 0) +
    shellConflicts +
    systemIssues +
    (doctor && !doctor.activated ? 1 : 0) +
    (info && info.supported && !info.recommendedMet ? 1 : 0);
  const count = errors + warnings;
  return { count, severity: errors ? "error" : warnings ? "warning" : null };
}

/** Developer basics that are missing. */
export function systemIssues(checks: SystemCheck[] | null): number {
  return (checks ?? []).filter((check) => !check.ok).length;
}

/** How to turn mise on in a shell: mise's own setup commands
 *  (https://mise.jdx.dev/shell-setup.html), which add the line only once and
 *  create the startup file if needed. */
export interface Activation {
  /** For prose: "zsh", "PowerShell", … */
  shell: string;
  /** The startup file, as written in the command. */
  file: string;
  /** The command to run once in that shell. */
  command: string;
  /** The prompt shown before the command. */
  prompt: string;
  /** Something else worth knowing for this shell. */
  note?: string;
  /** A second command some setups need (bash on macOS). */
  extra?: { text: string; command: string };
  /** A shell powermise has no steps for: point to mise's guide instead. */
  unsupported?: boolean;
}

/** mise's own guide to turning it on in every shell it supports. */
export const SHELL_SETUP_URL = "https://mise.jdx.dev/shell-setup.html";

/** With no shell reported (apps on Windows don't get `SHELL`), the platform's
 *  default shell. `misePath` matters when mise lives in `~/.local/bin`, which
 *  isn't on every shell's PATH: the line then names mise by its full path. */
export function activationCommand(
  shellName: string | undefined,
  platform: Platform,
  misePath?: string | null,
): Activation {
  const name = (shellName ?? (platform === "win32" ? "pwsh" : platform === "darwin" ? "zsh" : "bash"))
    .toLowerCase()
    .split(/[\\/]/)
    .pop()!;
  const local = platform !== "win32" && !!misePath && /[\\/]\.local[\\/]bin[\\/]mise$/.test(misePath);
  const mise = local ? "~/.local/bin/mise" : "mise";
  const once = (line: string, file: string) =>
    `grep -qxF '${line}' ${file} 2>/dev/null || echo '${line}' >> ${file}`;

  if (name.includes("pwsh") || name.includes("powershell")) {
    const line = "(&mise activate pwsh) | Out-String | Invoke-Expression";
    return {
      shell: "PowerShell",
      file: "$PROFILE",
      prompt: "PS>",
      command: `if (-not (Test-Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force | Out-Null }; $line = '${line}'; if (-not (Select-String -Path $PROFILE -SimpleMatch $line -Quiet)) { Add-Content $PROFILE $line }`,
      note: "Windows PowerShell 5.1 picks up changes at the next prompt; PowerShell 7 and newer switch versions as soon as you change folders.",
    };
  }
  if (name.endsWith("cmd") || name.endsWith("cmd.exe")) {
    return {
      shell: "Command Prompt",
      file: "your user Path",
      prompt: "PS>",
      command: `$shims = "$env:LOCALAPPDATA\\mise\\shims"; $userPath = [Environment]::GetEnvironmentVariable('Path', 'User'); if (($userPath -split ';') -notcontains $shims) { [Environment]::SetEnvironmentVariable('Path', "$shims;$userPath", 'User') }`,
      note: "Command Prompt can’t turn mise on, so this puts mise’s shims on your Path instead. Run it in PowerShell, then open a new window.",
    };
  }
  if (name.includes("fish")) {
    const line = `${mise} activate fish | source`;
    const file = "~/.config/fish/config.fish";
    return {
      shell: "fish",
      file,
      prompt: "$",
      command: `mkdir -p ~/.config/fish; grep -qxF '${line}' ${file} 2>/dev/null; or echo '${line}' >> ${file}`,
      note: "If fish came from Homebrew, mise may already turn itself on there; open a new window first to check.",
    };
  }
  if (name === "nu" || name === "nu.exe" || name === "nushell") {
    // mise's Nushell setup: env.nu writes the mise module at every start (so it
    // follows mise upgrades), and config.nu loads it.
    const run = local ? '^($env.HOME | path join ".local/bin/mise")' : "^mise";
    const lines = ["let mise_path = $nu.default-config-dir | path join mise.nu", `${run} activate nu | save $mise_path --force`];
    const use = "use ($nu.default-config-dir | path join mise.nu)";
    return {
      shell: "Nushell",
      file: "env.nu and config.nu",
      prompt: ">",
      command:
        `let lines = [${lines.map((line) => `'${line}'`).join(", ")}]; let use = '${use}'; ` +
        "mkdir ($nu.env-path | path dirname); " +
        'if not (($nu.env-path | path exists) and (open --raw $nu.env-path | str contains ($lines | last))) { $"\\n($lines | str join "\\n")\\n" | save --append $nu.env-path }; ' +
        'if not (($nu.config-path | path exists) and (open --raw $nu.config-path | str contains $use)) { $"\\n($use)\\n" | save --append $nu.config-path }',
    };
  }
  if (name.includes("bash")) {
    const line = `eval "$(${mise} activate bash)"`;
    return {
      shell: "bash",
      file: "~/.bashrc",
      prompt: "$",
      command: once(line, "~/.bashrc"),
      extra:
        platform === "darwin"
          ? {
              text: "Terminal on macOS starts bash as a login shell, which reads ~/.bash_profile. If yours doesn’t load ~/.bashrc already, run this too:",
              command: once("[ -f ~/.bashrc ] && . ~/.bashrc", "~/.bash_profile"),
            }
          : undefined,
    };
  }
  if (!name.includes("zsh")) {
    return { shell: name, file: "", prompt: "$", command: "", unsupported: true };
  }
  const line = `eval "$(${mise} activate zsh)"`;
  return {
    shell: "zsh",
    file: "~/.zshrc",
    prompt: "$",
    command: `zshrc="\${ZDOTDIR:-$HOME}/.zshrc"; mkdir -p "$(dirname "$zshrc")"; ${once(line, '"$zshrc"')}`,
  };
}
