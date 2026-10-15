import { useMemo } from "react";
import { PLATFORMS, PLATFORM_LABEL } from "../api";
import type { AppContext } from "../App";
import { PROVIDER_LABEL, providerFor } from "../catalog";

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
        <div className="page-header">
          <div>
            <h1 className="page-title">Tools</h1>
            <p className="page-subtitle">
              Every program a harness can install, and which package manager provides it on each
              platform. Versions are pinned by the catalog.
            </p>
          </div>
        </div>
        <table className="table">
          <thead>
            <tr>
              <th>Tool</th>
              {PLATFORMS.map((platform) => (
                <th key={platform} className="provider-cell">
                  {PLATFORM_LABEL[platform]}
                  {platform === env.hostTarget && " •"}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {tools.map((tool) => (
              <tr key={tool.id}>
                <td>
                  <div className="row-title">{tool.title}</div>
                  <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                    {tool.unsupported_reason ?? tool.description}
                  </div>
                </td>
                {PLATFORMS.map((platform) => {
                  const provider = providerFor(tool, platform);
                  return (
                    <td key={platform} className="provider-cell">
                      {provider ? (
                        <span
                          className={`badge${platform === env.hostTarget ? " accent" : ""}`}
                          title={`${provider.package} ${provider.version}`}
                        >
                          {PROVIDER_LABEL[provider.kind]}
                        </span>
                      ) : (
                        <span className="dash">—</span>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
