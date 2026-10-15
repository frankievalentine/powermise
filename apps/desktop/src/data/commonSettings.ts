// The everyday mise settings, in plain language. Each maps to one mise key;
// a setting the installed mise doesn't have is left out automatically.

export type CommonControl =
  | { kind: "switch" }
  /** Writes the option's value as text (`false`, `true`, `7d`, `source`, …). */
  | { kind: "choice"; options: { value: string; label: string }[] }
  | { kind: "stepper"; min: number; max: number; unit: string }
  /** Durations like `30s`; the current value is kept even if not listed. */
  | { kind: "duration"; options: { value: string; label: string }[] }
  /** A list of tool names: suggestions plus anything installed. */
  | { kind: "tools"; suggestions: string[] }
  /** A list of folders, added with the folder picker. */
  | { kind: "folders" };

export interface CommonSetting {
  key: string;
  title: string;
  description: string;
  control: CommonControl;
  /** Extra words people might search for. */
  keywords: string[];
}

export interface CommonGroup {
  title: string;
  settings: CommonSetting[];
}

export const COMMON_SETTINGS: CommonGroup[] = [
  {
    title: "Installing tools",
    settings: [
      {
        key: "auto_install",
        title: "Install missing tools automatically",
        description: "When a command or task needs a tool that isn’t installed yet, mise installs it first.",
        control: { kind: "switch" },
        keywords: ["install", "missing", "automatic"],
      },
      {
        key: "not_found_auto_install",
        title: "Install a tool when a command isn’t found",
        description: "If your terminal can’t find a command, mise installs the tool that provides it.",
        control: { kind: "switch" },
        keywords: ["command not found", "terminal", "install"],
      },
      {
        key: "jobs",
        title: "Installs at the same time",
        description: "How many tools mise downloads and installs at once. Lower it on a slow connection.",
        control: { kind: "stepper", min: 1, max: 16, unit: "at once" },
        keywords: ["parallel", "concurrency", "download", "speed", "jobs"],
      },
      {
        key: "http_timeout",
        title: "Network timeout",
        description: "How long mise waits for a server before giving up. Raise it on a slow or unreliable connection.",
        control: {
          kind: "duration",
          options: [
            { value: "10s", label: "10 seconds" },
            { value: "30s", label: "30 seconds" },
            { value: "1m", label: "1 minute" },
            { value: "2m", label: "2 minutes" },
            { value: "5m", label: "5 minutes" },
          ],
        },
        keywords: ["network", "slow", "download", "internet", "wait"],
      },
      {
        key: "offline",
        title: "Work offline",
        description: "mise uses only installed tools and what it has saved, and doesn’t go online.",
        control: { kind: "switch" },
        keywords: ["offline", "network", "internet", "airplane"],
      },
    ],
  },
  {
    title: "Updates",
    settings: [
      {
        key: "tool_update.global_auto",
        title: "Keep your tools up to date",
        description: "mise updates the tools in your global settings in the background. Project tools aren’t touched.",
        control: {
          kind: "choice",
          options: [
            { value: "false", label: "Off" },
            { value: "true", label: "Every day" },
            { value: "7d", label: "Every week" },
          ],
        },
        keywords: ["update", "upgrade", "automatic", "tools"],
      },
      {
        key: "self_update.auto",
        title: "Keep mise up to date",
        description: "mise installs its own new releases when they come out.",
        control: {
          kind: "choice",
          options: [
            { value: "false", label: "Off" },
            { value: "1d", label: "Check every day" },
            { value: "true", label: "Check every week" },
          ],
        },
        keywords: ["update", "upgrade", "mise", "self update", "automatic"],
      },
    ],
  },
  {
    title: "Projects",
    settings: [
      {
        key: "idiomatic_version_file_enable_tools",
        title: "Use version files from other tools",
        description:
          "Pick tools whose usual version file mise should follow — like .nvmrc for Node.js or .python-version for Python.",
        control: { kind: "tools", suggestions: ["node", "python", "ruby", "go", "java", "terraform", "bun", "deno"] },
        keywords: ["nvmrc", "node-version", "python-version", "ruby-version", "version file", "idiomatic"],
      },
      {
        key: "trusted_config_paths",
        title: "Always-trusted folders",
        description:
          "mise loads project settings inside these folders without asking first. Only add folders whose contents you write yourself.",
        control: { kind: "folders" },
        keywords: ["trust", "security", "folder", "projects"],
      },
      {
        key: "status.show_tools",
        title: "Show tool changes in the terminal",
        description: "When you move between projects in the terminal, mise lists the tool versions that switch.",
        control: { kind: "switch" },
        keywords: ["terminal", "status", "cd", "directory", "show"],
      },
      {
        key: "status.missing_tools",
        title: "Warn about tools that aren’t installed",
        description: "When a project asks for a tool you don’t have, mise can mention it in the terminal.",
        control: {
          kind: "choice",
          options: [
            { value: "never", label: "Never" },
            { value: "if_other_versions_installed", label: "If another version is installed" },
            { value: "always", label: "Always" },
          ],
        },
        keywords: ["warning", "missing", "terminal"],
      },
      {
        key: "disable_tools",
        title: "Tools to ignore",
        description: "mise skips these tools even when a project asks for them.",
        control: { kind: "tools", suggestions: [] },
        keywords: ["disable", "ignore", "skip"],
      },
    ],
  },
  {
    title: "Languages",
    settings: [
      {
        key: "node.corepack",
        title: "Enable Corepack for Node.js",
        description: "Lets pnpm and Yarn use the exact version each project asks for.",
        control: { kind: "switch" },
        keywords: ["node", "pnpm", "yarn", "corepack", "javascript"],
      },
      {
        key: "python.uv_venv_auto",
        title: "Python virtual environments",
        description: "In uv projects, mise can switch to the project’s virtual environment for you.",
        control: {
          kind: "choice",
          options: [
            { value: "false", label: "Off" },
            { value: "source", label: "Use it if it exists" },
            { value: "create|source", label: "Create it if needed, then use it" },
          ],
        },
        keywords: ["python", "venv", "virtualenv", "uv"],
      },
    ],
  },
  {
    title: "Other",
    settings: [
      {
        key: "experimental",
        title: "Experimental features",
        description: "Turn on mise features that are still being built. They may change or go away.",
        control: { kind: "switch" },
        keywords: ["experimental", "beta", "preview"],
      },
    ],
  },
];
