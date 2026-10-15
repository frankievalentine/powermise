import { useMemo } from "react";
import type { AppContext } from "../App";
import { InstallTable } from "../components/InstallTable";
import { AgentsTabs } from "../components/SectionTabs";

export function ToolsView({ context, query }: { context: AppContext; query: string }) {
  const { catalog, env } = context;
  const tools = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return catalog.tools
      .filter(
        (tool) =>
          !needle ||
          tool.title.toLowerCase().includes(needle) ||
          tool.description.toLowerCase().includes(needle),
      )
      .sort((a, b) => a.title.localeCompare(b.title));
  }, [catalog.tools, query]);

  return (
    <div className="content">
      <div className="page">
        <AgentsTabs context={context} current="tools" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Agents</h1>
            <p className="page-subtitle">
              Every program a harness can install, and which package manager provides it on each
              platform. Versions are pinned by the catalog.
            </p>
          </div>
        </div>
        <InstallTable tools={tools} hostTarget={env.hostTarget} />
      </div>
    </div>
  );
}
