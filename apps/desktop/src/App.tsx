import { useCallback, useEffect, useMemo, useState } from "react";
import { MotionConfig, motion } from "motion/react";
import {
  AlertTriangle,
  Blocks,
  ChevronLeft,
  Cpu,
  Package,
  Plug,
  Plus,
  Search,
  Server,
  Terminal,
} from "lucide-react";
import {
  api,
  PLATFORM_LABEL,
  type EnvironmentInfo,
  type SavedSelection,
} from "./api";
import { indexCatalog, type CatalogIndex } from "./catalog";
import { HarnessesView } from "./views/HarnessesView";
import { HarnessDetail } from "./views/HarnessDetail";
import { ToolsView } from "./views/ToolsView";
import { ConfigView } from "./views/ConfigView";
import { SetupEditor } from "./views/SetupEditor";
import { SetupView } from "./views/SetupView";

export type Route =
  | { kind: "harnesses" }
  | { kind: "harness"; id: string }
  | { kind: "tools" }
  | { kind: "mcp" }
  | { kind: "integrations" }
  | { kind: "setup"; name: string }
  | { kind: "editor"; name?: string; seed?: string[] };

export interface AppContext {
  env: EnvironmentInfo;
  catalog: CatalogIndex;
  selections: SavedSelection[];
  navigate: (route: Route) => void;
  refreshSelections: () => Promise<SavedSelection[]>;
}

/** Default UI spring: no overshoot, ~0.35s (apple-design). */
const spring = { type: "spring", bounce: 0, duration: 0.35 } as const;

const routeKey = (route: Route) =>
  route.kind === "harness"
    ? `harness:${route.id}`
    : route.kind === "setup"
      ? `setup:${route.name}`
      : route.kind === "editor"
        ? `editor:${route.name ?? "new"}`
        : route.kind;

export default function App() {
  const [env, setEnv] = useState<EnvironmentInfo | null>(null);
  const [catalog, setCatalog] = useState<CatalogIndex | null>(null);
  const [selections, setSelections] = useState<SavedSelection[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<Route[]>([{ kind: "harnesses" }]);
  const [query, setQuery] = useState("");

  const route = history[history.length - 1];

  const refreshSelections = useCallback(async () => {
    const next = await api.selections();
    setSelections(next);
    return next;
  }, []);

  useEffect(() => {
    Promise.all([api.environment(), api.catalog(), api.selections()])
      .then(([environment, bundle, saved]) => {
        setEnv(environment);
        setCatalog(indexCatalog(bundle));
        setSelections(saved);
      })
      .catch((reason: Error) => setError(reason.message));
  }, []);

  const navigate = useCallback((next: Route) => {
    setQuery("");
    setHistory((stack) => {
      // Top-level destinations reset the stack; detail pages push onto it.
      const topLevel = ["harnesses", "tools", "mcp", "integrations", "setup"].includes(next.kind);
      return topLevel ? [next] : [...stack, next];
    });
  }, []);

  const goBack = () => setHistory((stack) => (stack.length > 1 ? stack.slice(0, -1) : stack));

  const context = useMemo<AppContext | null>(
    () =>
      env && catalog ? { env, catalog, selections, navigate, refreshSelections } : null,
    [env, catalog, selections, navigate, refreshSelections],
  );

  if (error) {
    return (
      <div className="empty" style={{ paddingTop: 120 }}>
        <AlertTriangle size={28} color="var(--orange)" />
        <h3>Powermise couldn’t load its catalog</h3>
        <p className="selectable">{error}</p>
      </div>
    );
  }
  if (!context) return null;

  const searchable = ["harnesses", "tools", "mcp", "integrations"].includes(route.kind);

  return (
    <MotionConfig reducedMotion="user">
      <div className="window">
        <Sidebar route={route} context={context} />
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
            {searchable && (
              <label className="search">
                <Search size={14} />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search"
                  aria-label="Search"
                />
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
    </MotionConfig>
  );
}

function toolbarTitle(route: Route, context: AppContext): string {
  switch (route.kind) {
    case "harnesses":
      return "Harnesses";
    case "harness":
      return context.catalog.harnessById.get(route.id)?.title ?? route.id;
    case "tools":
      return "Tools";
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

function RouteView({ route, context, query }: { route: Route; context: AppContext; query: string }) {
  switch (route.kind) {
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

function Sidebar({ route, context }: { route: Route; context: AppContext }) {
  const { catalog, selections, env, navigate } = context;
  const library = [
    { kind: "harnesses" as const, label: "Harnesses", icon: Cpu, count: catalog.harnesses.length },
    { kind: "tools" as const, label: "Tools", icon: Package, count: catalog.tools.length },
    { kind: "mcp" as const, label: "MCP Servers", icon: Server, count: catalog.mcpServers.length },
    {
      kind: "integrations" as const,
      label: "Integrations",
      icon: Plug,
      count: catalog.integrations.length,
    },
  ];
  const libraryActive = (kind: string) =>
    route.kind === kind || (kind === "harnesses" && route.kind === "harness");

  return (
    <nav className="sidebar" data-tauri-drag-region aria-label="Sidebar">
      <div className="sidebar-scroll">
        <div className="sidebar-section">
          <div className="sidebar-heading">Library</div>
          {library.map(({ kind, label, icon: Icon, count }) => (
            <button
              key={kind}
              className="sidebar-item"
              aria-current={libraryActive(kind) ? "page" : undefined}
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
            const failed = selection.lastRun?.operations.some((op) => op.outcome === "failed");
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
                  className={`status-dot${failed ? " attention" : selection.applied ? " applied" : ""}`}
                  title={failed ? "Last apply had failures" : selection.applied ? "Applied" : "Not applied yet"}
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
        <div className="row">
          <Terminal size={13} />
          This computer: {PLATFORM_LABEL[env.hostTarget]}
        </div>
        {env.misePath ? (
          <div className="row" title={env.misePath}>
            mise {env.miseVersion?.split(" ")[0] ?? "found"}
          </div>
        ) : (
          <div className="row warn">
            <AlertTriangle size={13} /> mise not found
          </div>
        )}
        <div className="row">Catalog revision {catalog.revision}</div>
      </div>
    </nav>
  );
}
