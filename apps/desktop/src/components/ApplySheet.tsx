import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  Loader2,
  MinusCircle,
  XCircle,
} from "lucide-react";
import {
  api,
  inTauri,
  onApplyProgress,
  PLATFORM_LABEL,
  type ApplyResult,
  type Operation,
  type OperationOutcome,
  type Platform,
  type Preview,
} from "../api";
import { operationTitle, type CatalogIndex } from "../catalog";
import { CommandBlock, DiffView, Segmented } from "./ui";
import { Sheet } from "./Sheet";

type Phase =
  | { kind: "review" }
  | { kind: "running"; steps: string[] }
  | { kind: "done"; result: ApplyResult }
  | { kind: "error"; message: string };

type Tab = "changes" | "file" | "command";

const GROUPS: { label: string; providers: Operation["provider"][] }[] = [
  { label: "Install with mise", providers: ["mise-tools"] },
  { label: "Install system packages", providers: ["mise-bootstrap"] },
  { label: "Install with Windows package managers", providers: ["winget", "scoop", "npm", "verified-script"] },
  { label: "Needs manual setup", providers: ["config-adapter"] },
];

export function ApplySheet({
  open,
  name,
  target,
  preview,
  catalog,
  onClose,
  onApplied,
}: {
  open: boolean;
  name: string;
  target: Platform;
  preview: Preview | null;
  catalog: CatalogIndex;
  onClose: () => void;
  onApplied: () => void;
}) {
  const [phase, setPhase] = useState<Phase>({ kind: "review" });
  const [tab, setTab] = useState<Tab>("changes");

  useEffect(() => {
    if (open) {
      setPhase({ kind: "review" });
      setTab("changes");
    }
  }, [open]);

  const installCount = useMemo(
    () => preview?.operations.filter((operation) => operation.provider !== "config-adapter").length ?? 0,
    [preview],
  );

  const run = async () => {
    setPhase({ kind: "running", steps: [inTauri ? "Starting" : "Installing tools (browser preview)"] });
    const unlisten = await onApplyProgress((step) =>
      setPhase((current) =>
        current.kind === "running" ? { kind: "running", steps: [...current.steps, step] } : current,
      ),
    );
    try {
      const result = await api.apply(name, target);
      setPhase({ kind: "done", result });
      onApplied();
    } catch (error) {
      setPhase({ kind: "error", message: (error as Error).message });
    } finally {
      unlisten();
    }
  };

  const running = phase.kind === "running";

  return (
    <Sheet open={open} onDismiss={onClose} dismissible={!running} labelledBy="apply-title">
      <div className="sheet-head">
        <h2 id="apply-title">
          {phase.kind === "done"
            ? doneTitle(phase.result)
            : phase.kind === "error"
              ? "Couldn’t apply this setup"
              : running
                ? `Applying “${name}”…`
                : `Apply “${name}” to this computer?`}
        </h2>
        {phase.kind === "review" && (
          <p>
            powermise will install {installCount} {installCount === 1 ? "item" : "items"} on{" "}
            {PLATFORM_LABEL[target]} using mise, and write one configuration file it owns. Your own
            mise settings aren’t changed.
          </p>
        )}
        {running && <p>This can take a few minutes the first time. You can keep using your computer.</p>}
      </div>

      <div className="sheet-body">
        {phase.kind === "review" && preview && (
          <>
            <div style={{ marginBottom: 12 }}>
              <Segmented
                label="Review"
                value={tab}
                onChange={setTab}
                options={[
                  { value: "changes", label: "Changes" },
                  { value: "file", label: "Config File" },
                  { value: "command", label: "Command" },
                ]}
              />
            </div>
            {tab === "changes" && <ChangeGroups operations={preview.operations} catalog={catalog} />}
            {tab === "file" && (
              <>
                <p className="section-note" style={{ margin: "0 0 8px" }}>
                  <span className="mono selectable">{preview.change.path}</span> ·{" "}
                  {preview.change.kind === "create"
                    ? "new file"
                    : preview.change.kind === "update"
                      ? "updated"
                      : "no changes"}
                </p>
                <DiffView
                  diff={preview.change.kind === "unchanged" ? preview.configContents : preview.change.diff}
                />
              </>
            )}
            {tab === "command" && (
              <>
                <p className="section-note" style={{ margin: "0 0 8px" }}>
                  Applying here does exactly what this command does in a terminal:
                </p>
                <CommandBlock command={preview.applyCommand} />
              </>
            )}
          </>
        )}

        {running && (
          <div className="progress-list" aria-live="polite">
            {phase.steps.map((step, index) => {
              const current = index === phase.steps.length - 1;
              return (
                <div key={index} className={`progress-step${current ? " current" : ""}`}>
                  {current ? (
                    <Loader2 size={14} className="spin" />
                  ) : (
                    <CheckCircle2 size={14} className="outcome-icon succeeded" />
                  )}
                  {humanizeStep(step, catalog)}
                </div>
              );
            })}
          </div>
        )}

        {phase.kind === "done" && <ApplySummary result={phase.result} catalog={catalog} />}

        {phase.kind === "error" && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span style={{ whiteSpace: "pre-wrap" }}>{phase.message}</span>
          </div>
        )}
      </div>

      <div className="sheet-foot">
        {phase.kind === "review" && (
          <>
            <button className="button large" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary large" onClick={run} disabled={!preview}>
              Apply
            </button>
          </>
        )}
        {running && (
          <button className="button large" disabled>
            Working…
          </button>
        )}
        {(phase.kind === "done" || phase.kind === "error") && (
          <button className="button primary large" onClick={onClose}>
            Done
          </button>
        )}
      </div>
    </Sheet>
  );
}

