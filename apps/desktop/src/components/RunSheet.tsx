import { useEffect, useRef, useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2, XCircle } from "lucide-react";
import { api, onMiseFinished, onMiseOutput, type ActivityEntry, type MiseAction } from "../api";
import { CommandBlock } from "./ui";
import { Sheet } from "./Sheet";

/** What a change is, in words, before it runs. */
export interface RunRequest {
  action: MiseAction;
  title: string;
  description: string;
  /** The confirm button's label, e.g. "Install". */
  verb: string;
  destructive?: boolean;
  /** Start without the confirm step — for flows that already showed the
   *  exact command (like the task form). */
  autoStart?: boolean;
}

type Phase =
  | { kind: "confirm" }
  | { kind: "running"; jobId: number }
  | { kind: "done"; entry: ActivityEntry }
  | { kind: "error"; message: string };

/** Confirms a mise change, runs it with live output and cancel, and reports
 *  the result. Every change in the app goes through here. */
export function RunSheet({
  request,
  onClose,
}: {
  request: RunRequest | null;
  onClose: () => void;
}) {
  const [phase, setPhase] = useState<Phase>({ kind: "confirm" });
  const [command, setCommand] = useState("");
  // Events are buffered by job id: a job can print (or even finish) before
  // `run` has returned its id.
  const [output, setOutput] = useState<Record<number, string[]>>({});
  const [finished, setFinished] = useState<Record<number, ActivityEntry>>({});
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    if (!request) return;
    setPhase({ kind: "confirm" });
    setCommand("");
    api
      .describe(request.action)
      .then(setCommand)
      .catch((error: Error) => setPhase({ kind: "error", message: error.message }));
  }, [request]);

  useEffect(() => {
    const unlisten = [
      onMiseOutput((event) =>
        setOutput((current) => ({
          ...current,
          [event.jobId]: [...(current[event.jobId] ?? []), event.text],
        })),
      ),
      onMiseFinished((entry) => setFinished((current) => ({ ...current, [entry.id]: entry }))),
    ];
    return () => {
      unlisten.forEach((promise) => promise.then((stop) => stop()));
    };
  }, []);

  const jobId = phase.kind === "running" ? phase.jobId : phase.kind === "done" ? phase.entry.id : null;
  const lines = jobId === null ? [] : (output[jobId] ?? []);

  useEffect(() => {
    if (phase.kind === "running" && finished[phase.jobId]) {
      setPhase({ kind: "done", entry: finished[phase.jobId] });
    }
  }, [phase, finished]);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [lines.length]);

  const start = async () => {
    if (!request) return;
    try {
      setPhase({ kind: "running", jobId: await api.run(request.action) });
    } catch (error) {
      setPhase({ kind: "error", message: (error as Error).message });
    }
  };

  // Flows that already showed the command start as soon as it's resolved.
  useEffect(() => {
    if (request?.autoStart && command && phase.kind === "confirm") void start();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request, command]);

  if (!request) return <Sheet open={false} onDismiss={onClose} labelledBy="run-title">{null}</Sheet>;

  const running = phase.kind === "running";
  const result = phase.kind === "done" ? phase.entry : null;
  const succeeded = !!result?.outcome?.success;

  return (
    <Sheet open onDismiss={onClose} dismissible={!running} labelledBy="run-title">
      <div className="sheet-head">
        <h2 id="run-title">
          {result
            ? result.outcome?.cancelled
              ? "Cancelled"
              : succeeded
                ? "Done"
                : "That didn’t work"
            : phase.kind === "error"
              ? "Couldn’t start"
              : request.title}
        </h2>
        {phase.kind === "confirm" && <p>{request.description}</p>}
      </div>
      <div className="sheet-body">
        {command && <CommandBlock command={command} />}
        {(running || result || lines.length > 0) && (
          <pre className="code-view run-log" ref={logRef} aria-live="polite">
            {lines.length === 0 && running ? "Starting…" : lines.join("\n")}
          </pre>
        )}
        {result && !succeeded && !result.outcome?.cancelled && (
          <div className="callout error selectable" style={{ marginTop: 10 }}>
            <AlertTriangle size={14} />
            <span>
              {result.error ??
                `mise exited with code ${result.outcome?.code ?? "unknown"}. The output above says why.`}
            </span>
          </div>
        )}
        {phase.kind === "error" && (
          <div className="callout error selectable" style={{ marginTop: 10 }}>
            <AlertTriangle size={14} />
            <span>{phase.message}</span>
          </div>
        )}
      </div>
      <div className="sheet-foot">
        {phase.kind === "confirm" && (
          <>
            <button className="button large" onClick={onClose}>
              Cancel
            </button>
            <button
              className={`button large ${request.destructive ? "danger" : "primary"}`}
              onClick={start}
              disabled={!command}
            >
              {request.verb}
            </button>
          </>
        )}
        {running && (
          <>
            <span className="run-status">
              <Loader2 size={14} className="spin" /> Running…
            </span>
            <button className="button large" onClick={() => api.cancel(phase.jobId)}>
              Stop
            </button>
          </>
        )}
        {(result || phase.kind === "error") && (
          <>
            {result && (
              <span className="run-status">
                {succeeded ? (
                  <CheckCircle2 size={14} className="outcome-icon succeeded" />
                ) : (
                  <XCircle size={14} className="outcome-icon failed" />
                )}
                {succeeded ? "Finished" : result.outcome?.cancelled ? "Stopped" : "Failed"}
              </span>
            )}
            <button className="button primary large" onClick={onClose}>
              Done
            </button>
          </>
        )}
      </div>
    </Sheet>
  );
}
