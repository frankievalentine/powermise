import { CheckCircle2, Plus, XCircle } from "lucide-react";
import { PLATFORMS, PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { categoryLabel, isAvailable, providerFor, targetTitle } from "../catalog";
import { Monogram, ProviderBadge, Section } from "../components/ui";

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
  const configs = [...harness.mcp_server_ids, ...harness.integration_ids]
    .map((entryId) => catalog.configById.get(entryId))
    .filter((entry) => entry !== undefined);

  const toolRows = (ids: string[]) => (
    <div className="list">
      {ids.map((toolId) => {
        const tool = catalog.toolById.get(toolId);
        const provider = providerFor(tool, target);
        return (
          <div className="list-row" key={toolId}>
            <div className="row-main">
              <div className="row-title">{tool?.title ?? toolId}</div>
              <div className="row-subtitle">{tool?.description}</div>
            </div>
            <div className="row-trailing">
              {provider && (
                <span className="mono">
                  {provider.package} {provider.version}
                </span>
              )}
              <ProviderBadge provider={provider} />
            </div>
          </div>
        );
      })}
    </div>
  );

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
            note={`How each tool is installed on ${PLATFORM_LABEL[target]}, with its pinned version.`}
          >
            {toolRows(harness.required_tool_ids)}
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
            {toolRows(harness.desktop_tool_ids)}
          </Section>
        )}

        {configs.length > 0 && (
          <Section
            title="MCP servers & integrations"
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
