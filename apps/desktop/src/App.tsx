import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { MotionConfig, motion } from "motion/react";
import {
  AlertTriangle,
  Blocks,
  ChevronLeft,
  Compass,
  Cpu,
  Folder,
  HeartPulse,
  History,
  House,
  Layers,
  Plus,
  Search,
  MonitorCog,
  SlidersHorizontal,
} from "lucide-react";
import {
  api,
  inTauri,
  onMiseFinished,
  PLATFORM_LABEL,
  type Doctor,
  type EnvironmentInfo,
  type MiseInfo,
  type ProjectEntry,
  type OutdatedReport,
  type RegistryEntry,
  type SavedSelection,
  type SetupFile,
  type SystemCheck,
  type ToolInventory,
} from "./api";
import { indexCatalog, type CatalogIndex } from "./catalog";
import appIcon from "../src-tauri/icons/128x128@2x.png";
import { healthSummary, systemIssues } from "./health";
import { SidebarUpdate, checkForAppUpdate } from "./components/AppUpdate";
import { RunSheet, type RunRequest } from "./components/RunSheet";
import { CommandPalette } from "./components/CommandPalette";
import { PlatformSwitcher, WindowChrome } from "./components/WindowChrome";
import { hasPrimaryModifier, shortcutLabel, usePlatform } from "./platform";
import { HarnessesView } from "./views/HarnessesView";
import { HarnessDetail } from "./views/HarnessDetail";
import { ToolsView } from "./views/ToolsView";
import { PluginsView } from "./views/PluginsView";
import { SkillsView } from "./views/SkillsView";
import { ImportView } from "./views/mise/ImportView";
import { McpServersView } from "./views/McpServersView";
import { SetupEditor } from "./views/SetupEditor";
import { SetupView } from "./views/SetupView";
import { InstalledView } from "./views/mise/InstalledView";
import { DiscoverView } from "./views/mise/DiscoverView";
import { ToolPage } from "./views/mise/ToolPage";
import { UpdatesView } from "./views/mise/UpdatesView";
import { HealthView } from "./views/mise/HealthView";
import { SettingsView } from "./views/mise/SettingsView";
import { MachineView } from "./views/mise/MachineView";
import { MacosSettingsView } from "./views/mise/MacosSettingsView";
import { WelcomeView, hasBeenWelcomed } from "./views/mise/WelcomeView";
import { HomeView } from "./views/HomeView";
import { ActivityView } from "./views/mise/ActivityView";
import { ProjectView } from "./views/mise/ProjectView";

export type Route =
  | { kind: "home" }
  // mise
  | { kind: "installed" }
  | { kind: "discover" }
  | { kind: "tool"; name: string }
  | { kind: "updates" }
  | { kind: "activity" }
  | { kind: "health"; section?: "github" | "imports" }
  | { kind: "settings" }
  | { kind: "machine" }
  | { kind: "macos-settings" }
  | { kind: "welcome" }
  | { kind: "project"; path: string }
  // AI Setups
  | { kind: "harnesses" }
  | { kind: "harness"; id: string }
  | { kind: "tools" }
  | { kind: "mcp" }
  | { kind: "integrations" }
  | { kind: "skills" }
  | { kind: "import" }
  | { kind: "setup"; name: string }
  | { kind: "editor"; name?: string; seed?: string[]; imported?: SetupFile };

/** What mise reports, shared by every mise screen and refreshed after changes. */
export interface MiseData {
  info: MiseInfo | null;
  inventory: ToolInventory | null;
  outdated: OutdatedReport | null;
  registry: RegistryEntry[] | null;
  registryByName: Map<string, RegistryEntry>;
  error: string | null;
  /** `mise doctor`, for Health and its sidebar badge. */
  doctor: Doctor | null;
  doctorError: string | null;
  /** Shell startup lines that turn on another version manager; Health
   *  counts them, so its badge does too. */
  shellConflicts: number;
  /** Developer basics (Command Line Tools, build tools, git, PowerShell's
   *  script policy); the missing ones count toward Health. */
  systemChecks: SystemCheck[] | null;
  reloadDoctor: () => Promise<void>;
  /** Increments whenever a change finishes; screens reload on it. */
  revision: number;
}

