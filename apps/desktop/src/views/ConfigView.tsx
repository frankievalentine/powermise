import { useMemo } from "react";
import { Info } from "lucide-react";
import type { ConfigEntry } from "../api";
import type { AppContext } from "../App";
import { targetTitle } from "../catalog";
import { Section } from "../components/ui";

const COPY = {
  mcp: {
    title: "MCP Servers",
    subtitle: "Model Context Protocol servers give an agent extra abilities, like reading current library docs.",
  },
  integrations: {
    title: "Integrations",
    subtitle: "Connections between an agent and the services you already use.",
  },
};

export function ConfigView({
  context,
  query,
  kind,
}: {
  context: AppContext;
  query: string;
  kind: "mcp" | "integrations";
}) {
  const { catalog } = context;
  const entries = kind === "mcp" ? catalog.mcpServers : catalog.integrations;

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const byTarget = new Map<string, ConfigEntry[]>();
    for (const entry of entries) {
      if (
        needle &&
        !entry.title.toLowerCase().includes(needle) &&
        !entry.description.toLowerCase().includes(needle)
      ) {
        continue;
      }
      const target = entry.providers[0]?.config_target ?? "other";
      byTarget.set(target, [...(byTarget.get(target) ?? []), entry]);
    }
    return [...byTarget.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [entries, query]);

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">{COPY[kind].title}</h1>
            <p className="page-subtitle">{COPY[kind].subtitle}</p>
          </div>
        </div>
        <div className="banner warn">
          <Info size={16} />
          <span>
            Powermise lists these but doesn’t configure them yet. Applying a setup marks them
            “Manual setup” rather than pretending they’re done.
          </span>
        </div>
        {groups.map(([target, group]) => (
          <Section key={target} title={`For ${targetTitle(catalog, target)}`}>
            <div className="list">
              {group.map((entry) => (
                <div className="list-row" key={entry.id}>
                  <div className="row-main">
                    <div className="row-title">{entry.title}</div>
                    <div className="row-subtitle">{entry.description}</div>
                  </div>
                  <div className="row-trailing">
                    <span className="badge orange">Manual setup</span>
                  </div>
                </div>
              ))}
            </div>
          </Section>
        ))}
      </div>
    </div>
  );
}
