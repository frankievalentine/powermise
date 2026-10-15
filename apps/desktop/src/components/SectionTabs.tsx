import type { AppContext } from "../App";
import { PageTabs } from "./ui";

/** My Tools: what's installed, and what has an update. */
export function MyToolsTabs({ context, current }: { context: AppContext; current: "installed" | "updates" }) {
  const updates = Object.keys(context.mise.outdated ?? {}).length;
  return (
    <PageTabs
      label="My Tools"
      current={current}
      onSelect={(kind) => context.navigate({ kind })}
      tabs={[
        { value: "installed", label: "Installed" },
        { value: "updates", label: "Updates", badge: updates },
      ]}
    />
  );
}

/** Browse Agents: the agents, and what they can bring along. */
export function AgentsTabs({
  context,
  current,
}: {
  context: AppContext;
  current: "harnesses" | "tools" | "mcp" | "integrations" | "skills";
}) {
  return (
    <PageTabs
      label="Browse agents"
      current={current}
      onSelect={(kind) => context.navigate({ kind })}
      tabs={[
        { value: "harnesses", label: "Agents" },
        { value: "tools", label: "Agent Tools" },
        { value: "mcp", label: "MCP Servers" },
        { value: "integrations", label: "Plugins" },
        { value: "skills", label: "Skills" },
      ]}
    />
  );
}
