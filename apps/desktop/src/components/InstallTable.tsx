import { PLATFORMS, PLATFORM_LABEL, type Platform, type Tool } from "../api";
import { PROVIDER_HINT, PROVIDER_SHORT, providerFor } from "../catalog";

/** How each tool installs on every platform: one row per tool, one column
 *  per platform, with this computer's column marked. */
export function InstallTable({
  tools,
  hostTarget,
  platforms = PLATFORMS,
}: {
  tools: Tool[];
  hostTarget: Platform;
  /** Columns to show; defaults to every platform. */
  platforms?: Platform[];
}) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Tool</th>
          {platforms.map((platform) => (
            <th key={platform} className="provider-cell">
              {PLATFORM_LABEL[platform]}
              {platform === hostTarget && " •"}
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
            {platforms.map((platform) => {
              const provider = providerFor(tool, platform);
              return (
                <td key={platform} className="provider-cell">
                  {provider ? (
                    <span
                      className={`badge${platform === hostTarget ? " accent" : ""}`}
                      title={`${PROVIDER_HINT[provider.kind]}\n${provider.package} ${provider.version}`}
                    >
                      {PROVIDER_SHORT[provider.kind]}
                    </span>
                  ) : (
                    <span className="dash" title={`Not available on ${PLATFORM_LABEL[platform]}`}>
                      —
                    </span>
                  )}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
