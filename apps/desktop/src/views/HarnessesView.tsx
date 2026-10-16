import { useMemo, useState } from "react";
import { PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { categoryLabel, isAvailable } from "../catalog";
import { Monogram, PlatformChips, Segmented } from "../components/ui";
import { HarnessesTabs } from "../components/SectionTabs";

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

  return (
    <div className="content">
      <div className="page">
        <HarnessesTabs context={context} current="harnesses" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Harnesses</h1>
            <p className="page-subtitle">
              The apps and CLIs that run AI coding agents (Claude Code, Codex, Cursor…). Pick the ones you want
              and powermise installs everything they need with mise.
            </p>
          </div>
          <Segmented label="Filter harnesses" value={filter} options={FILTERS} onChange={setFilter} />
        </div>

        {harnesses.length === 0 ? (
          <div className="empty">
            <h3>No matches</h3>
            <p>Try a different search or filter.</p>
          </div>
        ) : (
          <div className="grid">
            {harnesses.map((harness) => {
              const available = isAvailable(harness, env.hostTarget);
              const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;
              const tools = harness.required_tool_ids.length;
              const apps = harness.desktop_tool_ids.length;
              const mcp = harness.mcp_server_ids.length;
              // What picking it adds; desktop apps are opt-in unless the app is the harness.
              const adds = [
                tools > 0 && plural(tools, "tool"),
                apps > 0 && (harness.desktop_required ? plural(apps, "app") : plural(apps, "optional app")),
                mcp > 0 && `${mcp} MCP`,
              ].filter(Boolean);
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
                      {adds.length ? adds.join(" · ") : "Nothing extra to install"}
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
