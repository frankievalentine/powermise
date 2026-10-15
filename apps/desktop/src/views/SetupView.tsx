import { useEffect, useState } from "react";
import { AlertTriangle, Blocks, CheckCircle2, FolderOpen, Loader2, Pencil, RefreshCw, Trash2 } from "lucide-react";
import { api, type Preview, type StatusResult } from "../api";
import type { AppContext } from "../App";
import { ApplySheet, ResultList } from "../components/ApplySheet";
import { CommandBlock, Monogram, Section } from "../components/ui";
import { Sheet } from "../components/Sheet";

export function SetupView({ context, name }: { context: AppContext; name: string }) {
  const { catalog, env, selections, navigate, refreshSelections } = context;
  const selection = selections.find((entry) => entry.name === name);
  const target = env.hostTarget;

  const [status, setStatus] = useState<StatusResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [applyOpen, setApplyOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [removeError, setRemoveError] = useState<string | null>(null);

  useEffect(() => {
    setStatus(null);
    setStatusError(null);
  }, [name]);

  if (!selection) {
    return (
      <div className="empty">
        <h3>This setup no longer exists</h3>
      </div>
    );
  }

  const failed = selection.lastRun?.operations.some((op) => op.outcome === "failed");

  const checkStatus = async () => {
    setChecking(true);
    setStatusError(null);
    try {
      setStatus(await api.status(name, target));
    } catch (error) {
      setStatusError((error as Error).message);
    } finally {
      setChecking(false);
    }
  };

  const openApply = async () => {
    try {
      setPreview(
        await api.preview(
          { name, harnesses: selection.harnesses, includeDesktopApps: selection.includeDesktopApps },
          target,
        ),
      );
      setApplyOpen(true);
    } catch (error) {
      setStatusError((error as Error).message);
    }
  };

  const remove = async () => {
    setRemoveError(null);
    try {
      await api.remove(name);
      await refreshSelections();
      setRemoveOpen(false);
      navigate({ kind: "harnesses" });
    } catch (error) {
      setRemoveError((error as Error).message);
    }
  };

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
            <h1 className="page-title">{selection.name}</h1>
            <p className="page-subtitle">
              {failed ? (
                <span className="badge orange">Last apply had problems</span>
              ) : selection.applied ? (
                <span className="badge green">Applied</span>
              ) : (
                <span className="badge">Not applied yet</span>
              )}
            </p>
          </div>
          <div className="detail-actions">
            <button className="button large" onClick={() => navigate({ kind: "editor", name })}>
              <Pencil size={14} /> Edit
            </button>
            <button className="button large" onClick={checkStatus} disabled={checking || !env.misePath}>
              {checking ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} Check Status
            </button>
            <button className="button primary large" onClick={openApply} disabled={!env.misePath}>
              Apply…
            </button>
          </div>
        </div>

        <Section title="Harnesses">
          <div className="grid" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))" }}>
            {selection.harnesses.map((id) => {
              const harness = catalog.harnessById.get(id);
              return (
                <button
                  key={id}
                  className="card"
                  style={{ padding: 10 }}
                  onClick={() => navigate({ kind: "harness", id })}
                >
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

        {(status || statusError) && (
          <Section title="Status">
            {statusError && (
              <div className="callout error selectable">
                <AlertTriangle size={14} />
                <span style={{ whiteSpace: "pre-wrap" }}>{statusError}</span>
              </div>
            )}
            {status && (
              <>
                {status.satisfied ? (
                  <div className="callout info" style={{ color: "var(--green)" }}>
                    <CheckCircle2 size={14} />
                    <span>Everything in this setup is installed and up to date.</span>
                  </div>
                ) : (
                  <div className="callout warn">
                    <AlertTriangle size={14} />
                    <div>
                      <strong>This computer doesn’t match the setup yet.</strong>
                      {status.change.kind !== "unchanged" && (
                        <div>The configuration file needs to be {status.change.kind === "create" ? "written" : "updated"} — apply to fix.</div>
                      )}
                      {status.drift && <div>{status.drift}</div>}
                      {status.followUps.map((followUp) => (
                        <div key={followUp}>{followUp}</div>
                      ))}
                      {status.native
                        .filter((entry) => !entry.present)
                        .map((entry) => (
                          <div key={entry.operationId}>{entry.package} is not installed</div>
                        ))}
                    </div>
                  </div>
                )}
                <div style={{ marginTop: 10 }}>
                  <CommandBlock command={status.command} />
                </div>
              </>
            )}
          </Section>
        )}

        {selection.lastRun && (
          <Section title="Last apply">
            <ResultList results={selection.lastRun.operations} catalog={catalog} />
          </Section>
        )}

        <Section title="In the terminal" note="This setup is a plain file the powermise CLI uses too.">
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <CommandBlock command={`powermise apply --name ${name} --target ${target} --yes`} />
            <CommandBlock command={`powermise status --name ${name} --target ${target}`} />
            <div className="command">
              <code>{selection.path}</code>
              <button
                className="icon-button"
                onClick={() => api.reveal(selection.path)}
                aria-label="Show in folder"
                title="Show in folder"
              >
                <FolderOpen size={14} />
              </button>
            </div>
          </div>
        </Section>

        <Section title="Remove">
          <button className="button destructive" onClick={() => setRemoveOpen(true)}>
            <Trash2 size={14} /> Remove Setup…
          </button>
        </Section>
      </div>

      <ApplySheet
        open={applyOpen}
        name={name}
        target={target}
        preview={preview}
        catalog={catalog}
        onClose={() => setApplyOpen(false)}
        onApplied={refreshSelections}
      />

      <Sheet open={removeOpen} onDismiss={() => setRemoveOpen(false)} labelledBy="remove-title">
        <div className="sheet-head">
          <h2 id="remove-title">Remove “{name}”?</h2>
          <p>
            powermise deletes this setup and the configuration files it wrote for it. Tools that
            were already installed stay installed in mise.
          </p>
        </div>
        <div className="sheet-body">
          <CommandBlock command={`powermise uninstall --name ${name} --yes`} />
          {removeError && (
            <div className="callout error selectable" style={{ marginTop: 10 }}>
              <AlertTriangle size={14} />
              <span>{removeError}</span>
            </div>
          )}
        </div>
        <div className="sheet-foot">
          <button className="button large" onClick={() => setRemoveOpen(false)}>
            Cancel
          </button>
          <button className="button large" style={{ background: "var(--red)", color: "#fff", boxShadow: "none" }} onClick={remove}>
            Remove
          </button>
        </div>
      </Sheet>
    </div>
  );
}
