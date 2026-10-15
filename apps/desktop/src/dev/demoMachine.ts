// Demo state for machine-wide screens: settings, GitHub downloads, versions
// from other managers, and installing mise (`?nomise` starts without it).
import { enableIdiomatic } from "./demoProjects";
import type {
  DetectedImport,
  GithubStatus,
  InstallerOption,
  MiseAction,
  Setting,
} from "../api";

const GLOBAL_CONFIG = "/Users/demo/.config/mise/config.toml";

/** Whether the demo machine has mise. `?nomise` shows the first-run install. */
export let demoHasMise = !new URLSearchParams(globalThis.location?.search ?? "").has("nomise");

const setting = (
  key: string,
  type: string,
  value: unknown,
  description: string,
  extra: Partial<Setting> = {},
): Setting => ({ key, type, value, description, source: null, deprecated: false, ...extra });

const settings: Setting[] = [
  setting("jobs", "number", 8, "How many jobs to run concurrently such as tool installs.", {
    source: GLOBAL_CONFIG,
  }),
  setting("experimental", "boolean", false, "Enable experimental features, which may change or be removed in any release."),
  setting("auto_install", "boolean", true, "Install missing tools automatically when running commands like `mise exec` or `mise run`."),
  setting("not_found_auto_install", "boolean", true, "Install a missing tool when its command isn't found in an activated shell."),
  setting("http_timeout", "number", "30s", "Timeout for connecting or waiting between reads during HTTP requests."),
  setting("offline", "boolean", false, "Block every HTTP request, so mise uses only installed tools and cached data."),
  setting("status.missing_tools", "string", "if_other_versions_installed", "When to warn that tools requested by config are not installed."),
  setting("idiomatic_version_file_enable_tools", "array", [], "Tools whose idiomatic version files (like .nvmrc) mise reads."),
  setting("trusted_config_paths", "array", [], "Config paths mise trusts without asking."),
  setting("disable_tools", "array", [], "Tools mise ignores even when a config asks for them."),
  setting("always_keep_download", "boolean", false, "Keep downloaded archives and sources after installing, for debugging.", {
    deprecated: true,
  }),
  setting("node.corepack", "boolean", false, "Enable corepack after installing Node.js, so pnpm and yarn match the project."),
  setting("node.compile", "boolean | string", false, "Build Node.js from source instead of downloading a prebuilt binary."),
  setting("python.uv_venv_auto", "boolean | string", false, "Create and activate a uv virtual environment for projects that have a uv.lock."),
  setting("python.default_packages_file", "string", "~/.default-python-packages", "Packages to install with every new Python version."),
  setting("go.default_packages_file", "string", "~/.default-go-packages", "Packages to install with every new Go version."),
  setting("github.credential_command", "string", "", "Command that prints a GitHub token on stdout."),
  setting("github.use_git_credentials", "boolean", false, "Get GitHub tokens from your Git credential helpers with `git credential fill`."),
  setting("github.gh_cli_tokens", "boolean", true, "Read GitHub tokens that the gh CLI stores in its `hosts.yml` file."),
  setting("github.github_attestations", "boolean", true, "Verify GitHub artifact attestations for github backend tools."),
  setting("status.show_tools", "boolean", false, "Show which tools are active when you change directories."),
  setting("self_update.auto", "boolean", false, "Update mise itself automatically in the background."),
  setting("tool_update.global_auto", "boolean", false, "Upgrade global tools automatically in the background."),
];

let cacheBytes = 412_000_000;

/** `mise bootstrap status`, as the demo machine has it. */
const machineSections = [
  {
    id: "packages",
    items: [
      { name: "brew:git", state: "installed", detail: "2.51.0" },
      { name: "brew:wget", state: "missing", detail: "latest" },
    ],
  },
  {
    id: "dotfiles",
    items: [
      { name: "~/.gitconfig", state: "applied", detail: "symlink" },
      { name: "~/.zshrc", state: "missing", detail: "symlink" },
    ],
  },
  { id: "repos", items: [{ name: "~/code/dotfiles", state: "missing", detail: "github.com/demo/dotfiles" }] },
  { id: "macos_defaults", items: [{ name: "com.apple.dock autohide", state: "applied", detail: "true" }] },
];
const dotHistory = { enabled: true, trackedFiles: 12, checkpoints: 4, latest: null, unsaved: true as unknown, pendingOperations: 0 };
let dotOrigin: string | null = null;
/** macOS preferences: what powermise's file manages, and what "macOS" has. */
const macManaged = new Map<string, boolean | number | string>([["com.apple.dock tilesize", 36]]);
const macCurrent = new Map<string, string>([
  ["com.apple.dock tilesize", "48"],
  ["com.apple.dock autohide", "0"],
  ["com.apple.finder AppleShowAllFiles", "0"],
]);
const asDefaultsRead = (value: boolean | number | string) =>
  typeof value === "boolean" ? (value ? "1" : "0") : String(value);
