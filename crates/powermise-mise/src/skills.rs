//! Agent skills (folders with a `SKILL.md`), installed with the `skills` CLI
//! from skills.sh (<https://github.com/vercel-labs/skills>), which mise
//! installs as an npm tool.
//!
//! Reading what's installed doesn't need the CLI: it records global installs
//! in `~/.agents/.skill-lock.json` (or `$XDG_STATE_HOME/skills/…`) and links
//! each skill into every agent's skills folder, so powermise reads the lock
//! and those folders. Installing, updating, and removing run the CLI through
//! `mise x`, with its usage reporting off unless you turn it on.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::MiseCommand;

/// The `skills` CLI version powermise runs.
pub const SKILLS_CLI: &str = "npm:skills@1.7.2";

/// An agent the skills CLI installs into: its `-a` name, a title, its global
/// skills folder, and the folder whose presence means the agent is set up.
pub struct SkillsAgent {
    pub id: &'static str,
    pub title: &'static str,
    skills_dir: fn(&Path) -> PathBuf,
    detect: fn(&Path) -> PathBuf,
}

fn config(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| home.join(".config"), PathBuf::from)
}

/// The agents powermise offers (from the skills CLI's own table).
pub const SKILLS_AGENTS: &[SkillsAgent] = &[
    SkillsAgent {
        id: "claude-code",
        title: "Claude Code",
        skills_dir: |home| home.join(".claude/skills"),
        detect: |home| home.join(".claude"),
    },
    SkillsAgent {
        id: "codex",
        title: "Codex",
        skills_dir: |home| home.join(".agents/skills"),
        detect: |home| home.join(".codex"),
    },
    SkillsAgent {
        id: "gemini-cli",
        title: "Gemini CLI",
        skills_dir: |home| home.join(".gemini/skills"),
        detect: |home| home.join(".gemini"),
    },
    SkillsAgent {
        id: "cursor",
        title: "Cursor",
        skills_dir: |home| home.join(".cursor/skills"),
        detect: |home| home.join(".cursor"),
    },
    SkillsAgent {
        id: "opencode",
        title: "OpenCode",
        skills_dir: |home| config(home).join("opencode/skills"),
        detect: |home| config(home).join("opencode"),
    },
    SkillsAgent {
        id: "github-copilot",
        title: "GitHub Copilot",
        skills_dir: |home| home.join(".copilot/skills"),
        detect: |home| home.join(".copilot"),
    },
    SkillsAgent {
        id: "goose",
        title: "Goose",
        skills_dir: |home| config(home).join("goose/skills"),
        detect: |home| config(home).join("goose"),
    },
    SkillsAgent {
        id: "antigravity",
        title: "Antigravity",
        skills_dir: |home| home.join(".gemini/antigravity/skills"),
        detect: |home| home.join(".gemini/antigravity"),
    },
    SkillsAgent {
        id: "hermes-agent",
        title: "Hermes Agent",
        skills_dir: |home| home.join(".hermes/skills"),
        detect: |home| home.join(".hermes"),
    },
];

/// Agents that look set up on this computer (their config folder exists).
pub fn detected_skills_agents(home: &Path) -> Vec<&'static str> {
    SKILLS_AGENTS
        .iter()
        .filter(|agent| (agent.detect)(home).is_dir())
        .map(|agent| agent.id)
        .collect()
}

/// A skill installed for one or more agents.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkill {
    pub name: String,
    pub description: Option<String>,
    /// Where it came from (`owner/repo`), when the skills CLI installed it.
    pub source: Option<String>,
    /// The agents (`-a` names) that have it.
    pub agents: Vec<String>,
}

/// Every skill in the agents' global skills folders, with its source from
/// the skills CLI's lock file.
pub fn installed_skills(home: &Path) -> Vec<InstalledSkill> {
    let lock = read_lock(home);
    let mut skills: BTreeMap<String, InstalledSkill> = BTreeMap::new();
    for agent in SKILLS_AGENTS {
        let Ok(entries) = fs::read_dir((agent.skills_dir)(home)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let manifest = entry.path().join("SKILL.md");
            let Ok(text) = fs::read_to_string(&manifest) else {
                continue;
            };
            let (name, description) = frontmatter(&text);
            let name = name.unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            let skill = skills
                .entry(name.clone())
                .or_insert_with(|| InstalledSkill {
                    source: lock.get(&name).cloned(),
                    description,
                    name,
                    agents: Vec::new(),
                });
            if !skill.agents.iter().any(|id| id == agent.id) {
                skill.agents.push(agent.id.into());
            }
        }
    }
    skills.into_values().collect()
}