export interface AppContext {
  env: EnvironmentInfo;
  catalog: CatalogIndex;
  selections: SavedSelection[];
  projects: ProjectEntry[];
  refreshProjects: () => Promise<ProjectEntry[]>;
  /** Opens the folder picker (or adds `path`) and shows the project. */
  addProject: (path?: string) => Promise<void>;
  mise: MiseData;
  navigate: (route: Route) => void;
  refreshSelections: () => Promise<SavedSelection[]>;
  /** Asks to run a mise change: confirm, stream output, report. */
  run: (request: RunRequest) => void;
}

/** Default UI spring: no overshoot, ~0.35s (apple-design). */
const spring = { type: "spring", bounce: 0, duration: 0.35 } as const;

const TOP_LEVEL: Route["kind"][] = [
  "home",
  "installed",
  "discover",
  "updates",
  "activity",
  "health",
  "settings",
  "machine",
  "welcome",
  "project",
  "harnesses",
  "tools",
  "mcp",
  "integrations",
  "skills",
  "setup",
];

/** What each page's search box filters. */
const SEARCH_PLACEHOLDER: Partial<Record<Route["kind"], string>> = {
  installed: "Search your tools",
  discover: "Search 1,000+ tools",
  settings: "Search settings",
  "macos-settings": "Search macOS settings",
  harnesses: "Search agents",
  tools: "Search agent tools",
  mcp: "Search MCP servers",
  integrations: "Search plugins",
  skills: "Search skills",
};

const SEARCHABLE: Route["kind"][] = ["installed", "discover", "settings", "macos-settings", "harnesses", "tools", "mcp", "integrations", "skills"];

/** Pages that share a row of tabs: switching between them swaps only the
 *  content, without the page transition, so the tabs stay put. */
const TAB_GROUPS: Partial<Record<Route["kind"], string>> = {
  installed: "my-tools",
  updates: "my-tools",
  harnesses: "agents",
  tools: "agents",
  mcp: "agents",
  integrations: "agents",
  skills: "agents",
};

const routeKey = (route: Route) =>
  TAB_GROUPS[route.kind]
    ? `tabs:${TAB_GROUPS[route.kind]}`
    : route.kind === "harness"
    ? `harness:${route.id}`
    : route.kind === "tool" || route.kind === "setup"
      ? `${route.kind}:${route.name}`
      : route.kind === "project"
        ? `project:${route.path}`
      : route.kind === "editor"
        ? `editor:${route.name ?? "new"}`
        : route.kind;

