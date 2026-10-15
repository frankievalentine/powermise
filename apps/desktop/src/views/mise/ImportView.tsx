import { useMemo, useState } from "react";
import { AlertTriangle, FileUp, Loader2, ScanSearch } from "lucide-react";
import { api, type Brewfile, type ImportFile, type ImportSource, type RegistryEntry } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { prettyPath } from "../../mise";
import {
  BREW_RENAMES,
  RUNTIMES,
  SOURCE_LABEL,
  importTodo,
  packageTool,
  pendingImports,
  programTools,
  toolLabel,
} from "../../components/machine";
import { Sheet } from "../../components/Sheet";
import { Checkbox, CommandBlock, Section } from "../../components/ui";

/** What an import row becomes in mise. */
type Becomes =
  | { kind: "versions"; source: ImportSource; tool: string; links: number; defaultVersion: string | null }
  | { kind: "tool"; tool: string; version: string }
  | { kind: "package"; spec: string };

interface ImportItem {
  key: string;
  group: string;
  title: string;
  detail: string;
  becomes: Becomes;
  /** Already on this computer: importing only records it in mise's config. */
  installed: boolean;
  /** Checked to start. */
  suggested: boolean;
}

const GROUPS = [
  { id: "versions", title: "Version managers", note: "Versions mise can use where they are, without downloading them again." },
  { id: "packages", title: "Command-line packages", note: "From npm -g, pipx, uv, cargo, or default-package files. As mise tools, they stay put when you switch Node or Python versions." },
  { id: "homebrew", title: "Homebrew", note: "Formulas and apps mise can install and keep track of itself — no Homebrew needed on your next Mac. What Homebrew already installed stays as it is." },
  { id: "appstore", title: "Mac App Store", note: "Installed with mas, signed in to your Apple account. Paid apps must already be yours." },
  { id: "other", title: "Installed another way", note: "From an install script, go install, and so on — mise has them too." },
] as const;

/** Homebrew formulas whose same-named mise tool is a different project. */
const SAME_NAME_DIFFERENT_TOOL = new Set(["coreutils"]);

const BECOMES_LABEL = (becomes: Becomes) =>
  becomes.kind === "versions"
    ? `${becomes.links ? `Link ${becomes.links}` : "Linked"}${becomes.defaultVersion ? ` · default ${becomes.defaultVersion}` : ""}`
    : becomes.kind === "tool"
      ? `mise tool · ${becomes.tool}`
      : becomes.spec.startsWith("brew-cask:font-")
        ? `Font · ${becomes.spec}`
        : becomes.spec.startsWith("brew-cask:")
        ? `Mac app · ${becomes.spec}`
        : becomes.spec.startsWith("mas:")
          ? `App Store · ${becomes.spec}`
          : `Homebrew package · ${becomes.spec}`;

/** Bring over what you already have: one scan of every source, a review
 *  with mise's own dry run, and one file powermise owns
 *  (`conf.d/powermise-imported.toml`) for what you pick. */
