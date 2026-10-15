import { useState } from "react";
import { AlertTriangle, CheckCircle2, Download, ExternalLink, Github, Info, Loader2, Trash2 } from "lucide-react";
import { api, type DetectedImport, type GlobalPackage, type ImportSource, type ShellConflict } from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import type { Activation } from "../health";
import { isGlobalPath, prettyPath } from "../mise";
import { Checkbox, CommandBlock } from "./ui";
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

const SOURCE_LABEL: Record<ImportSource, string> = {
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
const toolLabel = (tool: string) => TOOL_LABEL[tool] ?? tool;

/** Sources mise imports itself with `mise sync`; the rest are linked. */
const SYNCED: Partial<Record<ImportSource, string>> = {
  nvm: "node --nvm",
  nodenv: "node --nodenv",
  "node-brew": "node --brew",
  pyenv: "python --pyenv",
  uv: "python --uv",
  "ruby-brew": "ruby --brew",
};

/** The version of `tool` your global mise config uses, if any. */
function globalVersion(context: AppContext, tool: string): string | undefined {
  return context.mise.inventory?.[tool]?.find(
    (version) => version.source && isGlobalPath(version.source.path, context.env.miseConfigDir),
  )?.version;
}

/** What's left to do for an import: versions mise doesn't have, and whether
 *  the manager's default isn't your mise default yet. */
function importTodo(context: AppContext, entry: DetectedImport) {
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

/** Versions other version managers installed, each with a button to let
 *  mise use them (`mise sync`, or `mise link` per version) and, when that
 *  manager had a default, to make it your mise default too. Returns null
 *  when there's nothing left to bring over. */
export function ImportList({ context, imports }: { context: AppContext; imports: DetectedImport[] }) {
  const pending = pendingImports(context, imports);
  if (!pending.length) return null;
  return (
    <div className="list">
      {pending.map((entry) => {
        const { missing, current, offerDefault } = importTodo(context, entry);
        const from = SOURCE_LABEL[entry.source];
        const how = SYNCED[entry.source] ? `mise sync ${SYNCED[entry.source]}` : `mise link (${missing.length})`;
        const bring = (makeDefault: boolean) =>
          context.run({
            action: { kind: "importVersions", source: entry.source, tool: entry.tool, makeDefault },
            title: makeDefault
              ? missing.length
                ? `Bring over ${from}’s ${toolLabel(entry.tool)} and use ${entry.defaultVersion}?`
                : `Use ${toolLabel(entry.tool)} ${entry.defaultVersion} by default?`
              : `Use your ${from} ${toolLabel(entry.tool)} versions in mise?`,
            description: [
              missing.length
                ? `mise links ${new Intl.ListFormat("en", { type: "conjunction" }).format(missing.map((version) => version.version))} where ${from} installed ${missing.length === 1 ? "it" : "them"}, so it can use ${missing.length === 1 ? "it" : "them"} without downloading again. Nothing is moved or removed.`
                : "",
              makeDefault
                ? `Then ${entry.defaultVersion} becomes your default everywhere${current ? `, replacing ${current}` : ""} — the version ${from} had as your default.`
                : "",
            ]
              .filter(Boolean)
              .join(" "),
            verb: missing.length ? "Bring Over" : "Use It",
          });
        return (
          <div className="list-row" key={`${entry.source}-${entry.tool}`}>
            <Download size={16} color="var(--text-tertiary)" />
            <div className="row-main">
              <div className="row-title">
                {toolLabel(entry.tool)} from {from}
              </div>
              <div className="row-subtitle mono">
                {entry.versions
                  .map((version) => `${version.version}${version.version === entry.defaultVersion ? " (default)" : ""}${version.inMise ? " ✓" : ""}`)
                  .join(", ")}
              </div>
            </div>
            <div className="row-trailing">
              {offerDefault && (
                <button
                  className="button"
                  onClick={() => bring(true)}
                  title={`Also make ${entry.defaultVersion} your mise default (mise use -g ${entry.tool}@${entry.defaultVersion})`}
                >
                  {missing.length ? `Bring Over + Use ${entry.defaultVersion}` : `Use ${entry.defaultVersion}`}
                </button>
              )}
              {missing.length > 0 && (
                <button className="button" onClick={() => bring(false)} title={how}>
                  Bring Over
                </button>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
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

/** The mise tool a global package becomes: the registry's short name when
 *  mise has one for it, otherwise `<backend>:<name>`. */
function packageTool(context: AppContext, pkg: GlobalPackage): { tool: string; version: string } {
  const plain = pkg.name.replace(/^@[^/]+\//, "");
  const entry = context.mise.registryByName.get(pkg.name) ?? context.mise.registryByName.get(plain);
  if (entry) {
    return { tool: entry.short, version: "latest" };
  }
  return { tool: `${pkg.backend}:${pkg.name}`, version: pkg.version ?? "latest" };
}

/** Command-line packages installed with npm -g, pipx, uv, or cargo install,
 *  which mise can keep instead — so they survive a Node or Python switch.
 *  Returns null when there are none mise doesn't already have. */
export function GlobalPackageList({ context, packages }: { context: AppContext; packages: GlobalPackage[] }) {
  const inMise = (tool: string) => !!context.mise.inventory?.[tool];
  const rows = packages
    .map((pkg) => ({ pkg, ...packageTool(context, pkg) }))
    .filter((row) => !inMise(row.tool));
  const [picked, setPicked] = useState<Set<string> | null>(null);
  if (!rows.length) return null;
  const selected = picked ?? new Set(rows.map((row) => row.tool));
  const toggle = (tool: string) => {
    const next = new Set(selected);
    if (next.has(tool)) next.delete(tool);
    else next.add(tool);
    setPicked(next);
  };
  const chosen = rows.filter((row) => selected.has(row.tool));
  return (
    <>
      <div className="list">
        {rows.map((row) => (
          <button
            key={row.tool}
            className="list-row interactive"
            onClick={() => toggle(row.tool)}
            role="checkbox"
            aria-checked={selected.has(row.tool)}
          >
            <Checkbox checked={selected.has(row.tool)} />
            <div className="row-main">
              <div className="row-title">
                {row.pkg.name}
                {row.pkg.version && <span className="row-subtitle"> {row.pkg.version}</span>}
              </div>
              <div className="row-subtitle">From {row.pkg.foundIn}</div>
            </div>
            <span className="badge mono">{row.tool}</span>
          </button>
        ))}
      </div>
      <div style={{ marginTop: 10 }}>
        <button
          className="button"
          disabled={!chosen.length}
          onClick={() =>
            context.run({
              action: { kind: "useGlobalMany", tools: chosen.map((row) => [row.tool, row.version]) },
              title: `Let mise manage ${chosen.length === 1 ? chosen[0].pkg.name : `${chosen.length} packages`}?`,
              description:
                "mise installs them as tools in your global config, so they stay put when you switch Node or Python versions and show up in My Tools. The old copies stay where they are until you remove them.",
              verb: "Add to mise",
            })
          }
        >
          Add {chosen.length || ""} to mise
        </button>
      </div>
    </>
  );
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
