import { useEffect, useState } from "react";
import { ArrowUpCircle, Download, ExternalLink, Link2, Loader2, ShieldCheck } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { backendLabel, homeFrom, isGlobalPath, sourceLabel, upgradeTarget } from "../../mise";
import { CommandBlock, Monogram, Section, Switch } from "../../components/ui";
import { ExternalLinkButton, LinkSheet, canOpen } from "../../components/machine";

const SECURITY_LABEL: Record<string, string> = {
  checksum: "Checksum",
  github_attestations: "GitHub attestations",
  slsa: "SLSA provenance",
  cosign: "Cosign signature",
  minisign: "Minisign signature",
  gpg: "GPG signature",
};

function securityLabel(check: { type: string } & Record<string, unknown>): string {
  const label = SECURITY_LABEL[check.type] ?? check.type.replace(/_/g, " ");
  return check.type === "checksum" && typeof check.algorithm === "string" ? `${label} (${check.algorithm})` : label;
}

export function ToolPage({ context, name }: { context: AppContext; name: string }) {
  const { mise, env, run } = context;
  const home = homeFrom(env.miseConfigDir);
  const entry = mise.registryByName.get(name);
  const versions = mise.inventory?.[name] ?? [];
  const outdated = mise.outdated?.[name];
  const global = versions.find(
    (version) => version.source && isGlobalPath(version.source.path, env.miseConfigDir),
  );

  const remote = useAsync(() => api.remoteVersions(name), [name]);
  const info = useAsync(() => api.toolInfo(name), [name, mise.revision]);
  // Options mise reports for this tool, leaving out unset and empty ones.
  const options = Object.entries(info.data?.tool_options ?? {}).filter(
    ([key, value]) =>
      key !== "auto_update" &&
      value !== null &&
      value !== false &&
      !(Array.isArray(value) && value.length === 0) &&
      !(typeof value === "object" && !Array.isArray(value) && Object.keys(value as object).length === 0),
  );
  // Newest first; mise lists oldest first.
  const available = [...(remote.data ?? [])].reverse();
  const [selected, setSelected] = useState("latest");
  const [linkOpen, setLinkOpen] = useState(false);
  useEffect(() => setSelected("latest"), [name]);

  const chosen = selected === "latest" ? (available[0]?.version ?? "latest") : selected;
  const alreadyGlobal = global?.version === chosen;

  const install = () =>
    run({
      action: { kind: "useGlobal", tool: name, version: chosen },
      title: global ? `Switch ${name} to ${chosen}?` : `Install ${name} ${chosen}?`,
      description: global
        ? `mise installs ${chosen} if needed and makes it your default everywhere, replacing ${global.version} in your global config.`
        : `mise downloads ${name} ${chosen} and makes it your default everywhere.`,
      verb: global ? "Switch" : "Install",
    });

  return (
    <div className="content">
      <div className="page">
        <div className="detail-hero">
          <Monogram id={name} title={name.split(":").pop() ?? name} size={64} />
          <div style={{ minWidth: 0 }}>
            <h1 className="page-title">{name}</h1>
            <p className="page-subtitle">
              {global ? (
                <span className="badge green">Installed · {global.version}</span>
              ) : versions.length ? (
                <span className="badge">Installed, not your default</span>
              ) : (
                <span className="badge">Not installed</span>
              )}
            </p>
          </div>
          <div className="detail-actions">
            {outdated && (
              <button
                className="button large"
                onClick={() =>
                  run({
                    action: { kind: "upgrade", tool: name },
                    title: `Upgrade ${name}?`,
                    description: `${name} ${outdated.current ?? ""} → ${upgradeTarget(outdated)}. mise installs it and updates your config to match.`,
                    verb: "Upgrade",
                  })
                }
              >
                <ArrowUpCircle size={15} /> Upgrade to {upgradeTarget(outdated)}
              </button>
            )}
          </div>
        </div>

        {entry?.description && (
          <p className="page-subtitle selectable" style={{ marginTop: 16, maxWidth: 680 }}>
            {entry.description}
          </p>
        )}

        <Section
          title={global ? "Change version" : "Install"}
          note="Sets the version you get in every terminal. Projects can still ask for their own."
        >
          <div className="install-row">
            <select
              className="select"
              value={selected}
              onChange={(event) => setSelected(event.target.value)}
              aria-label="Version"
              disabled={remote.loading && !remote.data}
            >
              <option value="latest">
                {available[0] ? `Latest (${available[0].version})` : "Latest"}
              </option>
              {available.slice(0, 200).map((version) => (
                <option key={version.version} value={version.version}>
                  {version.version}
                </option>
              ))}
            </select>
            <button
              className="button primary large"
              onClick={install}
              disabled={alreadyGlobal || !env.misePath}
            >
              {remote.loading && !remote.data ? (
                <Loader2 size={15} className="spin" />
              ) : (
                <Download size={15} />
              )}
              {alreadyGlobal ? "Already your default" : global ? "Switch Version" : "Install"}
            </button>
          </div>
          {remote.error && (
            <p className="field-hint error selectable">Couldn’t list versions: {remote.error}</p>
          )}
        </Section>

        {versions.length > 0 && (
          <Section title="On this computer">
            <div className="list">
              {versions.map((version) => {
                const isGlobal =
                  !!version.source && isGlobalPath(version.source.path, env.miseConfigDir);
                return (
                  <div className="list-row" key={`${version.version}-${version.source?.path ?? ""}`}>
                    <div className="row-main">
                      <div className="row-title mono">{version.version}</div>
                      <div className="row-subtitle">
                        {sourceLabel(version, env.miseConfigDir, home)}
                        {version.requested_version && version.requested_version !== version.version &&
                          ` · asks for ${version.requested_version}`}
                      </div>
                    </div>
                    <div className="row-trailing">
                      {!version.installed && <span className="badge orange">Not installed</span>}
                      {version.active && <span className="badge green">Active</span>}
                      {isGlobal ? (
                        <button
                          className="button"
                          onClick={() =>
                            run({
                              action: { kind: "unuseGlobal", tool: name },
                              title: `Remove ${name} from your global tools?`,
                              description: `mise removes ${name} from your global config and deletes versions nothing else uses. Projects that ask for ${name} keep working.`,
                              verb: "Remove",
                              destructive: true,
                            })
                          }
                        >
                          Remove from Global
                        </button>
                      ) : (
                        version.installed && (
                          <button
                            className="button"
                            onClick={() =>
                              run({
                                action: { kind: "uninstall", tool: name, version: version.version },
                                title: `Uninstall ${name} ${version.version}?`,
                                description: `mise deletes this version from disk. Any config that asks for it will reinstall it when needed.`,
                                verb: "Uninstall",
                                destructive: true,
                              })
                            }
                          >
                            Uninstall
                          </button>
                        )
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Section>
        )}

        {(info.data || entry) && (
          <Section title="Details">
            <div className="list">
              <div className="list-row">
                <div className="row-main">
                  <div className="row-title">Installs from</div>
                  {entry && entry.backends.length > 1 && (
                    <div className="row-subtitle">mise uses the first one that works on this computer.</div>
                  )}
                </div>
                <div className="row-trailing" style={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
                  {(info.data ? [info.data.backend, ...(entry?.backends ?? []).filter((b) => b !== info.data!.backend)] : entry!.backends).map(
                    (backend, index) => (
                      <span
                        key={backend}
                        className={`badge${index === 0 && info.data ? " accent" : ""}`}
                        title={index === 0 && info.data ? `${backend} — what mise uses here` : backend}
                      >
                        {backendLabel(backend)} · {backend.split(":").slice(1).join(":")}
                      </span>
                    ),
                  )}
                </div>
              </div>
              {info.data && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Download checks</div>
                    <div className="row-subtitle">
                      {info.data.security.length
                        ? "mise verifies every download before installing it."
                        : "This source doesn’t publish checksums or signatures mise can verify."}
                    </div>
                  </div>
                  <div className="row-trailing" style={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
                    {info.data.security.length ? (
                      info.data.security.map((check) => (
                        <span
                          key={check.type}
                          className="badge green"
                          title={Object.entries(check)
                            .filter(([key]) => key !== "type")
                            .map(([key, value]) => `${key}: ${String(value)}`)
                            .join("\n")}
                        >
                          <ShieldCheck size={11} /> {securityLabel(check)}
                        </span>
                      ))
                    ) : (
                      <span className="badge orange">Not verified</span>
                    )}
                  </div>
                </div>
              )}
              {info.data && "auto_update" in info.data.tool_options && global && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Update automatically</div>
                    <div className="row-subtitle">mise upgrades {name} in the background, within the version you asked for.</div>
                  </div>
                  <Switch
                    checked={info.data.tool_options.auto_update === true || info.data.tool_options.auto_update === "true"}
                    label={`Update ${name} automatically`}
                    onChange={(on) =>
                      run({
                        action: { kind: "toolAutoUpdate", tool: name, on },
                        title: `${on ? "Turn on" : "Turn off"} automatic updates for ${name}?`,
                        description: "mise writes this to the tool’s entry in your global config.",
                        verb: on ? "Turn On" : "Turn Off",
                      })
                    }
                  />
                </div>
              )}
              {options.length > 0 && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Options</div>
                  </div>
                  <div className="row-trailing mono selectable" style={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
                    {options.map(([key, value]) => (
                      <span key={key} className="badge">
                        {key} = {typeof value === "string" ? value : JSON.stringify(value)}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {entry && entry.aliases.length > 0 && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Also known as</div>
                  </div>
                  <div className="row-trailing">{entry.aliases.join(", ")}</div>
                </div>
              )}
              {info.data?.url && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Homepage</div>
                  </div>
                  <div className="row-trailing">
                    {canOpen(info.data.url) ? (
                      <ExternalLinkButton url={info.data.url}>{info.data.url.replace(/^https:\/\//, "")}</ExternalLinkButton>
                    ) : (
                      <span className="selectable mono">{info.data.url}</span>
                    )}
                  </div>
                </div>
              )}
              {available[0]?.release_url && (
                <div className="list-row">
                  <div className="row-main">
                    <div className="row-title">Latest release notes</div>
                  </div>
                  <div className="row-trailing">
                    {canOpen(available[0].release_url) ? (
                      <ExternalLinkButton url={available[0].release_url}>{available[0].version}</ExternalLinkButton>
                    ) : (
                      <>
                        <ExternalLink size={13} />
                        <span className="selectable mono">{available[0].release_url}</span>
                      </>
                    )}
                  </div>
                </div>
              )}
            </div>
          </Section>
        )}

        <Section
          title="Use an existing install"
          note={`Already have ${name} installed outside mise — built by hand, or by another tool? mise can use it as a version.`}
        >
          <button className="button" onClick={() => setLinkOpen(true)} disabled={!env.misePath}>
            <Link2 size={14} /> Link a Folder…
          </button>
        </Section>

        <Section title="In the terminal">
          <CommandBlock command={`mise use -g ${name}@${chosen}`} />
        </Section>
      </div>
      <LinkSheet open={linkOpen} tool={name} onDismiss={() => setLinkOpen(false)} context={context} />
    </div>
  );
}
