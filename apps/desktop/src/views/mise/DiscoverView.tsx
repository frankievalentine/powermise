import { useMemo, useState } from "react";
import { ArrowRight, Check, Loader2 } from "lucide-react";
import { api } from "../../api";
import type { AppContext } from "../../App";
import { useAsync } from "../../hooks";
import { backendLabel } from "../../mise";
import { Monogram } from "../../components/ui";

/** Tools worth surfacing before anyone searches. */
const FEATURED = ["node", "python", "go", "rust", "bun", "uv", "terraform", "kubectl", "gh", "jq", "ripgrep", "deno"];

/** Rows rendered before search narrows the list. */
const PAGE = 120;

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

export function DiscoverView({ context, query }: { context: AppContext; query: string }) {
  const { mise, navigate } = context;
  const [showAll, setShowAll] = useState(false);
  const backends = useAsync(() => api.backends(), []);
  const spec = query.trim().match(BACKEND_SPEC);
  const specBackend = spec && backends.data?.includes(spec[1]) ? spec[1] : null;

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const entries = mise.registry ?? [];
    if (!needle) return entries;
    // Name matches first, then aliases, then descriptions.
    const score = (entry: (typeof entries)[number]) =>
      entry.short.toLowerCase() === needle
        ? 0
        : entry.short.toLowerCase().startsWith(needle)
          ? 1
          : entry.short.toLowerCase().includes(needle)
            ? 2
            : entry.aliases.some((alias) => alias.toLowerCase().includes(needle))
              ? 3
              : entry.description?.toLowerCase().includes(needle)
                ? 4
                : 9;
    return entries
      .map((entry) => ({ entry, rank: score(entry) }))
      .filter(({ rank }) => rank < 9)
      .sort((a, b) => a.rank - b.rank || a.entry.short.localeCompare(b.entry.short))
      .map(({ entry }) => entry);
  }, [mise.registry, query]);

  const featured = (mise.registry ?? []).filter((entry) => FEATURED.includes(entry.short));
  const visible = query || showAll ? results : results.slice(0, PAGE);

  return (
    <div className="content">
      <div className="page">
        <div className="page-header">
          <div>
            <h1 className="page-title">Discover</h1>
            <p className="page-subtitle">
              {mise.registry
                ? `${mise.registry.length.toLocaleString()} tools mise can install — languages, CLIs, and cloud tools.`
                : "Loading the mise registry…"}
            </p>
          </div>
        </div>

        {!mise.registry && (
          <div className="empty">
            <Loader2 size={20} className="spin" />
          </div>
        )}

        {!query && featured.length > 0 && (
          <section className="section" style={{ marginTop: 0 }}>
            <h2 className="section-title">Popular</h2>
            <div className="grid">
              {featured.map((entry) => (
                <button
                  key={entry.short}
                  className="card"
                  onClick={() => navigate({ kind: "tool", name: entry.short })}
                >
                  <div className="card-head">
                    <Monogram id={entry.short} title={entry.short} />
                    <div>
                      <div className="card-title">{entry.short}</div>
                      <div className="card-kind">{entry.backends[0] && backendLabel(entry.backends[0])}</div>
                    </div>
                    {mise.inventory?.[entry.short] && (
                      <span className="badge green" style={{ marginLeft: "auto" }}>
                        <Check size={11} /> Installed
                      </span>
                    )}
                  </div>
                  <p className="card-description">{entry.description}</p>
                </button>
              ))}
            </div>
          </section>
        )}

        {specBackend && (
          <section className="section" style={{ marginTop: 0 }}>
            <button
              className="list-row interactive install-spec"
              onClick={() => navigate({ kind: "tool", name: query.trim() })}
            >
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

        {!query && backends.data && backends.data.length > 0 && (
          <section className="section" style={{ marginTop: featured.length ? undefined : 0 }}>
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
            <div className="chip-row">
              {backends.data
                .filter((backend) => !["core", "asdf", "vfox"].includes(backend))
                .map((backend) => (
                  <span key={backend} className="badge mono" title={BACKEND_EXAMPLES[backend] ?? `${backend}:<name>`}>
                    {backend}:
                  </span>
                ))}
            </div>
          </section>
        )}

        {mise.registry && !(specBackend && results.length === 0) && (
          <section className="section">
            <h2 className="section-title">{query ? `Results for “${query}”` : "All tools"}</h2>
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
                  <button
                    key={entry.short}
                    className="list-row interactive"
                    onClick={() => navigate({ kind: "tool", name: entry.short })}
                  >
                    <Monogram id={entry.short} title={entry.short} size={28} />
                    <div className="row-main">
                      <div className="row-title">{entry.short}</div>
                      <div className="row-subtitle">{entry.description ?? entry.backends[0]}</div>
                    </div>
                    <div className="row-trailing">
                      {mise.inventory?.[entry.short] && <span className="badge green">Installed</span>}
                      {entry.backends[0] && <span className="badge">{backendLabel(entry.backends[0])}</span>}
                    </div>
                  </button>
                ))}
              </div>
            )}
            {!query && !showAll && results.length > PAGE && (
              <div style={{ textAlign: "center", marginTop: 14 }}>
                <button className="button" onClick={() => setShowAll(true)}>
                  Show all {results.length.toLocaleString()} tools
                </button>
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
