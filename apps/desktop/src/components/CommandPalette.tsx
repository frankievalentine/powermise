import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowRight, Blocks, Compass, Folder, Layers, Search, SlidersHorizontal } from "lucide-react";
import type { AppContext, Route } from "../App";
import { COMMON_SETTINGS } from "../data/commonSettings";
import popularity from "../data/popularity.json";
import { runsOn } from "../mise";

const RANKS = (popularity as { ranks: Record<string, number> }).ranks;

interface Result {
  id: string;
  group: string;
  title: string;
  detail?: string;
  icon: typeof Search;
  run: () => void;
}

const PAGES: { title: string; route: Route; keywords: string }[] = [
  { title: "Home", route: { kind: "home" }, keywords: "start getting started checklist" },
  { title: "My Tools", route: { kind: "installed" }, keywords: "installed tools" },
  { title: "Updates", route: { kind: "updates" }, keywords: "upgrade outdated new versions" },
  { title: "Discover", route: { kind: "discover" }, keywords: "install find browse registry" },
  { title: "Harnesses", route: { kind: "harnesses" }, keywords: "ai agents browse claude codex chatgpt amp cursor" },
  { title: "MCP Servers", route: { kind: "mcp" }, keywords: "ai agents model context protocol github linear notion stripe" },
  { title: "Plugins", route: { kind: "integrations" }, keywords: "ai agents claude codex gemini extensions" },
  { title: "Skills", route: { kind: "skills" }, keywords: "ai agents skills.sh instructions" },
  { title: "This Computer", route: { kind: "machine" }, keywords: "machine bootstrap packages dotfiles" },
  { title: "Import", route: { kind: "import" }, keywords: "bring over brewfile homebrew nvm pyenv asdf migrate" },
  { title: "macOS Settings", route: { kind: "macos-settings" }, keywords: "dock finder defaults mac" },
  { title: "Health", route: { kind: "health" }, keywords: "doctor problems trust cache disk github" },
  { title: "Settings", route: { kind: "settings" }, keywords: "preferences options" },
  { title: "History", route: { kind: "activity" }, keywords: "activity log commands" },
];

/** ⌘K: one search for pages, tools, projects, setups, and settings. */
export function CommandPalette({
  open,
  onClose,
  context,
  setQuery,
}: {
  open: boolean;
  onClose: () => void;
  context: AppContext;
  /** Sets the page search after navigating (for settings matches). */
  setQuery: (query: string) => void;
}) {
  const [text, setText] = useState("");
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const { navigate, mise, projects, selections, env } = context;

  useEffect(() => {
    if (open) {
      setText("");
      setIndex(0);
      requestAnimationFrame(() => input.current?.focus());
    }
  }, [open]);

  const results = useMemo<Result[]>(() => {
    const needle = text.trim().toLowerCase();
    const go = (route: Route, query?: string) => () => {
      onClose();
      navigate(route);
      if (query) setTimeout(() => setQuery(query), 0);
    };
    const matches = (...parts: (string | undefined)[]) =>
      !needle || parts.some((part) => part?.toLowerCase().includes(needle));
    const out: Result[] = [];

    for (const page of PAGES) {
      if (page.route.kind === "macos-settings" && env.hostTarget !== "darwin") continue;
      if (matches(page.title, page.keywords)) {
        out.push({ id: `page:${page.title}`, group: "Go to", title: page.title, icon: ArrowRight, run: go(page.route) });
      }
    }
    if (!needle) return out;

    for (const name of Object.keys(mise.inventory ?? {})) {
      if (matches(name)) {
        out.push({ id: `tool:${name}`, group: "My tools", title: name, icon: Layers, run: go({ kind: "tool", name }) });
      }
    }
    const installed = new Set(Object.keys(mise.inventory ?? {}));
    const registry = (mise.registry ?? [])
      .filter(
        (entry) =>
          !installed.has(entry.short) && runsOn(entry.short, env.hostTarget) && matches(entry.short, ...entry.aliases),
      )
      .sort((a, b) => (RANKS[a.short] ?? 1e9) - (RANKS[b.short] ?? 1e9) || a.short.localeCompare(b.short))
      .slice(0, 6);
    for (const entry of registry) {
      out.push({
        id: `discover:${entry.short}`,
        group: "Install",
        title: entry.short,
        detail: entry.description ?? undefined,
        icon: Compass,
        run: go({ kind: "tool", name: entry.short }),
      });
    }
    for (const project of projects) {
      if (matches(project.name, project.path)) {
        out.push({ id: `project:${project.path}`, group: "Projects", title: project.name, icon: Folder, run: go({ kind: "project", path: project.path }) });
      }
    }
    for (const selection of selections) {
      if (matches(selection.name)) {
        out.push({ id: `setup:${selection.name}`, group: "AI setups", title: selection.name, icon: Blocks, run: go({ kind: "setup", name: selection.name }) });
      }
    }
    for (const group of COMMON_SETTINGS) {
      for (const setting of group.settings) {
        if (matches(setting.title, setting.key, ...setting.keywords)) {
          out.push({
            id: `setting:${setting.key}`,
            group: "Settings",
            title: setting.title,
            detail: group.title,
            icon: SlidersHorizontal,
            run: go({ kind: "settings" }, setting.title),
          });
        }
      }
    }
    return out;
  }, [text, mise.inventory, mise.registry, projects, selections, env.hostTarget, navigate, onClose, setQuery]);

  useEffect(() => setIndex(0), [text]);

  if (!open) return null;

  const onKey = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setIndex((current) => Math.min(results.length - 1, current + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setIndex((current) => Math.max(0, current - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      results[index]?.run();
    }
  };

  let lastGroup = "";
  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div className="palette" role="dialog" aria-label="Search everything" onMouseDown={(event) => event.stopPropagation()}>
        <div className="palette-input">
          <Search size={16} />
          <input
            ref={input}
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={onKey}
            placeholder="Search tools, projects, setups, settings…"
            aria-label="Search everything"
            spellCheck={false}
          />
          <kbd>esc</kbd>
        </div>
        <div className="palette-results" role="listbox">
          {results.length === 0 && <div className="palette-empty">No matches.</div>}
          {results.map((result, position) => {
            const header = result.group !== lastGroup ? result.group : null;
            lastGroup = result.group;
            const Icon = result.icon;
            return (
              <div key={result.id}>
                {header && <div className="palette-group">{header}</div>}
                <button
                  role="option"
                  aria-selected={position === index}
                  className={`palette-item${position === index ? " selected" : ""}`}
                  onMouseEnter={() => setIndex(position)}
                  onClick={result.run}
                >
                  <Icon size={14} />
                  <span className="palette-title">{result.title}</span>
                  {result.detail && <span className="palette-detail">{result.detail}</span>}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