function doneTitle(result: ApplyResult): string {
  const failed = result.results.some((entry) => entry.outcome === "failed");
  return failed ? "Applied with problems" : "Setup applied";
}

function ChangeGroups({ operations, catalog }: { operations: Operation[]; catalog: CatalogIndex }) {
  return (
    <>
      {GROUPS.map((group) => {
        const members = operations.filter((operation) => group.providers.includes(operation.provider));
        if (members.length === 0) return null;
        return (
          <div key={group.label} style={{ marginBottom: 14 }}>
            <div className="side-heading" style={{ marginTop: 0 }}>
              {group.label}
            </div>
            <div className="list">
              {members.map((operation) => (
                <div className="list-row" key={operation.id} style={{ minHeight: 36 }}>
                  <div className="row-main">
                    <div className="row-title">{operationTitle(catalog, operation.id)}</div>
                  </div>
                  <div className="row-trailing">
                    {operation.provider === "config-adapter" ? (
                      <span className="badge orange">Manual setup</span>
                    ) : (
                      <span className="mono">{operation.requested_state}</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </>
  );
}

const OUTCOME_ICON: Record<OperationOutcome, typeof CheckCircle2> = {
  succeeded: CheckCircle2,
  failed: XCircle,
  skipped: MinusCircle,
};

export function ApplySummary({ result, catalog }: { result: ApplyResult; catalog: CatalogIndex }) {
  const counts = result.results.reduce(
    (totals, entry) => ({ ...totals, [entry.outcome]: totals[entry.outcome] + 1 }),
    { succeeded: 0, failed: 0, skipped: 0 } as Record<OperationOutcome, number>,
  );
  return (
    <>
      <div className="stats">
        <div className="stat">
          <div className="value">{counts.succeeded}</div>
          <div className="label">Done</div>
        </div>
        <div className="stat">
          <div className="value" style={{ color: counts.failed ? "var(--red)" : undefined }}>
            {counts.failed}
          </div>
          <div className="label">Failed</div>
        </div>
        <div className="stat">
          <div className="value">{counts.skipped}</div>
          <div className="label">Manual / skipped</div>
        </div>
      </div>
      {result.bootstrap.state === "failed" && (
        <div className="callout error selectable" style={{ marginBottom: 12 }}>
          <AlertTriangle size={14} />
          <span style={{ whiteSpace: "pre-wrap" }}>
            mise couldn’t finish installing: {result.bootstrap.message}
          </span>
        </div>
      )}
      {result.followUps.length > 0 && (
        <div className="callout warn" style={{ marginBottom: 12 }}>
          <AlertTriangle size={14} />
          <div>
            <strong>Still to do</strong>
            {result.followUps.map((followUp) => (
              <div key={followUp}>{followUp}</div>
            ))}
          </div>
        </div>
      )}
      <ResultList results={result.results} catalog={catalog} />
      {result.recoveryGuidance.length > 0 && (
        <div className="callout info" style={{ marginTop: 12 }}>
          <Circle size={14} />
          <div>
            {result.recoveryGuidance.map((guidance) => (
              <div key={guidance}>{guidance}</div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}

export function ResultList({
  results,
  catalog,
}: {
  results: ApplyResult["results"];
  catalog: CatalogIndex;
}) {
  return (
    <div className="list">
      {results.map((entry) => {
        const Icon = OUTCOME_ICON[entry.outcome];
        return (
          <div className="list-row" key={entry.operation_id}>
            <Icon size={16} className={`outcome-icon ${entry.outcome}`} />
            <div className="row-main">
              <div className="row-title">{operationTitle(catalog, entry.operation_id)}</div>
              {entry.message && (
                <div className="row-subtitle" title={entry.message}>
                  {entry.message}
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

/** Turns lifecycle progress lines into friendlier text. */
function humanizeStep(step: string, catalog: CatalogIndex): string {
  if (step.startsWith("applying ")) {
    return `Checking ${operationTitle(catalog, step.slice("applying ".length))}`;
  }
  const map: Record<string, string> = {
    "writing generated configuration": "Writing the mise configuration file",
    "recording ownership ledger": "Recording what powermise owns",
    "converging mise environment (installs tools and packages)": "Installing tools and packages with mise",
    "persisting run report": "Saving the report",
  };
  return map[step] ?? step.charAt(0).toUpperCase() + step.slice(1);
}
