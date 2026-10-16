import { useMemo } from "react";
import { PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { MCP_CLIENT_TITLE, MCP_CLIENT_UNSUPPORTED, desktopMcpClient, mcpRunsAs } from "../catalog";
import { ExternalLinkButton, canOpen } from "../components/machine";
import { HarnessesTabs } from "../components/SectionTabs";
import { Monogram, Section } from "../components/ui";

/** The MCP servers a setup can add, and the agents powermise adds them to. */
export function McpServersView({ context, query }: { context: AppContext; query: string }) {
  const { catalog, env } = context;
  const servers = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return catalog.mcpServers.filter(
      (server) =>
        !needle || server.title.toLowerCase().includes(needle) || server.description.toLowerCase().includes(needle),
    );
  }, [catalog.mcpServers, query]);
  // Agents with an MCP config, and which recommend each server.
  const agents = catalog.harnesses.filter((harness) => harness.mcp_client);
  const recommendedBy = (id: string) =>
    agents.filter((harness) => harness.mcp_server_ids.includes(id)).map((harness) => harness.title);

  return (
    <div className="content">
      <div className="page">
        <HarnessesTabs context={context} current="mcp" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Harnesses</h1>
            <p className="page-subtitle">
              MCP servers give an agent extra abilities, like reading current library docs or using a browser. Pick
              them in a setup: mise installs each one and adds it to every agent in the setup.
            </p>
          </div>
        </div>

        <Section title="MCP servers">
          <div className="list">
            {servers.map((server) => {
              const agentsUsing = recommendedBy(server.id);
              return (
                <div className="list-row" key={server.id}>
                  <Monogram id={server.id} title={server.title} size={28} />
                  <div className="row-main">
                    <div className="row-title">{server.title}</div>
                    <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                      {server.description}
                    </div>
                    <div className="row-subtitle">
                      {mcpRunsAs(server)}
                      {server.tool && (
                        <>
                          {" · "}
                          <span className="mono">
                            {server.tool}@{server.version}
                          </span>
                        </>
                      )}
                      {agentsUsing.length > 0 && ` · Recommended for ${agentsUsing.join(", ")}`}
                    </div>
                  </div>
                  {canOpen(server.homepage) && (
                    <div className="row-trailing">
                      <ExternalLinkButton url={server.homepage}>About</ExternalLinkButton>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </Section>

        <Section
          title="Where they go"
          note={`powermise adds servers to each agent’s own config file on ${PLATFORM_LABEL[env.hostTarget]} with a mise dotfiles merge, which owns only those servers and leaves everything else in the file alone.`}
        >
          <div className="list">
            {agents.map((harness) => {
              const client = harness.mcp_client!;
              const unsupported = MCP_CLIENT_UNSUPPORTED[client];
              return (
                <div className="list-row" key={harness.id}>
                  <div className="row-main">
                    <div className="row-title">{harness.title}</div>
                    <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                      {unsupported ?? MCP_CLIENT_TITLE[client]}
                      {(() => {
                        const desktop = desktopMcpClient(catalog, harness, env.hostTarget);
                        return desktop && !unsupported
                          ? ` and the ${MCP_CLIENT_TITLE[desktop]} (when desktop apps are included)`
                          : "";
                      })()}
                    </div>
                  </div>
                  <div className="row-trailing">
                    <span className={`badge${unsupported ? " orange" : " green"}`}>
                      {unsupported ? "Add in the app" : "Set up by powermise"}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </Section>
      </div>
    </div>
  );
}