const customAliases: Record<string, Record<string, string>> = {};
const BUILTIN_ALIASES: Record<string, Record<string, string>> = {
  node: { lts: "24", "lts-iron": "20", "lts-jod": "22" },
  java: { lts: "25" },
};
const checkpoints = [
  { id: 4, createdAt: "2026-10-14T21:02:00-07:00", trigger: "save", summary: "edited ~/.zshrc", machine: "studio", changed: ["~/.zshrc"] },
  { id: 3, createdAt: "2026-10-13T09:15:00-07:00", trigger: "edit", summary: "edited ~/.gitconfig", machine: "laptop", changed: ["~/.gitconfig"] },
  { id: 2, createdAt: "2026-10-10T18:40:00-07:00", trigger: "baseline", summary: "tracked ~/.zshrc", machine: "studio", changed: [] },
];
const trusted = ["/Users/demo/code/web-app", "/Users/demo/code/old-prototype"];

/** mise's defaults, for `settings unset`. */
const defaults = new Map(settings.map((entry) => [entry.key, entry.source ? defaultFor(entry) : entry.value]));
function defaultFor(entry: Setting): unknown {
  return entry.key === "jobs" ? 8 : entry.value;
}

/** How mise stores `mise settings set <key> <text>` for a setting's type. */
function parseSettingValue(entry: Setting, text: string): unknown {
  if (entry.type === "array") return text ? text.split(",").map((item) => item.trim()).filter(Boolean) : [];
  if (text === "true" || text === "false") return text === "true";
  if (entry.type === "number" && /^-?\d+(\.\d+)?$/.test(text)) return Number(text);
  return text;
}

const imports: DetectedImport[] = [
  { source: "nvm", tool: "node", versions: ["18.20.4", "20.11.1"] },
  { source: "pyenv", tool: "python", versions: ["3.11.9"] },
];

const installers: InstallerOption[] = [
  { id: "homebrew", label: "Homebrew", command: "brew install mise" },
  { id: "script", label: "mise's install script", command: "curl -fsSL https://mise.run | sh" },
];

function githubStatus(): GithubStatus {
  const credentialCommand = String(settings.find((entry) => entry.key === "github.credential_command")?.value ?? "");
  const useGitCredentials = settings.find((entry) => entry.key === "github.use_git_credentials")?.value === true;
  const signedIn = credentialCommand === "gh auth token" || useGitCredentials;
  return {
    token: {
      host: "github.com",
      token: signedIn ? "gho_…k2Qa" : null,
      source: signedIn ? (credentialCommand ? "github.credential_command" : "git credential fill") : null,
    },
    ghSignedIn: true,
    credentialCommand,
    useGitCredentials,
  };
}

const SYNC: Record<string, string> = {
  nvm: "node --nvm",
  nodenv: "node --nodenv",
  "node-brew": "node --brew",
  pyenv: "python --pyenv",
  uv: "python --uv",
  "ruby-brew": "ruby --brew",
};