/// Skill name → source, from the global lock file.
fn read_lock(home: &Path) -> BTreeMap<String, String> {
    let path = std::env::var_os("XDG_STATE_HOME").map_or_else(
        || home.join(".agents/.skill-lock.json"),
        |state| PathBuf::from(state).join("skills/.skill-lock.json"),
    );
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|lock| lock.get("skills")?.as_object().cloned())
        .map(|skills| {
            skills
                .into_iter()
                .filter_map(|(name, entry)| Some((name, entry.get("source")?.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// `name` and `description` from a SKILL.md's YAML frontmatter.
fn frontmatter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let (mut name, mut description) = (None, None);
    for line in lines.take_while(|line| line.trim() != "---") {
        let value = |prefix: &str| {
            line.strip_prefix(prefix)
                .map(|rest| rest.trim().trim_matches('"').trim_matches('\'').to_owned())
                .filter(|rest| !rest.is_empty() && rest != ">" && rest != "|")
        };
        name = name.or_else(|| value("name:"));
        description = description.or_else(|| value("description:"));
    }
    (name, description)
}

/// Sources (`owner/repo`, an https URL) and skill names powermise passes on:
/// nothing that reads as a flag, no control characters.
fn validate(text: &str, what: &str) -> Result<(), String> {
    if text.trim().is_empty()
        || text.starts_with('-')
        || text.len() > 512
        || text.chars().any(char::is_control)
    {
        return Err(format!("`{text}` isn't a {what} powermise can pass on"));
    }
    Ok(())
}

fn skills_command(arguments: &[&str], telemetry: bool) -> MiseCommand {
    let mut command = MiseCommand::exec_tool(SKILLS_CLI, "skills", arguments);
    if !telemetry {
        command = command.with_env("DISABLE_TELEMETRY", "1");
    }
    command
}

/// `skills add <source> -g -y -a <agent>… --skill <name>…` (every skill in
/// the source when `skills` is empty).
pub fn skills_add(
    source: &str,
    skills: &[String],
    agents: &[String],
    telemetry: bool,
) -> Result<MiseCommand, String> {
    validate(source, "skill source")?;
    if source.contains(':') && !source.starts_with("https://") {
        return Err(format!("`{source}`: use owner/repo or an https link"));
    }
    if agents.is_empty() {
        return Err("pick at least one agent".into());
    }
    let mut arguments = vec!["add", source, "-g", "-y"];
    for agent in agents {
        if !SKILLS_AGENTS.iter().any(|known| known.id == agent) {
            return Err(format!("unknown agent `{agent}`"));
        }
        arguments.extend(["-a", agent.as_str()]);
    }
    if skills.is_empty() {
        arguments.extend(["--skill", "*"]);
    }
    for skill in skills {
        validate(skill, "skill name")?;
        arguments.extend(["--skill", skill.as_str()]);
    }
    Ok(skills_command(&arguments, telemetry))
}

/// `skills remove <name> -g -y`: from every agent.
pub fn skills_remove(name: &str, telemetry: bool) -> Result<MiseCommand, String> {
    validate(name, "skill name")?;
    Ok(skills_command(&["remove", name, "-g", "-y"], telemetry))
}

/// `skills update -g -y`: every global skill to its source's latest.
pub fn skills_update(telemetry: bool) -> MiseCommand {
    skills_command(&["update", "-g", "-y"], telemetry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_skills_from_agent_folders_and_the_lock() {
        let home = std::env::temp_dir().join(format!("powermise-skills-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        for dir in [
            ".claude/skills/frontend-design",
            ".agents/skills/frontend-design",
            ".claude/skills/mine",
        ] {
            fs::create_dir_all(home.join(dir)).unwrap();
        }
        let skill =
            "---\nname: frontend-design\ndescription: \"Distinctive interfaces\"\n---\n# Body\n";
        fs::write(home.join(".claude/skills/frontend-design/SKILL.md"), skill).unwrap();
        fs::write(home.join(".agents/skills/frontend-design/SKILL.md"), skill).unwrap();
        fs::write(
            home.join(".claude/skills/mine/SKILL.md"),
            "# no frontmatter\n",
        )
        .unwrap();
        fs::write(
            home.join(".agents/.skill-lock.json"),
            r#"{"version":3,"skills":{"frontend-design":{"source":"anthropics/skills"}}}"#,
        )
        .unwrap();
        let skills = installed_skills(&home);
        fs::remove_dir_all(&home).unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "frontend-design");
        assert_eq!(
            skills[0].description.as_deref(),
            Some("Distinctive interfaces")
        );
        assert_eq!(skills[0].source.as_deref(), Some("anthropics/skills"));
        // `.agents/skills` is Codex's folder.
        assert_eq!(skills[0].agents, ["claude-code", "codex"]);
        assert_eq!(skills[1].name, "mine");
        assert_eq!(skills[1].source, None);
    }

    #[test]
    fn builds_commands_through_mise_with_reporting_off() {
        let add = skills_add(
            "anthropics/skills",
            &["frontend-design".into()],
            &["claude-code".into(), "codex".into()],
            false,
        )
        .unwrap();
        assert_eq!(
            add.display(),
            "DISABLE_TELEMETRY=1 mise x npm:skills@1.7.2 -- skills add anthropics/skills -g -y -a claude-code -a codex --skill frontend-design"
        );
        assert!(skills_add("--evil", &[], &["codex".into()], false).is_err());
        assert!(skills_add("file:///etc", &[], &["codex".into()], false).is_err());
        assert!(skills_add("a/b", &[], &["nope".into()], false).is_err());
        assert_eq!(
            skills_update(true).display(),
            "mise x npm:skills@1.7.2 -- skills update -g -y"
        );
    }
}
