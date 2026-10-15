import { useState } from "react";
import { AlertTriangle, Blocks, CheckCircle2, FolderOpen, Info, Loader2, Pencil, Trash2 } from "lucide-react";
import { api, type Preview, type RemovedSetup } from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import { PROVIDER_HINT, PROVIDER_LABEL, titleFor } from "../catalog";
import { ApplySheet } from "../components/ApplySheet";
import { CommandBlock, Monogram, Section } from "../components/ui";
import { Sheet } from "../components/Sheet";
import { prettyPath } from "../mise";

export function SetupView({ context, name }: { context: AppContext; name: string }) {
  const { catalog, env, selections, navigate, refreshSelections, run, mise } = context;
  const selection = selections.find((entry) => entry.name === name);
  const target = env.hostTarget;
  const status = useAsync(() => api.setupStatus(name, target), [name, target, mise.revision]);

  const [preview, setPreview] = useState<Preview | null>(null);
  const [applyOpen, setApplyOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [removed, setRemoved] = useState<RemovedSetup | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pretty = (path: string) => prettyPath(path);

  if (!selection && !removed) {
    return (
      <div className="empty">
        <h3>This setup no longer exists</h3>
      </div>
    );
  }

  const openApply = async () => {
    if (!selection) return;
    try {
      setPreview(
        await api.preview(
          { name, harnesses: selection.harnesses, includeDesktopApps: selection.includeDesktopApps },
          target,
        ),
      );
      setApplyOpen(true);
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const remove = async () => {
    setError(null);
    try {
      setRemoved(await api.removeSetup(name));
      await refreshSelections();
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  // Setups rely on conf.d and `mise bootstrap --only`, both new in 2026.10.
  const miseTooOld = !!mise.info && !mise.info.recommendedMet;
  const items = status.data?.items ?? [];
  const missing = items.filter((item) => !item.installed);
  const outOfDate = selection?.applied && status.data && status.data.change.kind !== "unchanged";

  return (
    <div className="content">
      <div className="page">
        <div className="detail-hero">
          <div
            className="monogram"
            style={{ width: 64, height: 64, background: "linear-gradient(180deg, #3a3d46, #17181d)" }}
            aria-hidden
          >
            <Blocks size={28} />
          </div>
          <div>
            <h1 className="page-title">{name}</h1>
            <p className="page-subtitle">
              {!selection?.applied ? (
                <span className="badge">Not applied yet</span>
              ) : missing.length ? (
                <span className="badge orange">{missing.length} not installed</span>
              ) : (
                <span className="badge green">Applied</span>
              )}
            </p>
          </div>
          {selection && (
            <div className="detail-actions">
              <button className="button large" onClick={() => navigate({ kind: "editor", name })}>
                <Pencil size={14} /> Edit
              </button>
              <button
                className="button primary large"
                onClick={openApply}
                disabled={!env.misePath || miseTooOld}
                title={miseTooOld ? `AI Setups need mise ${mise.info?.recommended} or newer` : undefined}
              >
                {selection.applied ? "Apply Again…" : "Apply…"}
              </button>
            </div>
          )}
        </div>

        {selection && miseTooOld && (
          <div className="callout warn" style={{ marginTop: 16 }}>
            <AlertTriangle size={14} />
            <span>
              AI Setups need mise {mise.info?.recommended} or newer — this computer has {mise.info?.version?.split(" ")[0]}.{" "}
              <button className="link-button" onClick={() => navigate({ kind: "health" })}>
                Update mise in Health
              </button>
            </span>
          </div>
        )}

        {error && (
          <div className="callout error selectable" style={{ marginTop: 16 }}>
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}

        {selection && (
          <Section title="Harnesses">
            <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))" }}>
              {selection.harnesses.map((id) => {
                const harness = catalog.harnessById.get(id);
                return (
                  <button key={id} className="card" style={{ padding: 10 }} onClick={() => navigate({ kind: "harness", id })}>
                    <div className="card-head">
                      <Monogram id={id} title={harness?.title ?? id} size={28} />
                      <div className="card-title">{harness?.title ?? id}</div>
                    </div>
                  </button>
                );
              })}
            </div>
            {selection.includeDesktopApps && (
              <p className="section-note" style={{ marginTop: 10 }}>
                Includes desktop apps where available.
              </p>
            )}
          </Section>
        )}

        {selection && (
          <Section
            title="On this computer"
            note={selection.applied ? "What mise reports for everything this setup installs." : "Apply the setup to install these."}
          >
            {outOfDate && (
              <div className="callout warn" style={{ marginBottom: 10 }}>
                <AlertTriangle size={14} />
                <span>The setup changed since it was applied. Apply again to update mise’s config.</span>
              </div>
            )}
            {status.loading && !status.data && (
              <p className="field-hint">
                <Loader2 size={12} className="spin" /> Asking mise…
              </p>
            )}
            {status.error && (
              <div className="callout error selectable">
                <AlertTriangle size={14} />
                <span>{status.error}</span>
              </div>
            )}
            {items.length > 0 && (
              <div className="list">
                {items.map((item) => (
                  <div className="list-row" key={item.key}>
                    {item.installed ? (
                      <CheckCircle2 size={16} className="outcome-icon succeeded" />
                    ) : (
                      <AlertTriangle size={16} color="var(--orange)" />
                    )}
                    <div className="row-main">
                      <div className="row-title">{titleFor(catalog, item.toolId)}</div>
                      <div className="row-subtitle">
                        {item.overriddenBy
                          ? `Using the version from ${pretty(item.overriddenBy)} — your config wins`
                          : item.state}
                      </div>
                    </div>
                    <div className="row-trailing">
                      <span className="mono">{item.key}</span>
                      <span className="badge" title={PROVIDER_HINT[item.kind]}>
                        {PROVIDER_LABEL[item.kind]}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </Section>
        )}

        {selection && (
          <Section title="In the terminal" note="A setup is one mise config file. These do what powermise does.">
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <CommandBlock command={status.data?.applyCommand ?? "mise bootstrap --yes --only packages,tools"} />
              <div className="command">
                <code>{pretty(selection.fragmentPath)}</code>
                <button className="icon-button" onClick={() => api.reveal(selection.fragmentPath)} aria-label="Show in folder" title="Show in folder">
                  <FolderOpen size={14} />
                </button>
              </div>
            </div>
          </Section>
        )}

        {selection && (
          <Section title="Remove">
            <button className="button destructive" onClick={() => setRemoveOpen(true)}>
              <Trash2 size={14} /> Remove Setup…
            </button>
          </Section>
        )}
      </div>

      <ApplySheet
        hostTarget={env.hostTarget}
        open={applyOpen}
        name={name}
        target={target}
        preview={preview}
        catalog={catalog}
        onClose={() => setApplyOpen(false)}
        onApply={() => {
          void refreshSelections();
          run({
            action: { kind: "bootstrapPackagesAndTools" },
            title: `Applying ${name}`,
            description: "",
            verb: "Apply",
            autoStart: true,
          });
        }}
      />

      <Sheet
        open={removeOpen}
        onDismiss={() => {
          setRemoveOpen(false);
          if (removed) navigate({ kind: "harnesses" });
        }}
        labelledBy="remove-title"
      >
        <div className="sheet-head">
          <h2 id="remove-title">{removed ? "Setup removed" : `Remove “${name}”?`}</h2>
          <p>
            {removed
              ? "mise no longer includes it. Tools and packages it installed are still on this computer."
              : "powermise deletes this setup and its mise config file. Tools and packages it installed stay installed."}
          </p>
        </div>
        <div className="sheet-body">
          {selection && !removed && (
            <div className="command">
              <code>{pretty(selection.fragmentPath)}</code>
            </div>
          )}
          {removed && (
            <div className="callout info">
              <Info size={14} />
              <div>
                <div>To free up space, delete tool versions nothing uses anymore:</div>
                <div style={{ marginTop: 6 }}>
                  <CommandBlock command={removed.cleanupCommand} />
                </div>
              </div>
            </div>
          )}
        </div>
        <div className="sheet-foot">
          {!removed ? (
            <>
              <button className="button large" onClick={() => setRemoveOpen(false)}>
                Cancel
              </button>
              <button className="button large danger" onClick={remove}>
                Remove
              </button>
            </>
          ) : (
            <>
              <button
                className="button large"
                onClick={() => {
                  setRemoveOpen(false);
                  navigate({ kind: "harnesses" });
                }}
              >
                Done
              </button>
              <button
                className="button primary large"
                onClick={() => {
                  setRemoveOpen(false);
                  navigate({ kind: "harnesses" });
                  run({
                    action: { kind: "pruneTools" },
                    title: "Delete unused tool versions?",
                    description: "mise deletes installed tool versions that no config asks for anymore.",
                    verb: "Clean Up",
                  });
                }}
              >
                Clean Up Unused Tools…
              </button>
            </>
          )}
        </div>
      </Sheet>
    </div>
  );
}
