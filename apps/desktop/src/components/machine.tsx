import { useState } from "react";
import { AlertTriangle, CheckCircle2, ExternalLink, Github, Info, Loader2, Trash2 } from "lucide-react";
import {
  api,
  type DetectedImport,
  type ImportSource,
  type InstalledProgram,
  type RegistryEntry,
  type ShellConflict,
  type SystemCheck,
} from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import { usePlatform } from "../platform";
import { SHELL_SETUP_URL, type Activation } from "../health";
import { isGlobalPath, prettyPath } from "../mise";
import { CommandBlock } from "./ui";
import { Sheet } from "./Sheet";

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

/** Hosts the app opens links to (the backend enforces the same list). */
const LINK_HOSTS = ["mise.jdx.dev", "github.com", "macos-defaults.com", "skills.sh", "docs.devin.ai"];

/** Whether `url` is a link the app can open in the browser. */
export function canOpen(url: string | null | undefined): url is string {
  if (!url) return false;
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && LINK_HOSTS.includes(parsed.hostname);
  } catch {
    return false;
  }
}

/** A link to mise's docs or GitHub, opened in the default browser. */
export function ExternalLinkButton({ url, children }: { url: string; children: React.ReactNode }) {
  return (
    <button className="link-button" onClick={() => void api.openUrl(url)}>
      {children} <ExternalLink size={11} aria-hidden />
    </button>
  );
}

// ---------------------------------------------------------------------------
// Clean up: versions no config uses
// ---------------------------------------------------------------------------

/** Lists installed versions no config uses, then deletes them with
 *  `mise prune --tools --yes`. */
export function CleanupSheet({
  open,
  onDismiss,
  context,
}: {
  open: boolean;
  onDismiss: () => void;
  context: AppContext;
}) {
  const prunable = useAsync(
    () => (open ? api.prunableVersions() : Promise.resolve([])),
    [open, context.mise.revision],
  );
  const versions = prunable.data ?? [];
  const total = versions.reduce((sum, entry) => sum + entry.bytes, 0);

  return (
    <Sheet open={open} onDismiss={onDismiss} labelledBy="cleanup-title">
      <div className="sheet-head">
        <h2 id="cleanup-title">Clean up unused versions</h2>
        <p>
          Installed versions that no mise config uses — not your global config, a project you’ve opened, or an AI
          setup. Deleting them frees disk space; mise reinstalls any version a config asks for later.
        </p>
      </div>
      <div className="sheet-body">
        {prunable.loading && !prunable.data && (
          <p className="field-hint">
            <Loader2 size={12} className="spin" /> Asking mise…
          </p>
        )}
        {prunable.error && <div className="callout error selectable">{prunable.error}</div>}
        {prunable.data && versions.length === 0 && (
          <div className="callout info">
            <CheckCircle2 size={14} color="var(--green)" />
            <span>Nothing to clean up. Every installed version is in use.</span>
          </div>
        )}
        {versions.length > 0 && (
          <>
            <div className="list">
              {versions.map((entry) => (
                <div className="list-row" key={`${entry.tool}@${entry.version}`} style={{ minHeight: 36 }}>
                  <div className="row-main">
                    <div className="row-title">
                      {entry.tool} <span className="mono">{entry.version}</span>
                    </div>
                  </div>
                  <span className="row-subtitle">{formatBytes(entry.bytes)}</span>
                </div>
              ))}
            </div>
            <p className="section-note" style={{ marginTop: 10 }}>
              {versions.length} {versions.length === 1 ? "version" : "versions"} · {formatBytes(total)}
            </p>
            <CommandBlock command="mise prune --tools --yes" />
          </>
        )}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onDismiss}>
          {versions.length ? "Cancel" : "Done"}
        </button>
        {versions.length > 0 && (
          <button
            className="button large danger"
            onClick={() => {
              onDismiss();
              context.run({
                action: { kind: "pruneTools" },
                title: "Cleaning up",
                description: "",
                verb: "Clean Up",
                autoStart: true,
              });
            }}
          >
            <Trash2 size={14} /> Delete {versions.length} {versions.length === 1 ? "Version" : "Versions"}
          </button>
        )}
      </div>
    </Sheet>
  );
}

// ---------------------------------------------------------------------------
// GitHub downloads
// ---------------------------------------------------------------------------

/** Whether mise downloads from GitHub signed in, and ways to sign it in
 *  with a login the person already has. */
