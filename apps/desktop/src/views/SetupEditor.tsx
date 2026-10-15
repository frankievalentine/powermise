import { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, Info } from "lucide-react";
import {
  api,
  isValidSetupName,
  PLATFORMS,
  PLATFORM_LABEL,
  type Platform,
  type Preview,
  type SelectionDraft,
} from "../api";
import type { AppContext } from "../App";
import { CATEGORY_LABEL, isAvailable, titleFor } from "../catalog";
import { ApplySheet } from "../components/ApplySheet";
import { Checkbox, Monogram, ProviderBadge, Segmented, Switch } from "../components/ui";

export function SetupEditor({
  context,
  name: existingName,
  seed,
}: {
  context: AppContext;
  name?: string;
  seed?: string[];
}) {
  const { catalog, env, selections, navigate, refreshSelections, run, mise } = context;
  // Once a new setup is saved, the editor edits that saved setup.
  const [savedAs, setSavedAs] = useState<string | undefined>(existingName);
  const existing = selections.find((selection) => selection.name === savedAs);

  const [name, setName] = useState(existing?.name ?? "");
  const [harnesses, setHarnesses] = useState<string[]>(existing?.harnesses ?? seed ?? []);
  const [includeDesktopApps, setIncludeDesktopApps] = useState(existing?.includeDesktopApps ?? false);
  // `null` follows the agents' recommendations until you change the list.
  const [mcpPicks, setMcpPicks] = useState<string[] | null>(existing?.mcpServers ?? null);
  const [target, setTarget] = useState<Platform>(env.hostTarget);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [applyOpen, setApplyOpen] = useState(false);
  const request = useRef(0);

  const draft: SelectionDraft = useMemo(
    () => ({ name, harnesses, mcpServers: mcpPicks, includeDesktopApps }),
    [name, harnesses, mcpPicks, includeDesktopApps],
  );
  const recommendedMcp = useMemo(
    () => [...new Set(harnesses.flatMap((id) => catalog.harnessById.get(id)?.mcp_server_ids ?? []))],
    [harnesses, catalog.harnessById],
  );
  const mcpSelected = mcpPicks ?? recommendedMcp;
  const toggleMcp = (id: string) =>
    setMcpPicks(mcpSelected.includes(id) ? mcpSelected.filter((entry) => entry !== id) : [...mcpSelected, id]);
  const takesMcp = harnesses.some((id) => catalog.harnessById.get(id)?.mcp_client);

  // Live preview: recompile on every change, ignoring stale responses.
  useEffect(() => {
    const id = ++request.current;
    const timer = setTimeout(() => {
      api
        .preview(draft, target)
        .then((next) => {
          if (id !== request.current) return;
          setPreview(next);
          setPreviewError(null);
        })
        .catch((error: Error) => {
          if (id !== request.current) return;
          setPreview(null);
          setPreviewError(error.message);
        });
    }, 120);
    return () => clearTimeout(timer);
  }, [draft, target]);

  const nameTaken = !existing && selections.some((selection) => selection.name === name);
  const nameError =
    name.length === 0
      ? null
      : !isValidSetupName(name)
        ? "Use lowercase letters, numbers, and single hyphens — for example “work-laptop”."
        : nameTaken
          ? "A setup with this name already exists."
          : null;
  const canSave = isValidSetupName(name) && !nameTaken && harnesses.length > 0 && !!preview;
  // Setups rely on conf.d and `mise bootstrap --only`, both new in 2026.10.
  const miseTooOld = !!mise.info && !mise.info.recommendedMet;
  const canApply = canSave && target === env.hostTarget && !!env.misePath && !miseTooOld;

  const toggle = (id: string) =>
    setHarnesses((current) =>
      current.includes(id) ? current.filter((entry) => entry !== id) : [...current, id],
    );

  const save = async (thenApply: boolean) => {
    setSaving(true);
    setSaveError(null);
    try {
      await api.save(draft, !!existing);
      await refreshSelections();
      setSavedAs(name);
      if (thenApply) setApplyOpen(true);
      else navigate({ kind: "setup", name });
    } catch (error) {
      setSaveError((error as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const groups = useMemo(() => {
    const order = ["terminal", "ide", "gui"];
    return order
      .map((category) => ({
        category,
        harnesses: catalog.harnesses
          .filter((harness) => harness.category === category)
          .sort((a, b) => a.title.localeCompare(b.title)),
      }))
      .filter((group) => group.harnesses.length > 0);
  }, [catalog.harnesses]);

  const resolved = preview?.resolved;
  const cliTools = resolved
    ? resolved.toolIds.filter((id) => !resolved.desktopAppIds.includes(id))
    : [];
  const manual = resolved ? resolved.integrationIds : [];
  const installs = new Map(preview?.installs.map((install) => [install.toolId, install]));
  const hasOptionalDesktop = harnesses.some((id) => {
    const harness = catalog.harnessById.get(id);
    return harness && harness.desktop_tool_ids.length > 0 && !harness.desktop_required;
  });

  return (
    <div className="editor">
      <div className="editor-main">
        <div className="page">
          {existing ? (
            <h1 className="page-title">{existing.name}</h1>
          ) : (
            <>
              <input
                className="name-field"
                value={name}
                onChange={(event) => setName(event.target.value.toLowerCase().replace(/\s+/g, "-"))}
                placeholder="Name your setup"
                aria-label="Setup name"
                autoFocus
                spellCheck={false}
              />
              <div className={`field-hint${nameError ? " error" : ""}`}>
                {nameError ?? "Something short, like “work-laptop” or “ai-tools”."}
              </div>
            </>
          )}

          <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 18 }}>
            <span className="row-subtitle">Preview for</span>
            <Segmented
              label="Target platform"
              value={target}
              onChange={setTarget}
              options={PLATFORMS.map((platform) => ({
                value: platform,
                label: platform === env.hostTarget ? `${PLATFORM_LABEL[platform]} (this computer)` : PLATFORM_LABEL[platform],
              }))}
            />
          </div>

          {groups.map((group) => (
            <section className="section" key={group.category}>
              <h2 className="section-title">{CATEGORY_LABEL[group.category] ?? group.category}</h2>
              <div className="list">
                {group.harnesses.map((harness) => {
                  const available = isAvailable(harness, target);
                  const checked = harnesses.includes(harness.id);
                  return (
                    <button
                      key={harness.id}
                      className="list-row interactive"
                      disabled={!available && !checked}
                      onClick={() => toggle(harness.id)}
                      role="checkbox"
                      aria-checked={checked}
                    >
                      <Checkbox checked={checked} />
                      <Monogram id={harness.id} title={harness.title} size={28} />
                      <div className="row-main">
                        <div className="row-title">{harness.title}</div>
                        <div className="row-subtitle">{harness.description}</div>
                      </div>
                      <div className="row-trailing">
                        {!available ? (
                          <span className="badge red">Not on {PLATFORM_LABEL[target]}</span>
                        ) : harness.desktop_required ? (
                          <span className="badge">App</span>
                        ) : harness.desktop_tool_ids.length > 0 ? (
                          <span className="badge">CLI + optional app</span>
                        ) : null}
                      </div>
                    </button>
                  );
                })}
              </div>
            </section>
          ))}

          <section className="section">
            <div className="section-heading-row">
              <h2 className="section-title">MCP servers</h2>
              {mcpPicks && (
                <button className="link-button" onClick={() => setMcpPicks(null)}>
                  Use recommended
                </button>
              )}
            </div>
            <p className="section-note">
              {takesMcp
                ? "mise installs each one and adds it to every agent in this setup. Your own servers in those agents stay as they are."
                : harnesses.length
                  ? "None of the agents you picked take MCP servers from powermise."
                  : "Pick an agent first; its recommended servers are checked for you."}
            </p>
            <div className="list">
              {catalog.mcpServers.map((server) => {
                const checked = mcpSelected.includes(server.id);
                return (
                  <button
                    key={server.id}
                    className="list-row interactive"
                    disabled={!takesMcp && !checked}
                    onClick={() => toggleMcp(server.id)}
                    role="checkbox"
                    aria-checked={checked}
                  >
                    <Checkbox checked={checked} />
                    <Monogram id={server.id} title={server.title} size={28} />
                    <div className="row-main">
                      <div className="row-title">{server.title}</div>
                      <div className="row-subtitle">{server.description}</div>
                    </div>
                    <div className="row-trailing">
                      {recommendedMcp.includes(server.id) && <span className="badge">Recommended</span>}
                      {server.url && <span className="badge">Remote</span>}
                    </div>
                  </button>
                );
              })}
            </div>
          </section>

          <section className="section">
            <h2 className="section-title">Options</h2>
            <div className="option-row">
              <div className="row-main">
                <div className="row-title">Include desktop apps</div>
                <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                  Also install companion apps, like Claude for Desktop, where one is available.
                  {!hasOptionalDesktop && " None of your harnesses have one."}
                </div>
              </div>
              <Switch
                label="Include desktop apps"
                checked={includeDesktopApps}
                onChange={setIncludeDesktopApps}
              />
            </div>
          </section>
        </div>
      </div>

      <aside className="editor-side" aria-label="What this setup does">
        <div className="editor-side-scroll">
          <div className="stats">
            <div className="stat">
              <div className="value">{cliTools.length}</div>
              <div className="label">Tools</div>
            </div>
            <div className="stat">
              <div className="value">{resolved?.desktopAppIds.length ?? 0}</div>
              <div className="label">Apps</div>
            </div>
            <div className="stat">
              <div className="value">{resolved?.mcpServerIds.length ?? 0}</div>
              <div className="label">MCP servers</div>
            </div>
          </div>

          {harnesses.length === 0 && (
            <div className="callout info">
              <Info size={14} />
              <span>Choose one or more harnesses to see what gets installed.</span>
            </div>
          )}

          {previewError && harnesses.length > 0 && (
            <div className="callout error selectable">
              <AlertTriangle size={14} />
              <span>{previewError}</span>
            </div>
          )}

          {resolved && harnesses.length > 0 && (
            <>
              <div className="side-heading">Installs</div>
              <div className="side-list">
                {cliTools.map((id) => (
                  <div className="side-item" key={id}>
                    <span className="name">{titleFor(catalog, id)}</span>
                    <ProviderBadge provider={installs.get(id)} />
                  </div>
                ))}
              </div>

              {resolved.desktopAppIds.length > 0 && (
                <>
                  <div className="side-heading">Desktop apps</div>
                  <div className="side-list">
                    {resolved.desktopAppIds.map((id) => (
                      <div className="side-item" key={id}>
                        <span className="name">{titleFor(catalog, id)}</span>
                        <ProviderBadge provider={installs.get(id)} />
                      </div>
                    ))}
                  </div>
                </>
              )}

              {resolved.unavailableDesktopIds.length > 0 && (
                <>
                  <div className="side-heading">Not available on {PLATFORM_LABEL[target]}</div>
                  <div className="side-list">
                    {resolved.unavailableDesktopIds.map((id) => (
                      <div className="side-item" key={id}>
                        <span className="name">{titleFor(catalog, id)}</span>
                        <span className="badge">Skipped</span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              {preview && preview.mcp.length > 0 && (
                <>
                  <div className="side-heading">MCP servers</div>
                  <div className="side-list">
                    {preview.mcp.map((entry) => (
                      <div className="side-item" key={entry.client} title={entry.target}>
                        <span className="name">
                          {entry.title}
                          <span className="row-subtitle"> · {entry.servers.map((id) => titleFor(catalog, id)).join(", ")}</span>
                        </span>
                        <span className="badge">Config</span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              {preview && preview.skipped.length > 0 && (
                <>
                  <div className="side-heading">Left out</div>
                  <div className="side-list">
                    {preview.skipped.map((skip) => (
                      <div className="side-item" key={`${skip.id}-${skip.reason}`} title={skip.reason}>
                        <span className="name">{titleFor(catalog, skip.id)}</span>
                        <span className="badge">Skipped</span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              {manual.length > 0 && (
                <>
                  <div className="side-heading">Set up yourself</div>
                  <div className="side-list">
                    {manual.map((id) => (
                      <div className="side-item" key={id}>
                        <span className="name">{titleFor(catalog, id)}</span>
                        <span className="badge orange">Manual</span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              {preview && (
                <>
                  <div className="side-heading">Writes</div>
                  <div className="side-item">
                    <span className="name mono selectable" title={preview.fragmentPath}>
                      {preview.fragmentPath.split(/[\\/]/).slice(-2).join("/")}
                    </span>
                    <span className="badge">
                      {preview.change.kind === "create"
                        ? "New file"
                        : preview.change.kind === "update"
                          ? "Updated"
                          : "No changes"}
                    </span>
                  </div>
                </>
              )}
            </>
          )}
        </div>

        <div className="editor-side-foot">
          {saveError && (
            <div className="callout error selectable">
              <AlertTriangle size={14} />
              <span>{saveError}</span>
            </div>
          )}
          {target !== env.hostTarget && (
            <div className="field-hint">
              Previewing for {PLATFORM_LABEL[target]}. Save it here, then apply it on that computer.
            </div>
          )}
          {!env.misePath && <div className="field-hint error">Install mise to apply setups.</div>}
          {env.misePath && miseTooOld && (
            <div className="field-hint error">AI Setups need mise {mise.info?.recommended} or newer. Update mise to apply.</div>
          )}
          <div className="actions">
            <button className="button large" disabled={!canSave || saving} onClick={() => save(false)}>
              Save
            </button>
            <button
              className="button primary large"
              disabled={!canApply || saving}
              onClick={() => save(true)}
            >
              Save & Apply…
            </button>
          </div>
        </div>
      </aside>

      <ApplySheet
        hostTarget={env.hostTarget}
        open={applyOpen}
        name={name}
        target={target}
        preview={preview}
        catalog={catalog}
        onClose={() => {
          setApplyOpen(false);
          navigate({ kind: "setup", name });
        }}
        onApply={() => {
          void refreshSelections();
          run({
            action: { kind: "applySetup", name },
            title: `Applying ${name}`,
            description: "",
            verb: "Apply",
            autoStart: true,
          });
        }}
      />
    </div>
  );
}