export default function App({ initialRoute }: { initialRoute?: Route }) {
  const [env, setEnv] = useState<EnvironmentInfo | null>(null);
  const [catalog, setCatalog] = useState<CatalogIndex | null>(null);
  const [selections, setSelections] = useState<SavedSelection[]>([]);
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  // First run starts on Welcome; after that (and in the site demo), Home.
  const [history, setHistory] = useState<Route[]>([
    initialRoute ??
      (hasBeenWelcomed() || document.body.classList.contains("embedded") ? { kind: "home" } : { kind: "welcome" }),
  ]);
  const [query, setQuery] = useState("");
  const [runRequest, setRunRequest] = useState<RunRequest | null>(null);

  const [revision, setRevision] = useState(0);
  const [miseInfo, setMiseInfo] = useState<MiseInfo | null>(null);
  const [inventory, setInventory] = useState<ToolInventory | null>(null);
  const [outdated, setOutdated] = useState<OutdatedReport | null>(null);
  const [registry, setRegistry] = useState<RegistryEntry[] | null>(null);
  const [miseError, setMiseError] = useState<string | null>(null);
  const [doctor, setDoctor] = useState<Doctor | null>(null);
  const [doctorError, setDoctorError] = useState<string | null>(null);
  const [shellConflicts, setShellConflicts] = useState(0);
  const [systemChecks, setSystemChecks] = useState<SystemCheck[] | null>(null);

  const route = history[history.length - 1];
  const platform = usePlatform();
  const searchRef = useRef<HTMLInputElement>(null);

  const [paletteOpen, setPaletteOpen] = useState(false);
  // ⌘K / Ctrl+K searches everything; ⌘F / Ctrl+F focuses the page's search.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && hasPrimaryModifier(event, platform)) {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (event.key.toLowerCase() === "f" && hasPrimaryModifier(event, platform)) {
        if (!searchRef.current) return;
        event.preventDefault();
        searchRef.current.focus();
        searchRef.current.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [platform]);

  const refreshSelections = useCallback(async () => {
    const next = await api.selections();
    setSelections(next);
    return next;
  }, []);

  const refreshProjects = useCallback(async () => {
    const next = await api.projects();
    setProjects(next);
    return next;
  }, []);

  useEffect(() => {
    refreshProjects().catch(() => setProjects([]));
    Promise.all([api.environment(), api.catalog(), api.selections()])
      .then(([environment, bundle, saved]) => {
        setEnv(environment);
        setCatalog(indexCatalog(bundle));
        setSelections(saved);
      })
      .catch((reason: Error) => setError(reason.message));
    api.registry().then(setRegistry).catch(() => setRegistry([]));
  }, []);

  const reloadDoctor = useCallback(async () => {
    api
      .shellConflicts()
      .then((conflicts) => setShellConflicts(conflicts.length))
      .catch(() => setShellConflicts(0));
    api
      .systemChecks()
      .then(setSystemChecks)
      .catch(() => setSystemChecks([]));
    try {
      setDoctor(await api.doctor());
      setDoctorError(null);
    } catch (reason) {
      setDoctorError((reason as Error).message);
    }
  }, []);

  // mise state reloads at start and after every finished change.
  useEffect(() => {
    void reloadDoctor();
    api.miseInfo().then(setMiseInfo).catch(() => setMiseInfo(null));
    Promise.all([api.installed(), api.outdated()])
      .then(([tools, report]) => {
        setInventory(tools);
        setOutdated(report);
        setMiseError(null);
      })
      .catch((reason: Error) => setMiseError(reason.message));
  }, [revision, reloadDoctor]);

  useEffect(() => {
    const stop = onMiseFinished(() => {
      setRevision((value) => value + 1);
      // A finished job can be mise's own install, which changes where it is.
      api.environment().then(setEnv).catch(() => undefined);
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  }, []);

  const navigate = useCallback((next: Route) => {
    setQuery("");
    setHistory((stack) => (TOP_LEVEL.includes(next.kind) ? [next] : [...stack, next]));
  }, []);

  // A newer powermise? Checked once, shortly after opening.
  useEffect(() => {
    if (!inTauri) return;
    const timer = setTimeout(() => void checkForAppUpdate(), 3000);
    return () => clearTimeout(timer);
  }, []);

  // The marketing-site demo steers the app from outside (see main.tsx).
  useEffect(() => {
    const onNavigate = (event: Event) => {
      const next = (event as CustomEvent<Route>).detail;
      if (next && typeof next === "object" && "kind" in next) {
        setRunRequest(null);
        navigate(next);
      }
    };
    window.addEventListener("powermise:navigate", onNavigate);
    return () => window.removeEventListener("powermise:navigate", onNavigate);
  }, [navigate]);

  const addProject = useCallback(
    async (path?: string) => {
      try {
        const added = await api.addProject(path);
        await refreshProjects();
        if (added) navigate({ kind: "project", path: added.path });
      } catch (reason) {
        window.alert((reason as Error).message);
      }
    },
    [navigate, refreshProjects],
  );

  // Drop a folder on the window to add it as a project.
  useEffect(() => {
    if (!inTauri) return;
    let stop: (() => void) | undefined;
    import("@tauri-apps/api/webview").then(async ({ getCurrentWebview }) => {
      stop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return;
        for (const path of event.payload.paths) void addProject(path);
      });
    });
    return () => stop?.();
  }, [addProject]);

  const goBack = () => setHistory((stack) => (stack.length > 1 ? stack.slice(0, -1) : stack));

  const mise = useMemo<MiseData>(
    () => ({
      info: miseInfo,
      inventory,
      outdated,
      registry,
      registryByName: new Map((registry ?? []).map((entry) => [entry.short, entry])),
      error: miseError,
      doctor,
      doctorError,
      shellConflicts,
      systemChecks,
      reloadDoctor,
      revision,
    }),
    [miseInfo, inventory, outdated, registry, miseError, doctor, doctorError, shellConflicts, systemChecks, reloadDoctor, revision],
  );

  const context = useMemo<AppContext | null>(
    () =>
      env && catalog
        ? {
            // In a browser (design work, the site demo), the "Preview as"
            // platform stands in for the computer, so platform-only parts
            // (like macOS settings) follow it. In the app it's the real one.
            env: inTauri ? env : { ...env, hostTarget: platform },
            catalog,
            selections,
            projects,
            refreshProjects,
            addProject,
            mise,
            navigate,
            refreshSelections,
            run: setRunRequest,
          }
        : null,
    [env, catalog, selections, projects, refreshProjects, addProject, mise, navigate, refreshSelections, platform],
  );

  if (error) {
    return (
      <div className="empty" style={{ paddingTop: 120 }}>
        <AlertTriangle size={28} color="var(--orange)" />
        <h3>powermise couldn’t start</h3>
        <p className="selectable">{error}</p>
      </div>
    );
  }
  if (!context) return null;

  // The first launch: Welcome fills the window, without the sidebar and
  // toolbar, until it's done or skipped.
  const onboarding = route.kind === "welcome" && !hasBeenWelcomed() && !document.body.classList.contains("embedded");

  return (
    <MotionConfig reducedMotion="user">
      <div className="app-frame">
        <WindowChrome />
        <div className={`window${onboarding ? " onboarding" : ""}`}>
          {!onboarding && <Sidebar route={route} root={history[0]} context={context} />}
          <main className="main">
            {onboarding ? (
              <div className="onboarding-bar" data-tauri-drag-region />
            ) : (
            <header className="toolbar" data-tauri-drag-region>
              {history.length > 1 && (
                <button className="icon-button" onClick={goBack} aria-label="Back">
                  <ChevronLeft size={18} />
                </button>
              )}
              <div className="toolbar-title" data-tauri-drag-region>
                {/* Home carries the brand: the app icon and its name. */}
                {route.kind === "home" && (
                  <img className="toolbar-brand" src={appIcon} alt="" width={20} height={20} data-tauri-drag-region />
                )}
                <span data-tauri-drag-region>{toolbarTitle(route, context)}</span>
              </div>
              <div className="toolbar-spacer" data-tauri-drag-region />
              <button
                className={`icon-button${route.kind === "activity" ? " active" : ""}`}
                onClick={() => navigate({ kind: "activity" })}
                aria-label="History"
                title="History — every command powermise ran"
              >
                <History size={16} />
              </button>
              {/* One search at a time: a page with its own filter shows just
                  that; elsewhere, Search all (⌘K opens it from any page). */}
              {!SEARCHABLE.includes(route.kind) && (
              <button
                className="toolbar-search"
                onClick={() => setPaletteOpen(true)}
                aria-label="Search everything"
                aria-keyshortcuts={platform === "darwin" ? "Meta+K" : "Control+K"}
              >
                <Search size={14} /> Search all <kbd aria-hidden>{shortcutLabel(platform, "k")}</kbd>
              </button>
              )}
              {SEARCHABLE.includes(route.kind) && (
                <label className="search">
                  <Search size={14} />
                  <input
                    ref={searchRef}
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder={SEARCH_PLACEHOLDER[route.kind] ?? "Search"}
                    aria-label="Search"
                    aria-keyshortcuts={platform === "darwin" ? "Meta+F" : "Control+F"}
                  />
                  <kbd aria-hidden>{shortcutLabel(platform, "f")}</kbd>
                </label>
              )}
            </header>
            )}
            {/* Swap pages immediately and ease the new one in; waiting on an
                exit fade makes navigation feel slower than it is. */}
            <motion.div
              key={routeKey(route)}
              style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ ...spring, opacity: { duration: 0.18 } }}
            >
              <RouteView route={route} context={context} query={query} />
            </motion.div>
          </main>
        </div>
      </div>
      <RunSheet request={runRequest} onClose={() => setRunRequest(null)} />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} context={context} setQuery={setQuery} />
      <PlatformSwitcher />
    </MotionConfig>
  );
}

