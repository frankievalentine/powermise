import type { Doctor, MiseInfo, Platform } from "./api";

export interface HealthSummary {
  /** Things that need attention: mise's own errors and warnings, an inactive
   *  shell, and a mise older than recommended. */
  count: number;
  /** `error` when something is broken (mise too old to support, or doctor
   *  errors); `warning` for everything else. */
  severity: "error" | "warning" | null;
}

/** What the Health screen reports, summarized for the sidebar badge. */
export function healthSummary(info: MiseInfo | null, doctor: Doctor | null): HealthSummary {
  const errors = (doctor?.errors.length ?? 0) + (info && !info.supported ? 1 : 0);
  const warnings =
    (doctor?.warnings.length ?? 0) +
    (doctor && !doctor.activated ? 1 : 0) +
    (info && info.supported && !info.recommendedMet ? 1 : 0);
  const count = errors + warnings;
  return { count, severity: errors ? "error" : warnings ? "warning" : null };
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
}

/** With no shell reported (apps on Windows don't get `SHELL`), the platform's
 *  default shell. `misePath` matters when mise lives in `~/.local/bin`, which
 *  isn't on every shell's PATH: the line then names mise by its full path. */
export function activationCommand(
  shellName: string | undefined,
  platform: Platform,
  misePath?: string | null,
): Activation {
  const name = (shellName ?? (platform === "win32" ? "pwsh" : platform === "darwin" ? "zsh" : "bash")).toLowerCase();
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
  const line = `eval "$(${mise} activate zsh)"`;
  return {
    shell: "zsh",
    file: "~/.zshrc",
    prompt: "$",
    command: `zshrc="\${ZDOTDIR:-$HOME}/.zshrc"; mkdir -p "$(dirname "$zshrc")"; ${once(line, '"$zshrc"')}`,
  };
}
