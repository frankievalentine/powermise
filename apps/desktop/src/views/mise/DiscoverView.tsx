import { useMemo, useState } from "react";
import { ArrowRight, Check, Loader2, SquareTerminal } from "lucide-react";
import { api, type RegistryEntry } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { backendLabel, runsOn } from "../../mise";
import { Monogram } from "../../components/ui";
import popularity from "../../data/popularity.json";
import { CATEGORIES, DESCRIPTIONS, ECOSYSTEM, UTILITIES, type Utility } from "../../data/discoverCurated";

const RANKS = (popularity as { ranks: Record<string, number> }).ranks;
const UNRANKED = Number.MAX_SAFE_INTEGER;
const rankOf = (short: string) => RANKS[short] ?? UNRANKED;
const describe = (entry: RegistryEntry) => DESCRIPTIONS[entry.short] ?? entry.description ?? entry.backends[0];

/** Rows rendered before "Show all". */
const PAGE = 120;
const POPULAR_COUNT = 12;
const UTILITIES_ID = "utilities";

/** Example full names, per backend, for the "install from anywhere" hint. */
const BACKEND_EXAMPLES: Record<string, string> = {
  npm: "npm:prettier",
  github: "github:cli/cli",
  cargo: "cargo:ripgrep",
  pipx: "pipx:black",
  pypi: "pypi:black",
  go: "go:github.com/charmbracelet/glow",
  gem: "gem:rubocop",
  aqua: "aqua:hashicorp/terraform",
  ubi: "ubi:sharkdp/fd",
};

/** `backend:name` — a tool spelled out by where it installs from. */
const BACKEND_SPEC = /^([a-z][a-z0-9-]*):(\S+)$/;

/** Popularity first, then name. */
const byPopularity = (a: RegistryEntry, b: RegistryEntry) =>
  rankOf(a.short) - rankOf(b.short) || a.short.localeCompare(b.short);