export function ImportView({ context }: { context: AppContext }) {
  const { mise, run, navigate } = context;
  const imports = useAsync(() => api.detectImports(), [mise.revision]);
  const packages = useAsync(() => api.globalPackages(), [mise.revision]);
  const programs = useAsync(() => api.installedPrograms(), [mise.revision]);
  const scan = useAsync(() => api.importScan(), [mise.revision]);
  const conflicts = useAsync(() => api.shellConflicts(), [mise.revision]);
  const [extraBrewfiles, setExtraBrewfiles] = useState<Brewfile[]>([]);
  const [picked, setPicked] = useState<Map<string, boolean>>(new Map());
  const [defaults, setDefaults] = useState<Map<string, boolean>>(new Map());
  const [review, setReview] = useState<ImportFile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const loading = !imports.data || !packages.data || !programs.data || !scan.data;

  const items = useMemo<ImportItem[]>(() => {
    if (loading) return [];
    const out: ImportItem[] = [];
    const seen = new Set<string>();
    const imported = new Set(scan.data?.importedPackages ?? []);
    const add = (item: ImportItem) => {
      const id = item.becomes.kind === "tool" ? `tool:${item.becomes.tool}` : item.key;
      if (seen.has(id) || (item.becomes.kind === "package" && imported.has(item.becomes.spec))) return;
      seen.add(id);
      out.push(item);
    };
    const inventory = mise.inventory ?? {};
    const registry = new Map<string, RegistryEntry>();
    for (const entry of mise.registry ?? []) {
      registry.set(entry.short, entry);
      for (const alias of entry.aliases) if (!registry.has(alias)) registry.set(alias, entry);
    }

    // 1. Version managers.
    for (const entry of pendingImports(context, imports.data ?? [])) {
      const { missing, offerDefault } = importTodo(context, entry);
      add({
        key: `versions:${entry.source}:${entry.tool}`,
        group: "versions",
        title: `${toolLabel(entry.tool)} from ${SOURCE_LABEL[entry.source]}`,
        detail: entry.versions.map((version) => `${version.version}${version.version === entry.defaultVersion ? " (default)" : ""}`).join(", "),
        becomes: {
          kind: "versions",
          source: entry.source,
          tool: entry.tool,
          links: missing.length,
          defaultVersion: offerDefault ? entry.defaultVersion : null,
        },
        installed: true,
        suggested: true,
      });
    }

    // 2. Packages: npm -g, pipx, uv, cargo, and default-package files.
    for (const pkg of [...(packages.data ?? []), ...(scan.data?.defaultPackages ?? []).map((pkg) => ({ ...pkg, foundIn: prettyPath(pkg.file) }))]) {
      const { tool, version } = packageTool(context, pkg);
      if (inventory[tool]) continue;
      add({
        key: `package:${pkg.backend}:${pkg.name}`,
        group: "packages",
        title: pkg.name,
        detail: `From ${pkg.foundIn}${pkg.version ? ` · ${pkg.version}` : ""}`,
        becomes: { kind: "tool", tool, version },
        installed: false,
        suggested: true,
      });
    }

    // 3. Homebrew: what's installed, and every Brewfile.
    const brewfiles = [...(scan.data?.brewfiles ?? []), ...extraBrewfiles];
    const installedFormulas = new Map((scan.data?.homebrew.formulas ?? []).map((item) => [item.name, item]));
    const installedCasks = new Map((scan.data?.homebrew.casks ?? []).map((item) => [item.name, item]));
    // A Brewfile names a tapped formula without its tap; use the installed one's full name.
    const qualify = (name: string, installed: Map<string, unknown>) =>
      name.includes("/") ? name : ([...installed.keys()].find((full) => full.split("/").pop() === name) ?? name);
    const listed = new Set(brewfiles.flatMap((file) => [...file.formulas.map((name) => qualify(name, installedFormulas)), ...file.casks.map((name) => `cask:${qualify(name, installedCasks)}`)]));
    const formulaNames = [...new Set([...installedFormulas.keys(), ...brewfiles.flatMap((file) => file.formulas.map((name) => qualify(name, installedFormulas)))])];
    for (const name of formulaNames) {
      const short = name.split("/").pop()!.replace(/@[\d.]+$/, "");
      if (RUNTIMES.has(short) || /^python@|^node@|^ruby@|^go@/.test(name.split("/").pop()!)) continue;
      // Same name, different project: mise's `coreutils` is a Rust rewrite.
      const entry = SAME_NAME_DIFFERENT_TOOL.has(short) ? undefined : registry.get(BREW_RENAMES[short] ?? short);
      const installed = installedFormulas.has(name);
      const inFile = listed.has(name);
      if (entry) {
        if (inventory[entry.short]) continue;
        add({
          key: `brew:${name}`,
          group: "homebrew",
          title: name,
          detail: `${inFile ? "In your Brewfile" : "Installed with Homebrew"} · mise has it as ${entry.short}`,
          becomes: { kind: "tool", tool: entry.short, version: "latest" },
          installed: false,
          suggested: inFile,
        });
      } else {
        add({
          key: `brew:${name}`,
          group: "homebrew",
          title: name,
          detail: inFile ? (installed ? "In your Brewfile · installed" : "In your Brewfile") : "Installed with Homebrew",
          becomes: { kind: "package", spec: `brew:${name}` },
          installed,
          suggested: inFile,
        });
      }
    }
    const caskNames = [...new Set([...installedCasks.keys(), ...brewfiles.flatMap((file) => file.casks.map((name) => qualify(name, installedCasks)))])];
    for (const name of caskNames) {
      const installed = installedCasks.has(name);
      const inFile = listed.has(`cask:${name}`);
      add({
        key: `cask:${name}`,
        group: "homebrew",
        title: name,
        detail: `${name.startsWith("font-") ? "Font" : "App"} · ${inFile ? (installed ? "In your Brewfile · installed" : "In your Brewfile") : "Installed with Homebrew"}`,
        becomes: { kind: "package", spec: `brew-cask:${name}` },
        installed,
        suggested: inFile,
      });
    }

    // 4. Mac App Store.
    const masInstalled = new Map((scan.data?.masApps ?? []).map((app) => [app.id, app]));
    const masListed = new Map(brewfiles.flatMap((file) => file.mas.map(([name, id]) => [id, name] as const)));
    for (const id of new Set([...masInstalled.keys(), ...masListed.keys()])) {
      const app = masInstalled.get(id);
      add({
        key: `mas:${id}`,
        group: "appstore",
        title: app?.name ?? masListed.get(id) ?? id,
        detail: masListed.has(id) ? (app ? "In your Brewfile · installed" : "In your Brewfile") : "Installed from the App Store",
        becomes: { kind: "package", spec: `mas:${id}` },
        installed: !!app,
        suggested: masListed.has(id),
      });
    }

    // 5. Installed another way (Homebrew's are above).
    const packageNames = new Set((packages.data ?? []).map((pkg) => pkg.name.replace(/^@[^/]+\//, "")));
    const others = (programs.data ?? []).filter((program) => !program.package);
    for (const row of programTools(context, others, packageNames)) {
      add({
        key: `program:${row.tool}`,
        group: "other",
        title: row.tool,
        detail: `From ${row.foundIn}${row.commands.length > 1 || row.commands[0] !== row.tool ? ` · ${row.commands.join(", ")}` : ""}`,
        becomes: { kind: "tool", tool: row.tool, version: "latest" },
        installed: false,
        suggested: false,
      });
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, imports.data, packages.data, programs.data, scan.data, extraBrewfiles, mise.inventory, mise.registry]);

  const isPicked = (item: ImportItem) => picked.get(item.key) ?? item.suggested;
  const makesDefault = (item: ImportItem) =>
    item.becomes.kind === "versions" && !!item.becomes.defaultVersion && (defaults.get(item.key) ?? true);
  const toggle = (key: string, value: boolean) =>
    setPicked((current) => new Map(current).set(key, value));
  const chosen = items.filter(isPicked);

  // What a run does, from the picks.
  const plan = useMemo(() => {
    const versions: [ImportSource, string, boolean][] = [];
    const tools: [string, string][] = [];
    const specs: string[] = [];
    let links = 0;
    for (const item of chosen) {
      const becomes = item.becomes;
      if (becomes.kind === "versions") {
        versions.push([becomes.source, becomes.tool, makesDefault(item)]);
        links += becomes.links;
      } else if (becomes.kind === "tool") tools.push([becomes.tool, becomes.version]);
      else specs.push(becomes.spec);
    }
    const already = chosen.filter((item) => item.becomes.kind === "package" && item.installed).length;
    const install = tools.length + chosen.filter((item) => item.becomes.kind === "package" && !item.installed).length;
    const defaultChanges = chosen.filter(makesDefault).length;
    return { versions, tools, specs, links, already, install, defaultChanges };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chosen, defaults]);

  const addBrewfile = async () => {
    setError(null);
    try {
      const file = await api.readBrewfile();
      if (file) {
        setExtraBrewfiles((current) => [...current.filter((existing) => existing.path !== file.path), file]);
      }
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const dryRun = () =>
    run({
      action: { kind: "importDryRun", tools: plan.tools, packages: plan.specs },
      title: "Dry run",
      description: `mise checks what it would install for your picks${plan.links ? ` (linking ${plan.links} ${plan.links === 1 ? "version" : "versions"} from other managers isn't part of the dry run)` : ""}. Nothing is written or installed.`,
      verb: "Run Dry Run",
    });

  const openReview = async () => {
    setError(null);
    try {
      setReview(await api.importFilePreview(plan.tools, plan.specs));
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const apply = async () => {
    setError(null);
    try {
      if (plan.tools.length || plan.specs.length) await api.writeImport(plan.tools, plan.specs);
      setReview(null);
      run({
        action: { kind: "applyImport", versions: plan.versions, tools: plan.tools, packages: plan.specs },
        title: "Importing",
        description: "",
        verb: "Import",
        autoStart: true,
      });
      setPicked(new Map());
    } catch (reason) {
      setError((reason as Error).message);
    }
  };

  const brewfiles = [...(scan.data?.brewfiles ?? []), ...extraBrewfiles];
  const sources = [
    ...new Set((imports.data ?? []).map((entry) => SOURCE_LABEL[entry.source])),
    ...(scan.data && scan.data.homebrew.formulas.length + scan.data.homebrew.casks.length
      ? [`Homebrew (${scan.data.homebrew.formulas.length} formulas, ${scan.data.homebrew.casks.length} casks)`]
      : []),
    ...brewfiles.map((file) => prettyPath(file.path, context.env.home)),
    ...(scan.data?.masApps.length ? [`App Store (${scan.data.masApps.length})`] : []),
  ];

  return (
    <div className="content">
      <div className="page import-page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Import</h1>
            <p className="page-subtitle">
              Bring over what you already have — versions from other version managers, Homebrew and a Brewfile, App
              Store apps, and command-line packages. Pick what to import, try a dry run, and powermise adds it to one
              file of its own in your mise config.
            </p>
          </div>
          <div className="detail-actions">
            <button className="button" onClick={() => void addBrewfile()}>
              <FileUp size={14} /> Add a Brewfile…
            </button>
          </div>
        </div>

        {loading && (
          <p className="field-hint">
            <Loader2 size={12} className="spin" /> Looking around this computer…
          </p>
        )}
        {error && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{error}</span>
          </div>
        )}
        {!loading && sources.length > 0 && (
          <p className="section-note">
            <ScanSearch size={12} aria-hidden /> Found: {sources.join(" · ")}
          </p>
        )}
        {conflicts.data && conflicts.data.length > 0 && (
          <div className="callout warn">
            <AlertTriangle size={14} />
            <span>
              {[...new Set(conflicts.data.map((conflict) => conflict.manager))].join(", ")} still{" "}
              {conflicts.data.length === 1 ? "turns" : "turn"} on in your shell startup files.{" "}
              <button className="link-button" onClick={() => navigate({ kind: "health" })}>
                See which lines in Health
              </button>
            </span>
          </div>
        )}
        {!loading && items.length === 0 && (
          <div className="empty">
            <h3>Nothing to bring over</h3>
            <p>mise already has everything powermise found. Have a Brewfile somewhere else? Add it above.</p>
          </div>
        )}

        {GROUPS.map((group) => {
          const rows = items.filter((item) => item.group === group.id);
          if (!rows.length) return null;
          const all = rows.every(isPicked);
          return (
            <Section key={group.id} title={`${group.title} (${rows.length})`} note={group.note}>
              <div className="detail-actions" style={{ marginBottom: 8 }}>
                <button
                  className="link-button"
                  onClick={() => setPicked((current) => {
                    const next = new Map(current);
                    for (const row of rows) next.set(row.key, !all);
                    return next;
                  })}
                >
                  {all ? "Select none" : "Select all"}
                </button>
              </div>
              <div className="list">
                {rows.map((item) => (
                  <div className="list-row import-row" key={item.key}>
                    <button
                      className="import-row-main"
                      onClick={() => toggle(item.key, !isPicked(item))}
                      role="checkbox"
                      aria-checked={isPicked(item)}
                    >
                      <Checkbox checked={isPicked(item)} />
                      <div className="row-main">
                        <div className="row-title">{item.title}</div>
                        <div className="row-subtitle">{item.detail}</div>
                      </div>
                    </button>
                    <div className="row-trailing">
                      {item.becomes.kind === "versions" && item.becomes.defaultVersion && (
                        <label className="import-default">
                          <input
                            type="checkbox"
                            checked={defaults.get(item.key) ?? true}
                            onChange={(event) => setDefaults((current) => new Map(current).set(item.key, event.target.checked))}
                          />
                          Use {item.becomes.defaultVersion}
                        </label>
                      )}
                      <span className="badge mono" title={BECOMES_LABEL(item.becomes)}>
                        {BECOMES_LABEL(item.becomes)}
                      </span>
                      {item.becomes.kind !== "versions" && (
                        <span className={`badge${item.installed ? " green" : ""}`}>
                          {item.installed ? "Already installed" : "Will install"}
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </Section>
          );
        })}

        {chosen.length > 0 && (
          <div className="import-bar">
            <span>
              <strong>{chosen.length} selected</strong>
              {plan.already ? ` · ${plan.already} already installed` : ""}
              {plan.install ? ` · ${plan.install} to install` : ""}
              {plan.links ? ` · ${plan.links} ${plan.links === 1 ? "version" : "versions"} to link` : ""}
              {plan.defaultChanges ? ` · ${plan.defaultChanges} default ${plan.defaultChanges === 1 ? "change" : "changes"}` : ""}
            </span>
            <div className="detail-actions">
              <button className="button" disabled={!plan.tools.length && !plan.specs.length} onClick={dryRun}>
                Dry Run
              </button>
              <button className="button primary" onClick={() => void openReview()}>
                Review & Import…
              </button>
            </div>
          </div>
        )}
      </div>

      <Sheet open={!!review} onDismiss={() => setReview(null)} labelledBy="import-review-title">
        <div className="sheet-head">
          <h2 id="import-review-title">Import {chosen.length} {chosen.length === 1 ? "thing" : "things"}?</h2>
          <p>
            {plan.tools.length || plan.specs.length
              ? `powermise adds them to ${review ? prettyPath(review.path, context.env.home) : "its import file"} — your own config isn’t touched — then mise installs what isn’t here yet. Anything already installed stays as it is.`
              : "mise links the versions where they are."}
            {plan.defaultChanges ? " Version-manager defaults you kept checked become your mise defaults (mise use -g)." : ""}
          </p>
        </div>
        <div className="sheet-body">
          {review && (plan.tools.length > 0 || plan.specs.length > 0) && (
            <>
              <div className="side-heading" style={{ marginTop: 0 }}>
                {review.change.kind === "create" ? "New file" : "Updated file"}
              </div>
              <pre className="code-view selectable">{review.contents}</pre>
            </>
          )}
          <div className="side-heading">Then mise runs</div>
          <CommandBlock
            command={[
              ...(plan.links ? [`mise sync / mise link (${plan.links})`] : []),
              ...(plan.defaultChanges ? ["mise use -g …"] : []),
              ...(plan.tools.length ? [`mise install ${plan.tools.map(([tool, version]) => `${tool}@${version}`).join(" ")}`] : []),
              ...(plan.specs.length ? [`mise bootstrap packages apply --yes ${plan.specs.join(" ")}`] : []),
            ].join(" && ")}
          />
        </div>
        <div className="sheet-foot">
          <button className="button large" onClick={() => setReview(null)}>
            Cancel
          </button>
          <button className="button primary large" onClick={() => void apply()}>
            Import
          </button>
        </div>
      </Sheet>
    </div>
  );
}
