import { useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2 } from "lucide-react";
import { api, type NewerTool, type NewerToolsFile } from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import { prettyPath } from "../mise";
import { usePlatform } from "../platform";
import { Sheet } from "./Sheet";
import { CommandBlock, Section } from "./ui";

/** Newer versions of tools macOS ships: git, bash, make, and curl from
 *  Homebrew through mise (with their folders first on PATH), and Ruby as a
 *  mise tool. macOS's own copies stay as they are. */
export function NewerTools({ context }: { context: AppContext }) {
  const { mise, run } = context;
  const mac = usePlatform() === "darwin";
  const tools = useAsync(() => (mac ? api.newerTools() : Promise.resolve([])), [mise.revision, mac]);
  const [review, setReview] = useState<{ tools: NewerTool[]; file: NewerToolsFile | null } | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Ruby counts as newer once mise provides it.
  const rubyInUse = (mise.inventory?.ruby ?? []).find((version) => version.active)?.version;
  const using = (tool: NewerTool) => (tool.kind === "mise" ? !!rubyInUse : tool.usingNewer);
  const current = (tool: NewerTool) => (tool.kind === "mise" ? (rubyInUse ?? tool.newerVersion) : tool.newerVersion);
  const pending = (tools.data ?? []).filter((tool) => !using(tool) && tool.newerVersion);

  const open = async (picked: NewerTool[]) => {
    setError(null);
    try {
      const homebrew = picked.filter((tool) => tool.kind === "homebrew").map((tool) => tool.id);
      setReview({ tools: picked, file: homebrew.length ? await api.newerToolsPreview(homebrew) : null });
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const apply = async () => {
    if (!review) return;
    setError(null);
    const ids = review.tools.filter((tool) => tool.kind === "homebrew").map((tool) => tool.id);
    const ruby = review.tools.some((tool) => tool.kind === "mise");
    try {
      if (ids.length) await api.writeNewerTools(ids);
      setReview(null);
      run({
        action: { kind: "useNewerTools", ids, ruby },
        title: "Using newer tools",
        description: "",
        verb: "Install",
        autoStart: true,
      });
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  if (!mac || (tools.data && tools.data.length === 0)) return null;

  return (
    <Section
      title="Newer system tools"
      note="macOS ships older versions of these and keeps them as they are. mise can install current ones and put them ahead of macOS’s on your PATH, in every terminal where mise is turned on."
    >
      {tools.loading && !tools.data && (
        <p className="field-hint">
          <Loader2 size={12} className="spin" /> Checking versions…
        </p>
      )}
      {error && (
        <div className="callout error selectable">
          <AlertTriangle size={14} />
          <span>{error}</span>
        </div>
      )}
      {tools.data && (
        <>
          <div className="list">
            {tools.data.map((tool) => (
              <div className="list-row" key={tool.id}>
                <div className="row-main">
                  <div className="row-title mono">{tool.title}</div>
                  <div className="row-subtitle">{tool.note}</div>
                </div>
                <div className="row-trailing">
                  <span className="row-subtitle mono">
                    {tool.systemVersion ?? "—"} → {current(tool) ?? "—"}
                  </span>
                  {using(tool) ? (
                    <span className="badge green">
                      <CheckCircle2 size={11} aria-hidden /> Using {current(tool)}
                    </span>
                  ) : (
                    <button className="button" disabled={!tool.newerVersion} onClick={() => void open([tool])}>
                      Use Newer
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
          {pending.length > 1 && (
            <div className="detail-actions" style={{ justifyContent: "flex-end", marginTop: 10 }}>
              <button className="button" onClick={() => void open(pending)}>
                Use All Newer
              </button>
            </div>
          )}
        </>
      )}

      <Sheet open={!!review} onDismiss={() => setReview(null)} labelledBy="newer-tools-title">
        <div className="sheet-head">
          <h2 id="newer-tools-title">
            Use newer {review?.tools.map((tool) => tool.title).join(", ")}?
          </h2>
          <p>
            {review?.file
              ? `powermise adds them to ${prettyPath(review.file.path, context.env.home)} — your own config isn’t touched — and mise installs them. Delete that file to go back to macOS’s versions. Open a new terminal window afterwards.`
              : "mise installs the latest Ruby and makes it your global one."}
          </p>
        </div>
        <div className="sheet-body">
          {review?.file && (
            <>
              <div className="side-heading" style={{ marginTop: 0 }}>
                {prettyPath(review.file.path, context.env.home)}
              </div>
              <pre className="code-view selectable">{review.file.contents}</pre>
            </>
          )}
          <div className="side-heading">Then mise runs</div>
          <CommandBlock
            command={[
              ...(review?.file
                ? [
                    `mise bootstrap packages apply --yes ${review.tools
                      .filter((tool) => tool.kind === "homebrew")
                      .map((tool) => `brew:${tool.id}`)
                      .join(" ")}`,
                  ]
                : []),
              ...(review?.tools.some((tool) => tool.kind === "mise") ? ["mise use -g ruby@latest"] : []),
            ].join(" && ")}
          />
        </div>
        <div className="sheet-foot">
          <button className="button large" onClick={() => setReview(null)}>
            Cancel
          </button>
          <button className="button primary large" onClick={() => void apply()}>
            Install
          </button>
        </div>
      </Sheet>
    </Section>
  );
}