export function GithubCard({ context }: { context: AppContext }) {
  const { mise, run } = context;
  const status = useAsync(() => api.githubStatus(), [mise.revision]);
  const data = status.data;

  if (status.loading && !data) {
    return (
      <p className="field-hint">
        <Loader2 size={12} className="spin" /> Checking…
      </p>
    );
  }
  if (status.error) return <div className="callout error selectable">{status.error}</div>;
  if (!data) return null;

  const signedIn = !!data.token.token;
  return (
    <div className="list prose">
      <div className="list-row">
        {signedIn ? (
          <CheckCircle2 size={16} className="outcome-icon succeeded" />
        ) : (
          <Github size={16} color="var(--text-tertiary)" />
        )}
        <div className="row-main">
          <div className="row-title">
            {signedIn ? "Downloads from GitHub are signed in" : "Downloads from GitHub aren’t signed in"}
          </div>
          <div className="row-subtitle">
            {signedIn
              ? `mise uses ${data.token.token} from ${data.token.source ?? "an unknown source"}.`
              : "Many tools download from GitHub, which allows only 60 anonymous requests an hour. Installs can fail with a rate limit until mise has a token."}
          </div>
        </div>
      </div>
      {!signedIn && (
        <>
          <div className="list-row">
            <div className="row-main">
              <div className="row-title">Use your GitHub CLI login</div>
              <div className="row-subtitle">
                {data.ghSignedIn
                  ? "The GitHub CLI (gh) is signed in on this computer. mise can ask it for a token whenever it needs one."
                  : "Install the GitHub CLI and run “gh auth login”, then come back to use it here."}
              </div>
            </div>
            <button
              className="button"
              disabled={!data.ghSignedIn}
              onClick={() =>
                run({
                  action: { kind: "setSetting", key: "github.credential_command", value: "gh auth token" },
                  title: "Use your GitHub CLI login?",
                  description:
                    "mise will run “gh auth token” when it downloads from GitHub. The token stays in the GitHub CLI; nothing is copied into your mise config.",
                  verb: "Use gh",
                })
              }
            >
              Use gh
            </button>
          </div>
          <div className="list-row">
            <div className="row-main">
              <div className="row-title">Use Git’s saved GitHub login</div>
              <div className="row-subtitle">
                If Git already remembers a GitHub password or token (for example in your system keychain), mise can
                ask Git for it.
              </div>
            </div>
            <button
              className="button"
              disabled={data.useGitCredentials}
              onClick={() =>
                run({
                  action: { kind: "setSetting", key: "github.use_git_credentials", value: "true" },
                  title: "Use Git’s saved GitHub login?",
                  description: "mise will ask your Git credential helper for a GitHub token when it needs one.",
                  verb: "Turn On",
                })
              }
            >
              {data.useGitCredentials ? "On" : "Turn On"}
            </button>
          </div>
        </>
      )}
      {!signedIn && (data.credentialCommand || data.useGitCredentials) && (
        <div className="list-row">
          <Info size={16} color="var(--orange)" />
          <div className="row-main">
            <div className="row-subtitle">
              mise is set to borrow a login, but didn’t find one. Check that{" "}
              {data.credentialCommand ? `“${data.credentialCommand}”` : "Git"} has a GitHub login.
            </div>
          </div>
        </div>
      )}
      <div className="list-row">
        <div className="row-main">
          <div className="row-subtitle">
            A token set in your shell (like GITHUB_TOKEN in ~/.zshrc) only reaches mise in the terminal, not this
            app. <ExternalLinkButton url="https://mise.jdx.dev/dev-tools/github-tokens.html">More about GitHub tokens</ExternalLinkButton>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Bring over versions from other managers
// ---------------------------------------------------------------------------

export const SOURCE_LABEL: Record<ImportSource, string> = {
  nvm: "nvm",
  nodenv: "nodenv",
  "node-brew": "Homebrew",
  pyenv: "pyenv",
  uv: "uv",
  "ruby-brew": "Homebrew",
  fnm: "fnm",
  volta: "Volta",
  n: "n",
  "nvm-windows": "nvm for Windows",
  rbenv: "rbenv",
  rvm: "RVM",
  chruby: "chruby",
  goenv: "goenv",
  asdf: "asdf",
};

const TOOL_LABEL: Record<string, string> = { node: "Node.js", python: "Python", ruby: "Ruby", go: "Go" };
export const toolLabel = (tool: string) => TOOL_LABEL[tool] ?? tool;

/** The version of `tool` your global mise config uses, if any. */
export function globalVersion(context: AppContext, tool: string): string | undefined {
  return context.mise.inventory?.[tool]?.find(
    (version) => version.source && isGlobalPath(version.source.path, context.env.miseConfigDir),
  )?.version;
}

/** What's left to do for an import: versions mise doesn't have, and whether
 *  the manager's default isn't your mise default yet. */
export function importTodo(context: AppContext, entry: DetectedImport) {
  const missing = entry.versions.filter((version) => !version.inMise);
  const current = globalVersion(context, entry.tool);
  const offerDefault = !!entry.defaultVersion && current !== entry.defaultVersion;
  return { missing, current, offerDefault };
}

/** Imports with something left to do — for checklists and badges. */
export function pendingImports(context: AppContext, imports: DetectedImport[]): DetectedImport[] {
  return imports.filter((entry) => {
    const todo = importTodo(context, entry);
    return todo.missing.length > 0 || todo.offerDefault;
  });
}

/** Startup-file lines that still turn on another version manager. */
export function ShellConflictRows({ conflicts }: { conflicts: ShellConflict[] }) {
  return (
    <>
      {conflicts.map((conflict) => (
        <div className="list-row setting-row" key={`${conflict.file}:${conflict.line}`}>
          <AlertTriangle size={16} color="var(--orange)" />
          <div className="row-main">
            <div className="row-title">
              {conflict.manager} is still turned on in {prettyPath(conflict.file)}
            </div>
            <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
              Line {conflict.line} changes your PATH too, so your terminal can pick {conflict.manager}’s version instead
              of mise’s. Once you’ve brought your versions over, remove or comment out the line, then open a new
              terminal window.
            </div>
            <div style={{ marginTop: 8 }}>
              <CommandBlock command={conflict.text} prompt={`${conflict.line}`} />
            </div>
          </div>
          <button className="button" onClick={() => void api.reveal(conflict.file)} title="Show the file">
            Show File
          </button>
        </div>
      ))}
    </>
  );
}

/** Developer basics, each with its fix: in powermise, or in your terminal
 *  when it needs your password. `all` lists the ones already there too. */
export function SystemCheckRows({
  context,
  checks,
  all = false,
}: {
  context: AppContext;
  checks: SystemCheck[];
  all?: boolean;
}) {
  const [inTerminal, setInTerminal] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const prompt = usePlatform() === "win32" ? "PS>" : "$";
  const fix = (check: SystemCheck) => {
    if (!check.fix) return;
    setError(null);
    if (check.fix.terminal) {
      api
        .systemFixInTerminal(check.id)
        .then(() => setInTerminal(check.id))
        .catch((reason: Error) => setError(reason.message));
      return;
    }
    context.run({
      action: { kind: "systemFix", id: check.id },
      title: check.fix.label.startsWith("Install") ? `Install ${check.title}?` : `${check.fix.label}?`,
      description:
        check.id === "command-line-tools"
          ? "Apple’s installer opens in its own window. When it finishes, click Check Again in Health."
          : check.detail,
      verb: check.fix.label.replace(/…$/, ""),
    });
  };
  return (
    <>
      {checks
        .filter((check) => all || !check.ok)
        .map((check) => (
          <div className="list-row setting-row" key={check.id}>
            {check.ok ? (
              <CheckCircle2 size={16} color="var(--green)" />
            ) : (
              <AlertTriangle size={16} color="var(--orange)" />
            )}
            <div className="row-main">
              <div className="row-title">{check.title}</div>
              <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                {check.detail}
              </div>
              {!check.ok && check.fix && (
                <div style={{ marginTop: 8 }}>
                  <CommandBlock command={check.fix.command} prompt={prompt} />
                </div>
              )}
              {inTerminal === check.id && (
                <p className="field-hint">
                  Finish in the terminal window that opened — it asks for your password — then click Check Again.
                </p>
              )}
              {error && <p className="field-hint error">{error}</p>}
            </div>
            {!check.ok && check.fix && (
              <button className="button" onClick={() => fix(check)}>
                {check.fix.label}
              </button>
            )}
          </div>
        ))}
    </>
  );
}

/** The mise tool a global package becomes: the registry's short name when
 *  mise has one for it, otherwise `<backend>:<name>`. */
export function packageTool(
  context: AppContext,
  pkg: { backend: string; name: string; version: string | null },
): { tool: string; version: string } {
  const plain = pkg.name.replace(/^@[^/]+\//, "");
  const entry = context.mise.registryByName.get(pkg.name) ?? context.mise.registryByName.get(plain);
  if (entry) {
    return { tool: entry.short, version: "latest" };
  }
  return { tool: `${pkg.backend}:${pkg.name}`, version: pkg.version ?? "latest" };
}

/** Homebrew names that differ from mise's (as in scripts/discover-popularity.py). */
export const BREW_RENAMES: Record<string, string> = {
  "git-delta": "delta",
  "kubernetes-cli": "kubectl",
  "go-task": "task",
  awscli: "aws-cli",
  "python-yq": "yq",
  "stripe-cli": "stripe",
};

/** Tools Bring Over already handles, with their versions and defaults. */
export const RUNTIMES = new Set(["node", "python", "ruby", "go"]);

interface ProgramTool {
  tool: string;
  description: string | null;
  foundIn: string;
  commands: string[];
  removeCommand: string | null;
}

/** Programs installed another way that mise's registry has, one row per
 *  tool: Homebrew's matched by formula or cask (`ripgrep`, `git-delta`), the
 *  rest only when the command is the tool's own name (`flyctl`, `stripe`),
 *  so a same-named stranger never matches. */
export function programTools(context: AppContext, programs: InstalledProgram[], skip: Set<string>): ProgramTool[] {
  const registry = context.mise.registry ?? [];
  const byName = new Map<string, RegistryEntry>();
  for (const entry of registry) {
    byName.set(entry.short, entry);
    for (const alias of entry.aliases) if (!byName.has(alias)) byName.set(alias, entry);
  }
  const rows = new Map<string, ProgramTool>();
  for (const program of programs) {
    const formula = program.package?.replace(/@[\d.]+$/, "");
    const entry = formula
      ? byName.get(BREW_RENAMES[formula] ?? formula)
      : (() => {
          const candidate = byName.get(program.name);
          return candidate && (candidate.bins ?? [candidate.short]).includes(program.name) ? candidate : undefined;
        })();
    if (!entry || RUNTIMES.has(entry.short) || skip.has(entry.short) || context.mise.inventory?.[entry.short]) continue;
    const row = rows.get(entry.short);
    if (row) {
      if (!row.commands.includes(program.name)) row.commands.push(program.name);
    } else {
      rows.set(entry.short, {
        tool: entry.short,
        description: entry.description,
        foundIn: program.foundIn,
        commands: [program.name],
        removeCommand: program.removeCommand,
      });
    }
  }
  return [...rows.values()].sort((a, b) => a.tool.localeCompare(b.tool));
}

// ---------------------------------------------------------------------------
// Link an install made outside mise
// ---------------------------------------------------------------------------

/** Registers a folder (a hand-built or other manager's install) as a version
 *  of `tool` with `mise link`, so mise uses it without downloading. */
export function LinkSheet({
  open,
  tool,
  onDismiss,
  context,
}: {
  open: boolean;
  tool: string;
  onDismiss: () => void;
  context: AppContext;
}) {
  const [version, setVersion] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // mise rejects `latest`, aliases, and channels: a version must be exact.
  const validVersion = /^[A-Za-z0-9][A-Za-z0-9._+-]*$/.test(version) && version !== "latest";

  const choose = async () => {
    setError(null);
    try {
      const picked = await api.pickFolder();
      if (picked) setFolder(picked);
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const close = () => {
    setVersion("");
    setFolder(null);
    setError(null);
    onDismiss();
  };

  return (
    <Sheet open={open} onDismiss={close} labelledBy="link-title">
      <div className="sheet-head">
        <h2 id="link-title">Use an existing {tool} install</h2>
        <p>
          Point mise at a folder where {tool} is already installed — built by hand or by another tool — and it uses
          that copy as a version, without downloading anything.
        </p>
      </div>
      <div className="sheet-body">
        <label className="task-field">
          <span className="task-field__label">Version</span>
          <input
            className="text-field mono"
            value={version}
            onChange={(event) => setVersion(event.target.value.trim())}
            placeholder="for example 20.11.1, or a name like system"
            spellCheck={false}
          />
          {version && !validVersion && (
            <span className="field-hint error">Use an exact version or a name, not “latest” or an alias.</span>
          )}
        </label>
        <div className="task-field" style={{ marginTop: 12 }}>
          <span className="task-field__label">Folder</span>
          <div className="setting-edit">
            <span className="mono row-subtitle" style={{ flex: 1 }}>
              {folder ?? "No folder chosen"}
            </span>
            <button className="button" onClick={choose}>
              Choose…
            </button>
          </div>
        </div>
        {validVersion && folder && (
          <div style={{ marginTop: 12 }}>
            <CommandBlock command={`mise link ${tool}@${version} ${folder}`} />
          </div>
        )}
        {error && <div className="callout error selectable">{error}</div>}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={close}>
          Cancel
        </button>
        <button
          className="button primary large"
          disabled={!validVersion || !folder}
          onClick={() => {
            const path = folder!;
            close();
            context.run({
              action: { kind: "link", tool, version, path },
              title: `Linking ${tool} ${version}`,
              description: "",
              verb: "Link",
              autoStart: true,
            });
          }}
        >
          Link
        </button>
      </div>
    </Sheet>
  );
}

const MANAGER_UPDATE: Record<"homebrew" | "winget" | "scoop", { name: string; command: string }> = {
  homebrew: { name: "Homebrew", command: "brew upgrade mise" },
  winget: { name: "winget", command: "winget upgrade --id jdx.mise --exact" },
  scoop: { name: "Scoop", command: "scoop update mise" },
};

/** Updates mise the way this install allows: `mise self-update`, the package
 *  manager that installed it, or — when that needs an administrator — the
 *  steps to run yourself. */
export function UpdateMise({ context, label = "Update mise" }: { context: AppContext; label?: string }) {
  const { mise, run } = context;
  const [showSteps, setShowSteps] = useState(false);
  const info = mise.info;
  const to = info?.latest ? ` to ${info.latest}` : "";
  const updater = info?.updater;
  // Without doctor yet, assume the usual case (self-update works).
  if (mise.doctor?.self_update_available !== false) {
    return (
      <button
        className="button primary"
        onClick={() =>
          run({
            action: { kind: "selfUpdate" },
            title: `Update mise${to}?`,
            description: "mise downloads its newest release and replaces itself. Your tools and settings stay as they are.",
            verb: "Update mise",
          })
        }
      >
        {label}
      </button>
    );
  }
  if (updater && updater.kind in MANAGER_UPDATE) {
    const manager = MANAGER_UPDATE[updater.kind as keyof typeof MANAGER_UPDATE];
    return (
      <button
        className="button primary"
        onClick={() =>
          run({
            action: { kind: "updateMiseWithManager" },
            title: `Update mise${to} with ${manager.name}?`,
            description: `${manager.name} installed mise, so it updates it too. Your tools and settings stay as they are.`,
            verb: "Update mise",
          })
        }
      >
        {label}
      </button>
    );
  }
  const steps =
    updater?.kind === "instructions"
      ? updater.message
      : "This mise can’t update itself. Update it the same way you installed it (for example with your system’s package manager).";
  return (
    <>
      <button className="button" onClick={() => setShowSteps((open) => !open)} aria-expanded={showSteps}>
        How to Update
      </button>
      <Sheet open={showSteps} onDismiss={() => setShowSteps(false)} labelledBy="update-mise-title">
        <div className="sheet-head">
          <h2 id="update-mise-title">Update mise{to}</h2>
        </div>
        <div className="sheet-body">
          <p className="selectable" style={{ whiteSpace: "pre-wrap" }}>
            {steps}
          </p>
          <ExternalLinkButton url="https://mise.jdx.dev/installing-mise.html#updating">
            More about updating mise
          </ExternalLinkButton>
        </div>
        <div className="sheet-foot">
          <button className="button large" onClick={() => setShowSteps(false)}>
            Done
          </button>
        </div>
      </Sheet>
    </>
  );
}

/** mise's setup command for the current shell, plus anything else that shell
 *  needs. powermise doesn't edit shell files itself. */
export function ActivationSteps({ activation }: { activation: Activation }) {
  if (activation.unsupported) {
    return (
      <div className="activation-steps">
        <p className="section-note">
          powermise doesn’t have steps for {activation.shell}.{" "}
          <ExternalLinkButton url={SHELL_SETUP_URL}>mise’s shell setup guide</ExternalLinkButton> covers bash, zsh,
          fish, PowerShell, Nushell, and more.
        </p>
      </div>
    );
  }
  return (
    <div className="activation-steps">
      <p className="section-note">
        Run this once in {activation.shell}, then open a new terminal window. It adds mise to {activation.file} if
        it isn’t there already.
      </p>
      <CommandBlock command={activation.command} prompt={activation.prompt} />
      {activation.extra && (
        <>
          <p className="section-note">{activation.extra.text}</p>
          <CommandBlock command={activation.extra.command} prompt={activation.prompt} />
        </>
      )}
      {activation.note && <p className="field-hint">{activation.note}</p>}
    </div>
  );
}
