import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { MotionConfig, motion } from "motion/react";
import {
  Activity,
  AlertTriangle,
  ArrowUpCircle,
  Blocks,
  ChevronLeft,
  Compass,
  Cpu,
  Folder,
  HeartPulse,
  Layers,
  Package,
  Plug,
  Plus,
  Search,
  Server,
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
  type ToolInventory,
} from "./api";
import { indexCatalog, type CatalogIndex } from "./catalog";
import { healthSummary } from "./health";
import { RunSheet, type RunRequest } from "./components/RunSheet";
import { PlatformSwitcher, WindowChrome } from "./components/WindowChrome";
import { hasPrimaryModifier, shortcutLabel, usePlatform } from "./platform";
import { HarnessesView } from "./views/HarnessesView";
import { HarnessDetail } from "./views/HarnessDetail";
import { ToolsView } from "./views/ToolsView";
import { ConfigView } from "./views/ConfigView";
import { SetupEditor } from "./views/SetupEditor";
import { SetupView } from "./views/SetupView";
import { InstalledView } from "./views/mise/InstalledView";
import { DiscoverView } from "./views/mise/DiscoverView";
import { ToolPage } from "./views/mise/ToolPage";
import { UpdatesView } from "./views/mise/UpdatesView";
import { HealthView } from "./views/mise/HealthView";
import { SettingsView } from "./views/mise/SettingsView";
import { WelcomeView, hasBeenWelcomed } from "./views/mise/WelcomeView";
import { ActivityView } from "./views/mise/ActivityView";
import { ProjectView } from "./views/mise/ProjectView";

export type Route =
  // mise
  | { kind: "installed" }
  | { kind: "discover" }
  | { kind: "tool"; name: string }
  | { kind: "updates" }
  | { kind: "activity" }
  | { kind: "health" }
  | { kind: "settings" }
  | { kind: "welcome" }
  | { kind: "project"; path: string }
  // AI Setups
  | { kind: "harnesses" }
  | { kind: "harness"; id: string }
  | { kind: "tools" }
  | { kind: "mcp" }
  | { kind: "integrations" }
  | { kind: "setup"; name: string }
  | { kind: "editor"; name?: string; seed?: string[] };

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
  "installed",
  "discover",
  "updates",
  "activity",
  "health",
  "settings",
  "welcome",
  "project",
  "harnesses",
  "tools",
  "mcp",
  "integrations",
  "setup",
];

const SEARCHABLE: Route["kind"][] = ["installed", "discover", "settings", "harnesses", "tools", "mcp", "integrations"];

