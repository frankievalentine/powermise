import { useMemo, useState } from "react";
import { Info } from "lucide-react";
import { PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { categoryLabel, isAvailable, titleFor } from "../catalog";
import { Monogram, PlatformChips, Segmented } from "../components/ui";

type Filter = "all" | "terminal" | "ide" | "gui";

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "terminal", label: "Terminal" },
  { value: "ide", label: "Editors" },
  { value: "gui", label: "Desktop Apps" },
];

export function HarnessesView({ context, query }: { context: AppContext; query: string }) {
  const { catalog, env, navigate } = context;
  const [filter, setFilter] = useState<Filter>("all");

  const harnesses = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return catalog.harnesses
      .filter((harness) => filter === "all" || harness.category === filter)
      .filter(
        (harness) =>
          !needle ||
          harness.title.toLowerCase().includes(needle) ||
          harness.description.toLowerCase().includes(needle),
      )
      .sort((a, b) => {
        // Available-here first, then alphabetical.
        const availability =
          Number(isAvailable(b, env.hostTarget)) - Number(isAvailable(a, env.hostTarget));
        return availability || a.title.localeCompare(b.title);
      });
  }, [catalog.harnesses, filter, query, env.hostTarget]);

  const baseTools = catalog.base?.required_tool_ids.map((id) => titleFor(catalog, id)) ?? [];

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Harnesses</h1>
            <p className="page-subtitle">
              AI coding agents and the editors they live in. Pick the ones you want and powermise
              installs everything they need with mise.
            </p>
          </div>
          <Segmented label="Filter harnesses" value={filter} options={FILTERS} onChange={setFilter} />
        </div>

        {baseTools.length > 0 && (
          <div className="banner">
            <Info size={16} />
            <span>
              Every setup also includes the basics: {baseTools.join(", ")}.
            </span>
          </div>
        )}

        {harnesses.length === 0 ? (
          <div className="empty">
            <h3>No matches</h3>
            <p>Try a different search or filter.</p>
          </div>
        ) : (
          <div className="grid">
            {harnesses.map((harness) => {
              const available = isAvailable(harness, env.hostTarget);
              const tools = harness.required_tool_ids.length + harness.desktop_tool_ids.length;
              const mcp = harness.mcp_server_ids.length;
              return (
                <button
                  key={harness.id}
                  className={`card${available ? "" : " unavailable"}`}
                  onClick={() => navigate({ kind: "harness", id: harness.id })}
                >
                  <div className="card-head">
                    <Monogram id={harness.id} title={harness.title} />
                    <div>
                      <div className="card-title">{harness.title}</div>
                      <div className="card-kind">
                        {available
                          ? categoryLabel(harness.category)
                          : `Not available on ${PLATFORM_LABEL[env.hostTarget]}`}
                      </div>
                    </div>
                  </div>
                  <p className="card-description">{harness.description}</p>
                  <div className="card-foot">
                    <PlatformChips supported={harness.supported_platforms} />
                    <span>
                      {tools === 0 ? "Basics only" : `${tools} ${tools === 1 ? "tool" : "tools"}`}
                      {mcp > 0 && ` · ${mcp} MCP`}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