export function describeMachineAction(action: MiseAction): string | null {
  switch (action.kind) {
    case "setSetting":
      return `mise settings set ${action.key} ${/\s/.test(action.value) ? `'${action.value}'` : action.value}`;
    case "unsetSetting":
      return `mise settings unset ${action.key}`;
    case "syncVersions":
      return `mise sync ${SYNC[action.source]}`;
    case "installMise":
      return installers.find((entry) => entry.id === action.installer)?.command ?? "brew install mise";
    case "untrust":
      return `mise untrust ${action.path}`;
    case "cachePrune":
      return "mise cache prune --yes";
    case "cacheClear":
      return "mise cache clear --yes";
    case "reshim":
      return "mise reshim";
    case "link":
      return `mise link ${action.tool}@${action.version} ${action.path}`;
    case "addSetting":
      return `mise settings add ${action.key} ${action.value}`;
    case "bootstrapPreview":
      return `mise bootstrap --dry-run${action.only ? ` --only ${action.only}` : ""}`;
    case "bootstrapApply":
      return `mise bootstrap --yes${action.only ? ` --only ${action.only}` : ""}`;
    case "dotfilesSave":
      return "mise dotfiles save";
    case "dotfilesSync":
      return "mise dotfiles sync";
    case "dotfilesOriginSet":
      return `mise dotfiles origin set ${action.url}`;
    case "dotfilesRollback":
      return `mise dotfiles rollback --to ${action.checkpoint} --all ${action.dryRun ? "--dry-run" : "--yes"}`;
    case "dotfilesUndo":
      return "mise dotfiles undo --yes";
    case "relaunchApp":
      return `killall ${action.app.includes(" ") ? `'${action.app}'` : action.app}`;
    case "setToolAlias":
      return `mise tool-alias set ${action.tool} ${action.alias} ${action.version}`;
    case "unsetToolAlias":
      return `mise tool-alias unset ${action.tool} ${action.alias}`;
    default:
      return null;
  }
}

/** Applies a machine action to the demo state; null when it isn't one. */
export function performMachineAction(action: MiseAction): string[] | null {
  switch (action.kind) {
    case "setSetting": {
      const entry = settings.find((candidate) => candidate.key === action.key);
      if (entry) {
        entry.value = parseSettingValue(entry, action.value);
        entry.source = GLOBAL_CONFIG;
      }
      return [];
    }
    case "unsetSetting": {
      const entry = settings.find((candidate) => candidate.key === action.key);
      if (entry) {
        entry.value = defaults.get(entry.key);
        entry.source = null;
      }
      return [];
    }
    case "syncVersions": {
      const index = imports.findIndex((entry) => entry.source === action.source);
      const [found] = index >= 0 ? imports.splice(index, 1) : [];
      return (found?.versions ?? []).map((version) => `mise linked ${found.tool}@${version}`);
    }
    case "untrust": {
      const index = trusted.indexOf(action.path);
      if (index >= 0) trusted.splice(index, 1);
      return [`mise untrusted ${action.path.replace("/Users/demo", "~")}`];
    }
    case "cachePrune": {
      const freed = Math.round(cacheBytes * 0.6);
      cacheBytes -= freed;
      return [`mise cache pruned 214 files, ${Math.round(freed / 1_000_000)} MB`];
    }
    case "cacheClear":
      cacheBytes = 0;
      return ["mise cache cleared"];
    case "reshim":
      return [];
    case "link":
      return [`mise linked ${action.tool}@${action.version} → ${action.path}`];
    case "bootstrapPreview": {
      const parts = action.only ? machineSections.filter((section) => section.id === action.only) : machineSections;
      return parts.flatMap((section) => [
        `mise bootstrap: ${section.id}`,
        ...section.items.filter((item) => item.state === "missing").map((item) => `would apply ${item.name}`),
      ]);
    }
    case "relaunchApp":
      return [];
    case "bootstrapApply": {
      const lines: string[] = [];
      if (!action.only || action.only === "macos-defaults") {
        lines.push("mise bootstrap: system defaults");
        for (const [key, value] of macManaged) {
          if (macCurrent.get(key) === asDefaultsRead(value)) continue;
          macCurrent.set(key, asDefaultsRead(value));
          const [domain, name] = key.split(" ");
          lines.push(`defaults write ${domain} ${name} -${typeof value === "boolean" ? "bool" : typeof value === "number" ? (Number.isInteger(value) ? "int" : "float") : "string"} ${value}`);
        }
        if (action.only) return lines;
      }
      for (const section of machineSections.filter((entry) => !action.only || entry.id === action.only)) {
        lines.push(`mise bootstrap: ${section.id}`);
        for (const item of section.items.filter((entry) => entry.state === "missing")) {
          item.state = section.id === "packages" ? "installed" : "applied";
          lines.push(`mise ${section.id}: applied ${item.name}`);
        }
      }
      return lines;
    }
    case "dotfilesSave":
      dotHistory.checkpoints += 1;
      dotHistory.unsaved = null;
      return ["mise history: saved checkpoint 5 (12 files)"];
    case "dotfilesSync":
      return ["mise history: pushed 1 checkpoint", "mise history: nothing to fetch"];
    case "dotfilesRollback":
      return action.dryRun
        ? ["~/.zshrc  write  a file 8395334  a file 8538864", "history: dry run; nothing was changed"]
        : ["mise history: saved checkpoint 5 before rolling back", `mise history: restored checkpoint ${action.checkpoint}`];
    case "dotfilesUndo":
      return ["mise history: undid the latest operation"];
    case "setToolAlias":
      customAliases[action.tool] = { ...customAliases[action.tool], [action.alias]: action.version };
      return [];
    case "unsetToolAlias":
      delete customAliases[action.tool]?.[action.alias];
      return [];
    case "dotfilesOriginSet":
      dotOrigin = action.url;
      return [`mise history: origin set to ${action.url}`];
    case "addSetting": {
      const entry = settings.find((candidate) => candidate.key === action.key);
      if (entry && Array.isArray(entry.value)) {
        entry.value = [...entry.value, action.value];
        entry.source = GLOBAL_CONFIG;
      }
      if (action.key === "idiomatic_version_file_enable_tools") enableIdiomatic(action.value);
      return [];
    }
    case "installMise":
      demoHasMise = true;
      return [
        "==> Downloading https://ghcr.io/v2/homebrew/core/mise/manifests/2026.10.10",
        "==> Pouring mise--2026.10.10.arm64_sequoia.bottle.tar.gz",
        "🍺  /opt/homebrew/Cellar/mise/2026.10.10: 12 files, 48.2MB",
      ];
    default:
      return null;
  }
}

