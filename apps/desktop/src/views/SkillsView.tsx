import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Loader2, RefreshCw } from "lucide-react";
import { api, type InstalledSkill } from "../api";
import type { AppContext } from "../App";
import { useAsync } from "../hooks";
import { CURATED_SKILLS } from "../data/skillsCurated";
import { ExternalLinkButton } from "../components/machine";
import { HarnessesTabs } from "../components/SectionTabs";
import { Monogram, Section, Switch } from "../components/ui";

const AGENTS_KEY = "powermise.skillsAgents";
const TELEMETRY_KEY = "powermise.skillsTelemetry";

function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // A convenience only; the defaults come back next time.
  }
}

/** Agent skills: folders of instructions an agent loads when they're relevant,
 *  installed for each agent with the skills CLI from skills.sh. */
export function SkillsView({ context, query }: { context: AppContext; query: string }) {
  const { mise, run } = context;
  const report = useAsync(() => api.skillsReport(), [mise.revision]);
  const [agents, setAgents] = useState<string[] | null>(() => readStored<string[] | null>(AGENTS_KEY, null));
  const [telemetry, setTelemetry] = useState(() => readStored(TELEMETRY_KEY, false));
  const [source, setSource] = useState("");
  const [names, setNames] = useState("");

  // Until you choose, install for the agents that look set up here.
  const detected = useMemo(
    () => report.data?.agents.filter((agent) => agent.detected).map((agent) => agent.id) ?? [],
    [report.data],
  );
  const chosen = agents ?? detected;
  useEffect(() => {
    if (agents) store(AGENTS_KEY, agents);
  }, [agents]);
  const toggleAgent = (id: string) =>
    setAgents(chosen.includes(id) ? chosen.filter((entry) => entry !== id) : [...chosen, id]);
  const setReporting = (on: boolean) => {
    setTelemetry(on);
    store(TELEMETRY_KEY, on);
  };

  const needle = query.trim().toLowerCase();
  const matches = (name: string, description?: string | null) =>
    !needle || name.toLowerCase().includes(needle) || (description ?? "").toLowerCase().includes(needle);
  const installed = (report.data?.installed ?? []).filter((skill) => matches(skill.name, skill.description));
  const installedNames = new Set(report.data?.installed.map((skill) => skill.name));
  const suggested = CURATED_SKILLS.filter((skill) => !installedNames.has(skill.name) && matches(skill.name, skill.description));
  const agentTitle = (id: string) => report.data?.agents.find((agent) => agent.id === id)?.title ?? id;
  const forAgents = new Intl.ListFormat("en", { type: "conjunction" }).format(chosen.map(agentTitle));

  const add = (from: string, skills: string[]) =>
    run({
      action: { kind: "skillsAdd", source: from, skills, agents: chosen, telemetry },
      title: skills.length === 1 ? `Install ${skills[0]}?` : `Install skills from ${from}?`,
      description: `The skills CLI downloads ${
        skills.length ? skills.join(", ") : "every skill"
      } from ${from} and adds it for ${forAgents}. Skills are instructions your agent follows, so only add ones you trust.`,
      verb: "Install",
    });
  const remove = (skill: InstalledSkill) =>
    run({
      action: { kind: "skillsRemove", name: skill.name, telemetry },
      title: `Remove ${skill.name}?`,
      description: `The skills CLI removes it from ${skill.agents.map(agentTitle).join(", ")}.`,
      verb: "Remove",
      destructive: true,
    });
  const extraNames = names
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean);
  const validSource = /^([\w.-]+\/[\w.-]+|https:\/\/\S+)$/.test(source.trim());

  return (
    <div className="content">
      <div className="page">
        <HarnessesTabs context={context} current="skills" />
        <div className="page-header">
          <div>
            <h1 className="page-title">Harnesses</h1>
            <p className="page-subtitle">
              Skills are folders of instructions an agent loads when they’re relevant — how to fill a PDF, or how your
              team writes React. powermise installs them with the skills CLI from skills.sh, which mise runs for you.
            </p>
          </div>
          <div className="detail-actions">
            <ExternalLinkButton url="https://skills.sh">Browse skills.sh</ExternalLinkButton>
          </div>
        </div>

        {report.loading && !report.data && (
          <p className="field-hint">
            <Loader2 size={12} className="spin" /> Reading your skills…
          </p>
        )}
        {report.error && (
          <div className="callout error selectable">
            <AlertTriangle size={14} />
            <span>{report.error}</span>
          </div>
        )}

        {report.data && (
          <Section title="Install for" note="New skills go to these agents. Agents set up on this computer are checked.">
            <div className="chip-row discover-chips" role="group" aria-label="Agents">
              {report.data.agents.map((agent) => (
                <button
                  key={agent.id}
                  type="button"
                  className={`chip-button${chosen.includes(agent.id) ? " selected" : ""}`}
                  aria-pressed={chosen.includes(agent.id)}
                  onClick={() => toggleAgent(agent.id)}
                >
                  {agent.title}
                </button>
              ))}
            </div>
            <div className="option-row" style={{ marginTop: 10 }}>
              <div className="row-main">
                <div className="row-title">Share anonymous install counts with skills.sh</div>
                <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                  The skills CLI reports which public skills get installed, for skills.sh’s rankings. Off unless you turn
                  it on.
                </div>
              </div>
              <Switch label="Share install counts" checked={telemetry} onChange={setReporting} />
            </div>
          </Section>
        )}

        {report.data && (
          <Section
            title="Installed"
            note={report.data.installed.length ? undefined : "No skills yet. Start with one below."}
          >
            {report.data.installed.length > 0 && (
              <>
                <div className="detail-actions" style={{ marginBottom: 8 }}>
                  <button
                    className="button"
                    onClick={() =>
                      run({
                        action: { kind: "skillsUpdate", telemetry },
                        title: "Update all skills?",
                        description: "The skills CLI fetches the latest version of each skill from where it came from.",
                        verb: "Update",
                      })
                    }
                  >
                    <RefreshCw size={13} /> Update All
                  </button>
                </div>
                <div className="list">
                  {installed.map((skill) => (
                    <div className="list-row" key={skill.name}>
                      <Monogram id={skill.name} title={skill.name} size={28} />
                      <div className="row-main">
                        <div className="row-title">{skill.name}</div>
                        <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                          {skill.description ?? "No description"}
                        </div>
                        <div className="row-subtitle">
                          {skill.agents.map(agentTitle).join(", ")}
                          {skill.source ? ` · from ${skill.source}` : " · added by hand"}
                        </div>
                      </div>
                      <div className="row-trailing">
                        {skill.source && (
                          <button className="button" onClick={() => remove(skill)}>
                            Remove…
                          </button>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}
          </Section>
        )}

        {report.data && suggested.length > 0 && (
          <Section title="Suggested" note="Popular skills from Anthropic, Vercel, and the community.">
            <div className="list">
              {suggested.map((skill) => (
                <div className="list-row" key={`${skill.source}/${skill.name}`}>
                  <Monogram id={skill.name} title={skill.name} size={28} />
                  <div className="row-main">
                    <div className="row-title">{skill.name}</div>
                    <div className="row-subtitle" style={{ whiteSpace: "normal" }}>
                      {skill.description}
                    </div>
                    <div className="row-subtitle mono">{skill.source}</div>
                  </div>
                  <div className="row-trailing">
                    <button className="button" disabled={!chosen.length} onClick={() => add(skill.source, [skill.name])}>
                      Install
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </Section>
        )}

        {report.data && (
          <Section
            title="Add from GitHub"
            note="A repository (owner/repo) or a link from skills.sh. Name the skills you want, or leave it empty for all of them."
          >
            <div className="install-row">
              <input
                className="text-field mono"
                style={{ flex: 3 }}
                value={source}
                onChange={(event) => setSource(event.target.value)}
                placeholder="vercel-labs/agent-skills"
                aria-label="Skill source"
                spellCheck={false}
              />
              <input
                className="text-field"
                style={{ flex: 2 }}
                value={names}
                onChange={(event) => setNames(event.target.value)}
                placeholder="Skills, comma-separated (optional)"
                aria-label="Skill names"
                spellCheck={false}
              />
              <button
                className="button primary"
                disabled={!validSource || !chosen.length}
                onClick={() => add(source.trim(), extraNames)}
              >
                Install
              </button>
            </div>
            <p className={`field-hint${chosen.length ? "" : " error"}`} style={{ marginTop: 8 }}>
              {chosen.length ? `Installs for ${forAgents}.` : "Pick at least one agent above."}
            </p>
          </Section>
        )}

        {report.data && report.data.toolSkills.length > 0 && (
          <Section
            title="From your tools"
            note="Some tools mise installs ship their own skills, matched to the version you use. mise links them for Claude Code."
          >
            <div className="list">
              {report.data.toolSkills.map((skill) => (
                <div className="list-row" key={`${skill.tool}/${skill.name}`}>
                  <div className="row-main">
                    <div className="row-title">{skill.name}</div>
                    <div className="row-subtitle mono">
                      {skill.tool} {skill.version}
                    </div>
                  </div>
                </div>
              ))}
            </div>
            <div style={{ marginTop: 8 }}>
              <button
                className="button"
                onClick={() =>
                  run({
                    action: { kind: "skillsSyncTools" },
                    title: "Link your tools’ skills?",
                    description: "mise links each one into ~/.claude/skills, following the tool version you use.",
                    verb: "Link",
                  })
                }
              >
                Link for Claude Code
              </button>
            </div>
          </Section>
        )}

      </div>
    </div>
  );
}
