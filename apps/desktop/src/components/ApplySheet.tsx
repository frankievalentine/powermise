import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Info, Loader2 } from "lucide-react";
import {
  api,
  PLATFORM_LABEL,
  type BootstrapPlan,
  type Install,
  type Platform,
  type Preview,
  type ToolProviderKind,
} from "../api";
import { PROVIDER_LABEL, titleFor, type CatalogIndex } from "../catalog";
import { CommandBlock, DiffView, Segmented } from "./ui";
import { Sheet } from "./Sheet";

type Tab = "changes" | "file" | "commands";

const GROUPS: { label: string; kinds: ToolProviderKind[] }[] = [
  { label: "Tools mise installs", kinds: ["mise-tool"] },
  { label: "Packages mise installs", kinds: ["bootstrap-brew", "bootstrap-apt"] },
  { label: "Mac apps", kinds: ["bootstrap-brew-cask"] },
  { label: "Windows packages", kinds: ["winget", "scoop"] },
  { label: "Already on this computer", kinds: ["system"] },
];

/** Reviews a setup, then writes its mise fragment and hands off to a streamed
 *  `mise bootstrap` run (via `onApply`). */
export function ApplySheet({
  open,
  name,
  target,
  hostTarget,
  preview,
  catalog,
  onClose,
  onApply,
}: {
  open: boolean;
  name: string;
  target: Platform;
  /** The platform powermise is running on. */
  hostTarget: Platform;
  preview: Preview | null;
  catalog: CatalogIndex;
  onClose: () => void;
  /** Called after the fragment is written: start the `mise bootstrap` run. */
  onApply: () => void;
}) {
  const [tab, setTab] = useState<Tab>("changes");
  const [plan, setPlan] = useState<BootstrapPlan | null>(null);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setTab("changes");
    setError(null);
    setWriting(false);
    setPlan(null);
    api.bootstrapPlan().then(setPlan).catch(() => setPlan(null));
  }, [open]);

  // Anything `mise bootstrap` would also install that isn't from this setup.
  const others = useMemo(() => {
    if (!plan || !preview) return [];
    const ours = new Set(preview.installs.map((install) => install.key));
    return plan.resources.filter(
      (resource) =>
        ["create", "update"].includes(resource.action) &&
        ["package", "tool"].includes(resource.id.kind) &&
        !ours.has(resource.id.name),
    );
  }, [plan, preview]);

  const manual = preview ? [...preview.resolved.mcpServerIds, ...preview.resolved.integrationIds] : [];
  const pretty = (path: string) => path.replace(/^\/(Users|home)\/[^/]+/, "~");

  // On Linux/WSL, the first Homebrew install needs `sudo` to create its
  // folder; the app has no terminal to ask for a password, so that first
  // run happens in the terminal app.
  const inTerminal = !!preview?.homebrewSetup;
  // apt (and the first Homebrew install) may need a password on Linux, which
  // only a terminal can ask for — so it's always an option there.
  const linuxHost = (target === "linux" || target === "wsl") && hostTarget === target;

  const apply = async (terminal = inTerminal) => {
    setWriting(true);
    setError(null);
    try {
      await api.writeSetup(name, target);
      if (terminal) {
        await api.bootstrapInTerminal();
        onClose();
        return;
      }
      onClose();
      onApply();
    } catch (reason) {
      setError((reason as Error).message);
      setWriting(false);
    }
  };

  return (
    <Sheet open={open} onDismiss={onClose} dismissible={!writing} labelledBy="apply-title">
      <div className="sheet-head">
        <h2 id="apply-title">Apply “{name}” to this computer?</h2>
        <p>
          powermise adds this setup to your mise config as one file, then mise installs it on{" "}
          {PLATFORM_LABEL[target]}. Removing the setup later deletes that file.
        </p>
      </div>
      <div className="sheet-body">
        {preview && (
          <>
            <div style={{ marginBottom: 12 }}>
              <Segmented
                label="Review"
                value={tab}
                onChange={setTab}
                options={[
                  { value: "changes", label: "What gets installed" },
                  { value: "file", label: "Config file" },
                  { value: "commands", label: "Commands" },
                ]}
              />
            </div>

            {tab === "changes" && (
              <>
                {GROUPS.map((group) => {
                  const members = preview.installs.filter((install) => group.kinds.includes(install.kind));
                  if (!members.length) return null;
                  return (
                    <div key={group.label} style={{ marginBottom: 14 }}>
                      <div className="side-heading" style={{ marginTop: 0 }}>
                        {group.label}
                      </div>
                      <div className="list">
                        {members.map((install) => (
                          <InstallRow key={install.key} install={install} catalog={catalog} />
                        ))}
                      </div>
                    </div>
                  );
                })}
                {manual.length > 0 && (
                  <div style={{ marginBottom: 14 }}>
                    <div className="side-heading" style={{ marginTop: 0 }}>
                      Set up yourself
                    </div>
                    <div className="list">
                      {manual.map((id) => (
                        <div className="list-row" key={id} style={{ minHeight: 36 }}>
                          <div className="row-main">
                            <div className="row-title">{titleFor(catalog, id)}</div>
                          </div>
                          <span className="badge orange">Manual setup</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                {preview.skipped.length > 0 && (
                  <div className="callout warn" style={{ marginBottom: 14 }}>
                    <AlertTriangle size={14} />
                    <div>
                      <strong>Not installed on this Mac:</strong>{" "}
                      {preview.skipped.map((skip) => titleFor(catalog, skip.id)).join(", ")}.{" "}
                      {preview.skipped[0].reason}; this Mac has an Intel processor.
                    </div>
                  </div>
                )}
                {preview.homebrewSetup && (
                  <div className="callout info" style={{ marginBottom: 14 }}>
                    <Info size={14} />
                    <div>
                      <strong>Homebrew needs a one-time setup.</strong> mise installs Homebrew packages into{" "}
                      <span className="mono">{preview.homebrewSetup}</span>, and creating that folder needs your
                      password once. powermise can’t type it for you, so this first install opens in your terminal
                      app. After that, installs run here as usual. To set it up yourself instead:
                      <div style={{ marginTop: 6 }}>
                        <CommandBlock
                          command={`sudo mkdir -p ${preview.homebrewSetup} && sudo chown -R "$USER" ${preview.homebrewSetup.replace(/\/\.linuxbrew$/, "")}`}
                        />
                      </div>
                    </div>
                  </div>
                )}
                {plan === null && (
                  <p className="field-hint">
                    <Loader2 size={12} className="spin" /> Checking the rest of your mise config…
                  </p>
                )}
                {others.length > 0 && (
                  <div className="callout info">
                    <Info size={14} />
                    <div>
                      <strong>Your mise config also asks for these, so mise installs them too:</strong>
                      <div className="mono" style={{ marginTop: 4 }}>
                        {others.map((resource) => resource.id.name).join(", ")}
                      </div>
                    </div>
                  </div>
                )}
              </>
            )}

            {tab === "file" && (
              <>
                <p className="section-note" style={{ margin: "0 0 8px" }}>
                  <span className="mono selectable">{pretty(preview.fragmentPath)}</span> ·{" "}
                  {preview.change.kind === "create"
                    ? "new file"
                    : preview.change.kind === "update"
                      ? "updated"
                      : "no changes"}
                </p>
                <DiffView diff={preview.change.kind === "unchanged" ? preview.fragmentContents : preview.change.diff} />
              </>
            )}

            {tab === "commands" && (
              <>
                <p className="section-note" style={{ margin: "0 0 8px" }}>
                  1. powermise writes the setup to <span className="mono">{pretty(preview.fragmentPath)}</span>.
                  mise always reads files in <span className="mono">conf.d</span>, so its tools work in every
                  terminal.
                </p>
                <p className="section-note" style={{ margin: "0 0 8px" }}>2. mise installs what it declares:</p>
                <CommandBlock command={preview.applyCommand} />
                <p className="field-hint" style={{ marginTop: 8 }}>
                  <span className="mono">--only packages,tools</span> keeps the run to packages and tools, so your
                  dotfiles, services, and other bootstrap settings aren’t touched.
                </p>
              </>
            )}
          </>
        )}
        {error && (
          <div className="callout error selectable" style={{ marginTop: 10 }}>
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="sheet-foot">
        <button className="button large" onClick={onClose} disabled={writing}>
          Cancel
        </button>
        {linuxHost && !inTerminal && (
          <button
            className="button large"
            onClick={() => void apply(true)}
            disabled={!preview || writing}
            title="Runs mise bootstrap in your terminal app, where sudo can ask for your password"
          >
            Run in Terminal
          </button>
        )}
        <button className="button primary large" onClick={() => void apply()} disabled={!preview || writing}>
          {writing && <Loader2 size={14} className="spin" />} {inTerminal ? "Apply in Terminal" : "Apply"}
        </button>
      </div>
    </Sheet>
  );
}

function InstallRow({ install, catalog }: { install: Install; catalog: CatalogIndex }) {
  return (
    <div className="list-row" style={{ minHeight: 36 }}>
      <div className="row-main">
        <div className="row-title">{titleFor(catalog, install.toolId)}</div>
      </div>
      <div className="row-trailing">
        <span className="mono">{install.key}</span>
        <span className="badge">{PROVIDER_LABEL[install.kind]}</span>
        <span className="mono version-pill">{install.version}</span>
      </div>
    </div>
  );
}