export const NOT_A_MACHINE_COMMAND = Symbol("not a machine command");

export function demoMachine(command: string, args: Record<string, unknown> = {}): unknown {
  switch (command) {
    case "settings":
      return settings.map((entry) => ({ ...entry }));
    case "github_status":
      return githubStatus();
    case "detect_imports":
      return imports.map((entry) => ({ ...entry }));
    case "mise_installers":
      return installers;
    case "open_url":
      return undefined;
    case "disk_usage":
      return {
        cacheDir: "/Users/demo/Library/Caches/mise",
        cacheBytes,
        installsDir: "/Users/demo/.local/share/mise/installs",
        installsBytes: 2_840_000_000,
      };
    case "trusted_folders":
      return {
        trusted: trusted.map((path) => ({ path, exists: !path.endsWith("old-prototype") })),
        ignored: [],
      };
    case "machine_status":
      return {
        sections: machineSections.map((section) => ({ ...section, items: section.items.map((item) => ({ ...item })) })),
        history: { ...dotHistory },
        origin: dotOrigin,
        supportsHistory: true,
        checkpoints: checkpoints.map((checkpoint) => ({ ...checkpoint })),
      };
    case "resolve_dotfile_conflicts":
      return undefined;
    case "tool_aliases": {
      const tool = args.tool as string;
      return [
        ...Object.entries(BUILTIN_ALIASES[tool] ?? {}).map(([alias, version]) => ({ alias, version, custom: false })),
        ...Object.entries(customAliases[tool] ?? {}).map(([alias, version]) => ({ alias, version, custom: true })),
      ];
    }
    case "plugins":
      return [];
    case "macos_settings": {
      const keys = args.keys as { domain: string; key: string }[];
      return {
        managed: [...macManaged].map(([key, value]) => {
          const [domain, name] = key.split(" ");
          return { domain, key: name, value };
        }),
        current: keys.map((entry) => macCurrent.get(`${entry.domain} ${entry.key}`) ?? null),
        fragmentPath: "/Users/demo/.config/mise/conf.d/powermise-macos.toml",
      };
    }
    case "set_macos_default": {
      const key = `${args.domain} ${args.key}`;
      if (args.value === null) macManaged.delete(key);
      else macManaged.set(key, args.value as boolean | number | string);
      return [...macManaged].map(([entry, value]) => {
        const [domain, name] = entry.split(" ");
        return { domain, key: name, value };
      });
    }
    case "pick_folder":
      return "/Users/demo/src/node-v24.11.0";
    default:
      return NOT_A_MACHINE_COMMAND;
  }
}