export function DiscoverView({ context, query }: { context: AppContext; query: string }) {
  const { mise, navigate, env } = context;
  const [showAll, setShowAll] = useState(false);
  const [category, setCategory] = useState<string>("all");
  const backends = useAsync(() => api.backends(), []);
  const spec = query.trim().match(BACKEND_SPEC);
  const specBackend = spec && backends.data?.includes(spec[1]) ? spec[1] : null;
  // Tools mise only installs on other systems are left out.
  const registry = useMemo(
    () => (mise.registry ?? []).filter((entry) => runsOn(entry.short, env.hostTarget)),
    [mise.registry, env.hostTarget],
  );
  const categories = useMemo(
    () => CATEGORIES.filter((entry) => !entry.platforms || entry.platforms.includes(env.hostTarget)),
    [env.hostTarget],
  );
  const needle = query.trim().toLowerCase();
  // System utilities install as packages: Homebrew on macOS, apt on Linux.
  const utilitiesAvailable = env.hostTarget !== "win32";

  const results = useMemo(() => {
    if (!needle) {
      const members = categories.find((entry) => entry.id === category)?.tools;
      const pool = members ? registry.filter((entry) => members.includes(entry.short)) : registry;
      return [...pool].sort(byPopularity);
    }
    // Name matches first, then aliases, then descriptions; popularity breaks ties.
    const score = (entry: RegistryEntry) =>
      entry.short.toLowerCase() === needle
        ? 0
        : entry.short.toLowerCase().startsWith(needle)
          ? 1
          : entry.short.toLowerCase().includes(needle)
            ? 2
            : entry.aliases.some((alias) => alias.toLowerCase().includes(needle))
              ? 3
              : describe(entry).toLowerCase().includes(needle)
                ? 4
                : 9;
    return registry
      .map((entry) => ({ entry, rank: score(entry) }))
      .filter(({ rank }) => rank < 9)
      .sort((a, b) => a.rank - b.rank || byPopularity(a.entry, b.entry))
      .map(({ entry }) => entry);
  }, [registry, needle, category, categories]);

  const popular = useMemo(
    () => registry.filter((entry) => RANKS[entry.short]).sort(byPopularity).slice(0, POPULAR_COUNT),
    [registry],
  );
  const matchingUtilities = utilitiesAvailable
    ? UTILITIES.filter(
        (utility) =>
          needle &&
          `${utility.title} ${utility.id} ${utility.description} ${utility.keywords.join(" ")}`.toLowerCase().includes(needle),
      )
    : [];
  const matchingEcosystem = ECOSYSTEM.filter(
    (tool) => needle && (tool.name.includes(needle) || tool.description.toLowerCase().includes(needle)),
  );
  const visible = needle || showAll || category !== "all" ? results : results.slice(0, PAGE);
  const showingUtilities = !needle && category === UTILITIES_ID;
  const chips = [
    { id: "all", title: "All" },
    ...categories.map(({ id, title }) => ({ id, title })),
    ...(utilitiesAvailable ? [{ id: UTILITIES_ID, title: "System utilities" }] : []),
  ];

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Discover</h1>
            <p className="page-subtitle">
              {mise.registry
                ? `${registry.length.toLocaleString()} tools mise can install, most popular first.`
                : "Loading the mise registry…"}
            </p>
          </div>
        </div>

        {!needle && (
          <div className="chip-row discover-chips" role="group" aria-label="Category">
            {chips.map((chip) => (
              <button
                key={chip.id}
                type="button"
                className={`chip-button${category === chip.id ? " selected" : ""}`}
                aria-pressed={category === chip.id}
                onClick={() => {
                  setCategory(chip.id);
                  setShowAll(false);
                }}
              >
                {chip.title}
              </button>
            ))}
          </div>
        )}

        {!mise.registry && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {specBackend && (
          <section className="section" style={{ marginTop: 0 }}>
            <button className="list-row interactive install-spec" onClick={() => navigate({ kind: "tool", name: query.trim() })}>
              <Monogram id={query.trim()} title={spec![2].split("/").pop() ?? spec![2]} size={30} />
              <div className="row-main">
                <div className="row-title mono">{query.trim()}</div>
                <div className="row-subtitle">
                  Install {spec![2]} from {backendLabel(`${specBackend}:`)} — pick a version on the next page.
                </div>
              </div>
              <ArrowRight size={16} />
            </button>
          </section>
        )}

        {showingUtilities && <UtilitiesSection context={context} utilities={UTILITIES} />}

        {!needle && category === "all" && popular.length > 0 && (
          <section className="section" style={{ marginTop: 6 }}>
            <h2 className="section-title">Popular</h2>
            <p className="section-note">The tools developers install most, by Homebrew’s install counts for the past year.</p>
            <div className="grid">
              {popular.map((entry) => (
                <button key={entry.short} className="card" onClick={() => navigate({ kind: "tool", name: entry.short })}>
                  <div className="card-head">
                    <Monogram id={entry.short} title={entry.short} />
                    <div>
                      <div className="card-title">{entry.short}</div>
                      <div className="card-kind">{CATEGORIES.find((group) => group.tools.includes(entry.short))?.title ?? ""}</div>
                    </div>
                    {mise.inventory?.[entry.short] && (
                      <span className="badge green" style={{ marginLeft: "auto" }}>
                        <Check size={11} /> Installed
                      </span>
                    )}
                  </div>
                  <p className="card-description">{describe(entry)}</p>
                </button>
              ))}
            </div>
          </section>
        )}

        {(matchingUtilities.length > 0 || matchingEcosystem.length > 0) && (
          <section className="section">
            <h2 className="section-title">Also available</h2>
            {matchingUtilities.length > 0 && <UtilitiesSection context={context} utilities={matchingUtilities} compact />}
            {matchingEcosystem.length > 0 && (
              <div className="list" style={{ marginTop: matchingUtilities.length ? 10 : 0 }}>
                {matchingEcosystem.map((tool) => (
                  <button key={tool.spec} className="list-row interactive" onClick={() => navigate({ kind: "tool", name: tool.spec })}>
                    <Monogram id={tool.spec} title={tool.name} size={28} />
                    <div className="row-main">
                      <div className="row-title">{tool.name}</div>
                      <div className="row-subtitle">{tool.description}</div>
                    </div>
                    <div className="row-trailing">
                      <span className="badge mono">{tool.spec}</span>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </section>
        )}

        {mise.registry && !showingUtilities && !(specBackend && results.length === 0) && (
          <section className="section">
            <h2 className="section-title">
              {needle
                ? `Results for “${query}”`
                : category === "all"
                  ? "All tools"
                  : categories.find((entry) => entry.id === category)?.title}
            </h2>
            {results.length === 0 ? (
              <div className="empty">
                <h3>No tools match</h3>
                <p>
                  mise can also install by full name from GitHub releases, npm, cargo, and more — try{" "}
                  <code>npm:{query.trim() || "prettier"}</code>.
                </p>
              </div>
            ) : (
              <div className="list">
                {visible.map((entry) => (
                  <button key={entry.short} className="list-row interactive" onClick={() => navigate({ kind: "tool", name: entry.short })}>
                    <Monogram id={entry.short} title={entry.short} size={28} />
                    <div className="row-main">
                      <div className="row-title">{entry.short}</div>
                      <div className="row-subtitle">{describe(entry)}</div>
                    </div>
                    <div className="row-trailing">
                      {mise.inventory?.[entry.short] && <span className="badge green">Installed</span>}
                      {entry.backends[0] && <span className="badge">{backendLabel(entry.backends[0])}</span>}
                    </div>
                  </button>
                ))}
              </div>
            )}
            {!needle && category === "all" && !showAll && results.length > PAGE && (
              <div style={{ textAlign: "center", marginTop: 14 }}>
                <button className="button" onClick={() => setShowAll(true)}>
                  Show all {results.length.toLocaleString()} tools
                </button>
              </div>
            )}
          </section>
        )}

        {!needle && category === "all" && backends.data && backends.data.length > 0 && (
          <section className="section">
            <h2 className="section-title">Install from anywhere</h2>
            <p className="section-note">
              Not in the list? mise installs from {backends.data.length} sources. Search for a full name like{" "}
              {backends.data
                .filter((backend) => BACKEND_EXAMPLES[backend])
                .slice(0, 4)
                .map((backend, index, shown) => (
                  <span key={backend}>
                    <code>{BACKEND_EXAMPLES[backend]}</code>
                    {index < shown.length - 2 ? ", " : index === shown.length - 2 ? ", or " : ""}
                  </span>
                ))}
              .
            </p>
          </section>
        )}
      </div>
    </div>
  );
}

/** Everyday Unix tools installed as system packages with
 *  `mise bootstrap packages use -g`. */
function UtilitiesSection({ context, utilities, compact }: { context: AppContext; utilities: Utility[]; compact?: boolean }) {
  const { env, run, mise } = context;
  const states = useAsync(() => api.packageStates(), [mise.revision]);
  const mac = env.hostTarget === "darwin";
  const shown = utilities.filter((utility) => (mac ? utility.brew : utility.apt));
  return (
    <section className={compact ? undefined : "section"} style={compact ? undefined : { marginTop: 6 }}>
      {!compact && (
        <>
          <h2 className="section-title">System utilities</h2>
          <p className="section-note">
            Everyday command-line tools mise installs as system packages — with Homebrew on macOS (no Homebrew needed),
            or apt on Linux. They’re added to your global mise config under <code>[bootstrap.packages]</code>; delete a
            line there to stop managing one.
          </p>
        </>
      )}
      <div className="list">
        {shown.map((utility) => {
          const spec = mac ? `brew:${utility.brew}` : `apt:${utility.apt}`;
          const installed = states.data?.[spec] === "installed";
          return (
            <div className="list-row" key={utility.id}>
              <Monogram id={utility.id} title={utility.title} size={28} />
              <div className="row-main">
                <div className="row-title">{utility.title}</div>
                <div className="row-subtitle">{utility.description}</div>
              </div>
              <div className="row-trailing">
                {installed ? (
                  <span className="badge green">Installed</span>
                ) : mac ? (
                  <button
                    className="button"
                    onClick={() =>
                      run({
                        action: { kind: "usePackage", spec },
                        title: `Install ${utility.title}?`,
                        description: `mise installs the Homebrew package ${utility.brew} and adds it to your global config, so other Macs you set up with mise get it too.`,
                        verb: "Install",
                      })
                    }
                  >
                    Install
                  </button>
                ) : (
                  <button
                    className="button"
                    title="apt needs your password, so this runs in your terminal app"
                    onClick={() => void api.usePackageInTerminal(spec).catch((reason: Error) => window.alert(reason.message))}
                  >
                    <SquareTerminal size={13} /> Install in Terminal
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
