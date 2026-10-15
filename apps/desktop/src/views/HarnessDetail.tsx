import { CheckCircle2, Plus, XCircle } from "lucide-react";
import { PLATFORMS, PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { MCP_CLIENT_TITLE, MCP_CLIENT_UNSUPPORTED, categoryLabel, isAvailable, targetTitle } from "../catalog";
import { InstallTable } from "../components/InstallTable";
import { Monogram, Section } from "../components/ui";

export function HarnessDetail({ context, id }: { context: AppContext; id: string }) {
  const { catalog, env, navigate } = context;
  const harness = catalog.harnessById.get(id);
  if (!harness) {
    return (
      <div className="empty">
        <h3>Unknown harness</h3>
      </div>
    );
  }
  const target = env.hostTarget;
  const available = isAvailable(harness, target);
  const configs = harness.integration_ids
    .map((entryId) => catalog.configById.get(entryId))
    .filter((entry) => entry !== undefined);

  const toolsFor = (ids: string[]) =>
    ids.map((toolId) => catalog.toolById.get(toolId)).filter((tool) => tool !== undefined);
  // Only the platforms this agent supports.
  const platforms = PLATFORMS.filter((platform) => harness.supported_platforms.includes(platform));

  return (
    <div className="content">
      <div className="page">
        <div className="detail-hero">
          <Monogram id={harness.id} title={harness.title} size={64} />
          <div>
            <h1 className="page-title">{harness.title}</h1>
            <p className="page-subtitle">{categoryLabel(harness.category)}</p>
          </div>
          <div className="detail-actions">
            <button
              className="button primary large"
              disabled={!available}
              title={available ? undefined : `Not available on ${PLATFORM_LABEL[target]}`}
              onClick={() => navigate({ kind: "editor", seed: [harness.id] })}
            >
              <Plus size={15} /> New Setup with {harness.title}
            </button>
          </div>
        </div>
        <p className="page-subtitle selectable" style={{ marginTop: 16 }}>
          {harness.description}
        </p>

        <Section title="Platforms">
          <div className="platform-grid">
            {PLATFORMS.map((platform) => {
              const on = harness.supported_platforms.includes(platform);
              return (
                <div key={platform} className={`platform-tile ${on ? "on" : "off"}`}>
                  {on ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
                  {PLATFORM_LABEL[platform]}
                  {platform === target && <span className="badge accent">This computer</span>}
                </div>
              );
            })}
          </div>
        </Section>

        {harness.required_tool_ids.length > 0 && (
          <Section
            title="What it installs"
            note="How mise installs each part on each system. Hover a badge for the package and its pinned version."
          >
            <InstallTable tools={toolsFor(harness.required_tool_ids)} hostTarget={target} platforms={platforms} />
          </Section>
        )}

        {harness.desktop_tool_ids.length > 0 && (
          <Section
            title="Desktop app"
            note={
              harness.desktop_required
                ? "Installed automatically: the app is the main part of this harness."
                : "Optional: turn on “Include desktop apps” in a setup to install it."
            }
          >
            <InstallTable tools={toolsFor(harness.desktop_tool_ids)} hostTarget={target} platforms={platforms} />
          </Section>
        )}

        {harness.mcp_client && (
          <Section
            title="MCP servers"
            note={
              MCP_CLIENT_UNSUPPORTED[harness.mcp_client] ??
              `A setup adds the servers you pick to ${MCP_CLIENT_TITLE[harness.mcp_client]}${
                harness.desktop_mcp_client ? ` and the ${MCP_CLIENT_TITLE[harness.desktop_mcp_client]}` : ""
              } through mise. These are checked for you:`
            }
          >
            {harness.mcp_server_ids.length > 0 && !MCP_CLIENT_UNSUPPORTED[harness.mcp_client] && (
              <div className="list">
                {harness.mcp_server_ids.map((id) => {
                  const server = catalog.mcpById.get(id);
                  return (
                    <div className="list-row" key={id}>
                      <Monogram id={id} title={server?.title ?? id} size={28} />
                      <div className="row-main">
                        <div className="row-title">{server?.title ?? id}</div>
                        <div className="row-subtitle">{server?.description}</div>
                      </div>
                      <div className="row-trailing">
                        {server && <span className="badge">{server.url ? "Remote" : "mise"}</span>}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </Section>
        )}

        {configs.length > 0 && (
          <Section
            title="Integrations"
            note="powermise doesn’t configure these yet. They’re listed so you know what to set up in the agent."
          >
            <div className="list">
              {configs.map((entry) => (
                <div className="list-row" key={entry.id}>
                  <div className="row-main">
                    <div className="row-title">{entry.title}</div>
                    <div className="row-subtitle">{entry.description}</div>
                  </div>
                  <div className="row-trailing">
                    <span className="badge">
                      {targetTitle(catalog, entry.providers[0]?.config_target ?? "")}
                    </span>
                    <span className="badge orange">Manual setup</span>
                  </div>
                </div>
              ))}
            </div>
          </Section>
        )}
      </div>
    </div>
  );
}