function toolbarTitle(route: Route, context: AppContext): string {
  switch (route.kind) {
    case "home":
      return "powermise";
    case "installed":
      return "My Tools";
    case "discover":
      return "Discover";
    case "tool":
      return route.name;
    case "updates":
      return "My Tools";
    case "activity":
      return "History";
    case "health":
      return "Health";
    case "settings":
      return "Settings";
    case "import":
      return "Import";
    case "machine":
      return "This Computer";
    case "macos-settings":
      return "macOS Settings";
    case "welcome":
      return "Welcome";
    case "project":
      return route.path.split(/[\\/]/).filter(Boolean).pop() ?? route.path;
    case "harnesses":
      return "Agents";
    case "harness":
      return context.catalog.harnessById.get(route.id)?.title ?? route.id;
    case "tools":
    case "mcp":
    case "integrations":
    case "skills":
      return "Agents";
    case "setup":
      return route.name;
    case "editor":
      return route.name ? `Edit ${route.name}` : "New Setup";
  }
}

/** Screens that need mise; without it they show the install step instead. */
const NEEDS_MISE: Route["kind"][] = [
  "installed",
  "discover",
  "tool",
  "updates",
  "health",
  "settings",
  "machine",
  "macos-settings",
  "project",
];

function RouteView({ route, context, query }: { route: Route; context: AppContext; query: string }) {
  if (!context.env.misePath && NEEDS_MISE.includes(route.kind)) return <WelcomeView context={context} />;
  switch (route.kind) {
    case "home":
      return <HomeView context={context} />;
    case "installed":
      return <InstalledView context={context} query={query} />;
    case "discover":
      return <DiscoverView context={context} query={query} />;
    case "tool":
      return <ToolPage context={context} name={route.name} />;
    case "updates":
      return <UpdatesView context={context} />;
    case "activity":
      return <ActivityView context={context} />;
    case "health":
      return <HealthView context={context} section={route.section} />;
    case "settings":
      return <SettingsView context={context} query={query} />;
    case "machine":
      return <MachineView context={context} />;
    case "macos-settings":
      // macOS only; anywhere else, show This Computer instead.
      return context.env.hostTarget === "darwin" ? (
        <MacosSettingsView context={context} query={query} />
      ) : (
        <MachineView context={context} />
      );
    case "welcome":
      return <WelcomeView context={context} />;
    case "project":
      return <ProjectView context={context} path={route.path} />;
    case "harnesses":
      return <HarnessesView context={context} query={query} />;
    case "harness":
      return <HarnessDetail context={context} id={route.id} />;
    case "tools":
      return <ToolsView context={context} query={query} />;
    case "mcp":
      return <McpServersView context={context} query={query} />;
    case "integrations":
      return <PluginsView context={context} query={query} />;
    case "skills":
      return <SkillsView context={context} query={query} />;
    case "import":
      return <ImportView context={context} />;
    case "setup":
      return <SetupView context={context} name={route.name} />;
    case "editor":
      return (
        <SetupEditor
          key={route.imported?.path ?? route.name ?? "new"}
          context={context}
          name={route.name}
          seed={route.seed}
          imported={route.imported}
        />
      );
  }
}