const routeKey = (route: Route) =>
  route.kind === "harness"
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
  // First run starts on Welcome; the site demo (embedded) never does.
  const [history, setHistory] = useState<Route[]>([
    initialRoute ??
      (hasBeenWelcomed() || document.body.classList.contains("embedded")
        ? { kind: "installed" }
        : { kind: "welcome" }),
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

  const route = history[history.length - 1];
  const platform = usePlatform();
  const searchRef = useRef<HTMLInputElement>(null);

  // ⌘F / Ctrl+F focuses search wherever a screen has one.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
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
      reloadDoctor,
      revision,
    }),
    [miseInfo, inventory, outdated, registry, miseError, doctor, doctorError, reloadDoctor, revision],
  );

  const context = useMemo<AppContext | null>(
    () =>
      env && catalog
        ? {
            env,
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
    [env, catalog, selections, projects, refreshProjects, addProject, mise, navigate, refreshSelections],
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

  return (
    <MotionConfig reducedMotion="user">
      <div className="app-frame">
        <WindowChrome />
        <div className="window">
          <Sidebar route={route} root={history[0]} context={context} />
          <main className="main">
            <header className="toolbar" data-tauri-drag-region>
              {history.length > 1 && (
                <button className="icon-button" onClick={goBack} aria-label="Back">
                  <ChevronLeft size={18} />
                </button>
              )}
              <div className="toolbar-title" data-tauri-drag-region>
                <span data-tauri-drag-region>{toolbarTitle(route, context)}</span>
              </div>
              <div className="toolbar-spacer" data-tauri-drag-region />
              {SEARCHABLE.includes(route.kind) && (
                <label className="search">
                  <Search size={14} />
                  <input
                    ref={searchRef}
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder={route.kind === "discover" ? "Search 1,000+ tools" : "Search"}
                    aria-label="Search"
                    aria-keyshortcuts={platform === "darwin" ? "Meta+F" : "Control+F"}
                  />
                  <kbd aria-hidden>{shortcutLabel(platform, "f")}</kbd>
                </label>
              )}
            </header>
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
      <PlatformSwitcher />
    </MotionConfig>
  );
}

function toolbarTitle(route: Route, context: AppContext): string {
  switch (route.kind) {
    case "installed":
      return "Installed";
    case "discover":
      return "Discover";
    case "tool":
      return route.name;
    case "updates":
      return "Updates";
    case "activity":
      return "Activity";
    case "health":
      return "Health";
    case "settings":
      return "Settings";
    case "welcome":
      return "Welcome";
    case "project":
      return route.path.split(/[\\/]/).filter(Boolean).pop() ?? route.path;
    case "harnesses":
      return "Harnesses";
    case "harness":
      return context.catalog.harnessById.get(route.id)?.title ?? route.id;
    case "tools":
      return "Agent Tools";
    case "mcp":
      return "MCP Servers";
    case "integrations":
      return "Integrations";
    case "setup":
      return route.name;
    case "editor":
      return route.name ? `Edit ${route.name}` : "New Setup";
  }
}

/** Screens that need mise; without it they show the install step instead. */
const NEEDS_MISE: Route["kind"][] = ["installed", "discover", "tool", "updates", "health", "settings", "project"];

function RouteView({ route, context, query }: { route: Route; context: AppContext; query: string }) {
  if (!context.env.misePath && NEEDS_MISE.includes(route.kind)) return <WelcomeView context={context} />;
  switch (route.kind) {
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
      return <HealthView context={context} />;
    case "settings":
      return <SettingsView context={context} query={query} />;
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
      return <ConfigView context={context} query={query} kind="mcp" />;
    case "integrations":
      return <ConfigView context={context} query={query} kind="integrations" />;
    case "setup":
      return <SetupView context={context} name={route.name} />;
    case "editor":
      return <SetupEditor context={context} name={route.name} seed={route.seed} />;
  }
}

function Sidebar({ route, root, context }: { route: Route; root: Route; context: AppContext }) {
  const { catalog, selections, projects, env, mise, navigate, addProject } = context;
  const platform = usePlatform();
  const installedCount = Object.keys(mise.inventory ?? {}).length;
  const updateCount = Object.keys(mise.outdated ?? {}).length;
  const health = healthSummary(mise.info, mise.doctor);

  const miseItems = [
    { kind: "installed" as const, label: "Installed", icon: Layers, trailing: installedCount || null },
    { kind: "discover" as const, label: "Discover", icon: Compass, trailing: null },
    {
      kind: "updates" as const,
      label: "Updates",
      icon: ArrowUpCircle,
      trailing: updateCount ? <span className="sidebar-badge">{updateCount}</span> : null,
    },
    { kind: "activity" as const, label: "Activity", icon: Activity, trailing: null },
    {
      kind: "health" as const,
      label: "Health",
      icon: HeartPulse,
      trailing: health.severity ? (
        <span
          className={`sidebar-badge health ${health.severity}`}
          title={`${health.count} ${health.count === 1 ? "thing needs" : "things need"} attention`}
        >
          {health.count}
        </span>
      ) : null,
    },
    { kind: "settings" as const, label: "Settings", icon: SlidersHorizontal, trailing: null },
  ];
  const aiItems = [
    { kind: "harnesses" as const, label: "Harnesses", icon: Cpu, count: catalog.harnesses.length },
    { kind: "tools" as const, label: "Agent Tools", icon: Package, count: catalog.tools.length },
    { kind: "mcp" as const, label: "MCP Servers", icon: Server, count: catalog.mcpServers.length },
    { kind: "integrations" as const, label: "Integrations", icon: Plug, count: catalog.integrations.length },
  ];
  // Detail pages keep the section they were opened from highlighted.
  const isActive = (kind: string) => root.kind === kind;

  return (
    <nav className="sidebar" data-tauri-drag-region aria-label="Sidebar">
      <div className="sidebar-scroll">
        <div className="sidebar-section">
          <div className="sidebar-heading">mise</div>
          {miseItems.map(({ kind, label, icon: Icon, trailing }) => (
            <button
              key={kind}
              className="sidebar-item"
              aria-current={isActive(kind) ? "page" : undefined}
              onClick={() => navigate({ kind })}
            >
              <Icon size={16} />
              <span className="label">{label}</span>
              {typeof trailing === "number" ? <span className="count">{trailing}</span> : trailing}
            </button>
          ))}
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
          <div className="sidebar-heading">AI Setups</div>
          {aiItems.map(({ kind, label, icon: Icon, count }) => (
            <button
              key={kind}
              className="sidebar-item"
              aria-current={isActive(kind) ? "page" : undefined}
              onClick={() => navigate({ kind })}
            >
              <Icon size={16} />
              <span className="label">{label}</span>
              <span className="count">{count}</span>
            </button>
          ))}
        </div>

        <div className="sidebar-section">
          <div className="sidebar-heading">
            My Setups
            <button onClick={() => navigate({ kind: "editor" })} aria-label="New setup" title="New Setup">
              <Plus size={14} />
            </button>
          </div>
          {selections.length === 0 && <div className="sidebar-empty">No setups yet</div>}
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
      </div>
    </nav>
  );
}
