import { CheckCircle2, History, Loader2, MinusCircle, XCircle } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { CommandBlock } from "../../components/ui";

const time = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });

export function ActivityView({ context }: { context: AppContext }) {
  const activity = useAsync(() => api.activity(), [context.mise.revision]);
  const entries = activity.data ?? [];

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Activity</h1>
            <p className="page-subtitle">
              Every command powermise has run, newest first — kept between launches. Copy any of them to run it yourself.
            </p>
          </div>
        </div>
        {activity.data && entries.length === 0 && (
          <div className="empty">
            <History size={28} color="var(--text-tertiary)" />
            <h3>Nothing yet</h3>
            <p>Installs, upgrades, and removals will show up here.</p>
          </div>
        )}
        <div className="activity">
          {entries.map((entry) => {
            const Icon = !entry.finishedAt
              ? Loader2
              : entry.outcome?.success
                ? CheckCircle2
                : entry.outcome?.cancelled
                  ? MinusCircle
                  : XCircle;
            const status = !entry.finishedAt
              ? "running"
              : entry.outcome?.success
                ? "succeeded"
                : entry.outcome?.cancelled
                  ? "skipped"
                  : "failed";
            return (
              <div className="activity-entry" key={entry.id}>
                <Icon size={16} className={`outcome-icon ${status}${!entry.finishedAt ? " spin" : ""}`} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <CommandBlock command={entry.command} />
                  <div className="row-subtitle" style={{ marginTop: 4 }}>
                    {time.format(entry.startedAt)}
                    {entry.finishedAt &&
                      ` · ${Math.max(1, Math.round((entry.finishedAt - entry.startedAt) / 1000))}s`}
                    {entry.error && ` · ${entry.error}`}
                    {entry.outcome && !entry.outcome.success && !entry.outcome.cancelled &&
                      ` · exited with code ${entry.outcome.code ?? "?"}`}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