function Sidebar({ route, root, context }: { route: Route; root: Route; context: AppContext }) {
  const { selections, projects, env, mise, navigate, addProject } = context;
  const platform = usePlatform();
  const installedCount = Object.keys(mise.inventory ?? {}).length;
  const updateCount = Object.keys(mise.outdated ?? {}).length;
  const health = healthSummary(mise.info, mise.doctor, mise.shellConflicts, systemIssues(mise.systemChecks));
  // Detail pages keep the section they were opened from highlighted.
  const isActive = (...kinds: Route["kind"][]) => kinds.includes(root.kind);

  const item = (
    kinds: Route["kind"][],
    label: string,
    Icon: typeof Layers,
    trailing: React.ReactNode = null,
  ) => (
    <button
      key={kinds[0]}
      className="sidebar-item"
      aria-current={isActive(...kinds) ? "page" : undefined}
      onClick={() => navigate({ kind: kinds[0] } as Route)}
    >
      <Icon size={16} />
      <span className="label">{label}</span>
      {typeof trailing === "number" ? <span className="count">{trailing}</span> : trailing}
    </button>
  );

  return (
    <nav className="sidebar" data-tauri-drag-region aria-label="Sidebar">
      <div className="sidebar-scroll">
        <div className="sidebar-section">{item(["home"], "Home", House)}</div>

        <div className="sidebar-section">
          <div className="sidebar-heading">Tools</div>
          {item(
            ["installed", "updates"],
            "My Tools",
            Layers,
            updateCount ? (
              <span className="sidebar-badge" title={`${updateCount} ${updateCount === 1 ? "update" : "updates"}`}>
                {updateCount}
              </span>
            ) : (
              installedCount || null
            ),
          )}
          {item(["discover"], "Discover", Compass)}
        </div>

        <div className="sidebar-section">
          <div className="sidebar-heading">
            Projects
            <button onClick={() => addProject()} aria-label="Add project folder" title="Add Project Folder">
              <Plus size={14} />
            </button>
          </div>
          {projects.length === 0 && (
            <button className="sidebar-item" onClick={() => addProject()}>
              <Plus size={16} />
              <span className="label">Add a project folder</span>
            </button>
          )}
          {projects.map((project) => (
            <button
              key={project.path}
              className="sidebar-item"
              aria-current={route.kind === "project" && route.path === project.path ? "page" : undefined}
              onClick={() => navigate({ kind: "project", path: project.path })}
              title={project.exists ? project.path : `${project.path} — not found`}
            >
              <Folder size={16} />
              <span className="label">{project.name}</span>
              {!project.exists && <span className="status-dot attention" />}
            </button>
          ))}
        </div>

        <div className="sidebar-section">
          <div className="sidebar-heading">
            AI Agents
            <button onClick={() => navigate({ kind: "editor" })} aria-label="New setup" title="New Setup">
              <Plus size={14} />
            </button>
          </div>
          {item(["harnesses", "harness", "tools", "mcp", "integrations", "skills"], "Browse Agents", Cpu)}
          {selections.map((selection) => {
            const active =
              (route.kind === "setup" && route.name === selection.name) ||
              (route.kind === "editor" && route.name === selection.name);
            return (
              <button
                key={selection.name}
                className="sidebar-item"
                aria-current={active ? "page" : undefined}
                onClick={() => navigate({ kind: "setup", name: selection.name })}
              >
                <Blocks size={16} />
                <span className="label">{selection.name}</span>
                <span
                  className={`status-dot${selection.applied ? " applied" : ""}`}
                  title={selection.applied ? "In your mise config" : "Not applied yet"}
                />
              </button>
            );
          })}
          {route.kind === "editor" && !route.name && (
            <button className="sidebar-item" aria-current="page">
              <Plus size={16} />
              <span className="label">New Setup</span>
            </button>
          )}
        </div>
      </div>
      <div className="sidebar-section sidebar-bottom">
        {item(["machine", "macos-settings", "import"], "This Computer", MonitorCog)}
        {item(
          ["health"],
          "Health",
          HeartPulse,
          health.severity ? (
            // A dot, not a count: the Health page lists each issue.
            <span
              className={`health-dot ${health.severity}`}
              role="img"
              aria-label={`${health.count} ${health.count === 1 ? "thing needs" : "things need"} attention`}
              title={`${health.count} ${health.count === 1 ? "thing needs" : "things need"} attention`}
            />
          ) : null,
        )}
        {item(["settings"], "Settings", SlidersHorizontal)}
      </div>
      <div className="sidebar-footer">
        {mise.info?.version ? (
          <div className={`row${mise.info.supported ? "" : " warn"}`} title={mise.info.path ?? undefined}>
            {!mise.info.supported && <AlertTriangle size={13} />}
            mise {mise.info.version.split(" ")[0]}
          </div>
        ) : (
          <div className="row warn">
            <AlertTriangle size={13} /> mise not found
          </div>
        )}
        <div className="row">{PLATFORM_LABEL[inTauri ? env.hostTarget : platform]}</div>
        <SidebarUpdate />
      </div>
    </nav>
  );
}
