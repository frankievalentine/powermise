//! Project-level reads: trust status, environment variables, task details,
//! and a read-only preview of an untrusted config.
//!
//! mise refuses to load an untrusted config because it can set environment
//! variables, source scripts, and run hooks. Before asking someone to trust a
//! folder, powermise parses the config files itself — without executing
//! anything — to show what trusting would allow.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// Whether mise trusts a project's config.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustStatus {
    pub path: PathBuf,
    pub trusted: bool,
}

/// Parses `mise trust --show` (`<path>: trusted` / `<path>: untrusted`).
pub fn parse_trust_show(output: &str) -> Option<TrustStatus> {
    let line = output.lines().rev().find(|line| line.contains(": "))?;
    let (path, state) = line.rsplit_once(": ")?;
    let trusted = match state.trim() {
        "trusted" => true,
        "untrusted" => false,
        _ => return None,
    };
    Some(TrustStatus {
        path: PathBuf::from(path.trim()),
        trusted,
    })
}

/// Whether a failed mise command failed because a config isn't trusted.
pub fn is_untrusted_error(stderr: &str) -> bool {
    stderr.contains("are not trusted") || stderr.contains("is not trusted")
}

/// One variable from `mise env --json-extended`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvVar {
    pub value: String,
    #[serde(default)]
    pub source: Option<PathBuf>,
}

pub type EnvReport = BTreeMap<String, EnvVar>;

/// Names that usually hold secrets, masked even when mise doesn't mark them
/// `redact = true`.
pub fn looks_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "APIKEY",
        "PRIVATE",
        "CREDENTIAL",
    ]
    .iter()
    .any(|marker| upper.contains(marker))
        || upper.ends_with("_KEY")
}

/// A task argument or flag from the usage spec in `mise tasks info --json`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub name: String,
    /// How it's written on the command line: `<env>` or `--dry-run`.
    pub usage: String,
    pub help: Option<String>,
    pub required: bool,
    /// For flags: whether it takes a value (`--level <n>`) or is a switch.
    pub takes_value: bool,
    pub choices: Vec<String>,
    pub default: Option<String>,
}

/// The parts of `mise tasks info --json` the run form needs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetails {
    pub name: String,
    pub description: String,
    pub run: Vec<String>,
    pub depends: Vec<String>,
    pub args: Vec<TaskInput>,
    pub flags: Vec<TaskInput>,
}

/// Extracts the run form's fields from `mise tasks info --json`. Tolerant of
/// missing pieces: a task without a usage spec simply has no inputs.
pub fn parse_task_details(json: &serde_json::Value) -> TaskDetails {
    let text = |value: &serde_json::Value| value.as_str().map(str::to_owned);
    let strings = |value: Option<&serde_json::Value>| -> Vec<String> {
        value
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let input = |entry: &serde_json::Value, is_flag: bool| TaskInput {
        name: entry.get("name").and_then(text).unwrap_or_default(),
        usage: entry.get("usage").and_then(text).unwrap_or_default(),
        help: entry
            .get("help_first_line")
            .or_else(|| entry.get("help"))
            .and_then(text),
        required: entry
            .get("required")
            .and_then(|value| value.as_bool())
            .unwrap_or(!is_flag),
        takes_value: !is_flag || entry.get("arg").is_some_and(|arg| !arg.is_null()),
        choices: strings(
            entry
                .get("choices")
                .and_then(|choices| choices.get("choices"))
                .or_else(|| {
                    entry
                        .get("arg")
                        .and_then(|arg| arg.get("choices"))
                        .and_then(|c| c.get("choices"))
                }),
        ),
        default: entry.get("default").and_then(|value| match value {
            serde_json::Value::String(text) => Some(text.clone()),
            serde_json::Value::Array(items) => items
                .first()
                .and_then(|item| item.as_str())
                .map(str::to_owned),
            _ => None,
        }),
    };
    let cmd = json.get("usage_spec").and_then(|spec| spec.get("cmd"));
    let list = |key: &str, is_flag: bool| -> Vec<TaskInput> {
        cmd.and_then(|cmd| cmd.get(key))
            .and_then(|items| items.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter(|item| {
                        !item
                            .get("hide")
                            .and_then(|hide| hide.as_bool())
                            .unwrap_or(false)
                    })
                    .map(|item| input(item, is_flag))
                    .collect()
            })
            .unwrap_or_default()
    };
    TaskDetails {
        name: json.get("name").and_then(text).unwrap_or_default(),
        description: json.get("description").and_then(text).unwrap_or_default(),
        run: strings(json.get("run")),
        depends: json
            .get("depends")
            .and_then(|items| items.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .or_else(|| item.get("task").and_then(text))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        args: list("args", false),
        flags: list("flags", true),
    }
}

/// What an untrusted config would do once trusted, read without executing it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreview {
    pub path: PathBuf,
    /// `tool = version` requests.
    pub tools: Vec<(String, String)>,
    /// Environment variable names it sets (values are not shown).
    pub env_names: Vec<String>,
    /// `_.file`, `_.source`, and `_.path` directives: files it loads or scripts it runs.
    pub env_directives: Vec<String>,
    /// `[hooks]`: commands mise runs automatically (on enter, cd, install, …).
    pub hooks: Vec<(String, String)>,
    /// Task names and the commands they run.
    pub tasks: Vec<(String, String)>,
    /// Set when the file couldn't be parsed; the preview is then incomplete.
    pub parse_error: Option<String>,
}

/// Config file names mise reads from a project directory.
pub const PROJECT_CONFIG_FILES: &[&str] = &[
    "mise.toml",
    ".mise.toml",
    "mise.local.toml",
    ".mise.local.toml",
    "mise/config.toml",
    ".mise/config.toml",
    ".config/mise.toml",
    ".config/mise/config.toml",
];

/// Previews every mise config file in `project` without executing anything.
pub fn preview_project_configs(project: &Path) -> Vec<ConfigPreview> {
    PROJECT_CONFIG_FILES
        .iter()
        .map(|name| project.join(name))
        .filter(|path| path.is_file())
        .map(|path| match fs::read_to_string(&path) {
            Ok(contents) => preview_config(&path, &contents),
            Err(error) => ConfigPreview {
                path,
                parse_error: Some(error.to_string()),
                ..Default::default()
            },
        })
        .collect()
}

/// Reads one config's contents into a preview.
pub fn preview_config(path: &Path, contents: &str) -> ConfigPreview {
    let mut preview = ConfigPreview {
        path: path.to_path_buf(),
        ..Default::default()
    };
    let table = match contents.parse::<toml::Table>() {
        Ok(table) => table,
        Err(error) => {
            preview.parse_error = Some(error.to_string());
            return preview;
        }
    };
    let describe = |value: &toml::Value| match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                toml::Value::String(text) => text.clone(),
                toml::Value::Table(table) => table
                    .get("run")
                    .or_else(|| table.get("script"))
                    .or_else(|| table.get("path"))
                    .map(|value| value.to_string().trim_matches('"').to_owned())
                    .unwrap_or_else(|| item.to_string()),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ; "),
        toml::Value::Table(table) => table
            .get("version")
            .or_else(|| table.get("run"))
            .map(|value| value.to_string().trim_matches('"').to_owned())
            .unwrap_or_else(|| value.to_string()),
        other => other.to_string(),
    };

    if let Some(toml::Value::Table(tools)) = table.get("tools") {
        preview.tools = tools
            .iter()
            .map(|(name, value)| (name.clone(), describe(value)))
            .collect();
    }
    if let Some(toml::Value::Table(env)) = table.get("env") {
        for (name, value) in env {
            if name == "_" {
                if let toml::Value::Table(directives) = value {
                    for (kind, target) in directives {
                        preview
                            .env_directives
                            .push(format!("_.{kind} = {}", describe(target)));
                    }
                }
            } else {
                preview.env_names.push(name.clone());
            }
        }
    }
    if let Some(toml::Value::Table(hooks)) = table.get("hooks") {
        preview.hooks = hooks
            .iter()
            .map(|(event, value)| (event.clone(), describe(value)))
            .collect();
    }
    if let Some(toml::Value::Table(tasks)) = table.get("tasks") {
        preview.tasks = tasks
            .iter()
            .map(|(name, value)| {
                let run = match value {
                    toml::Value::Table(task) => task.get("run").map(&describe).unwrap_or_default(),
                    other => describe(other),
                };
                (name.clone(), run)
            })
            .collect();
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_show_parses_both_states() {
        assert_eq!(
            parse_trust_show("/Users/demo/code/app: trusted\n"),
            Some(TrustStatus {
                path: "/Users/demo/code/app".into(),
                trusted: true
            })
        );
        assert!(
            !parse_trust_show("/Users/demo/code/infra: untrusted")
                .unwrap()
                .trusted
        );
        assert_eq!(parse_trust_show("nonsense"), None);
    }

    #[test]
    fn untrusted_errors_are_recognized() {
        assert!(is_untrusted_error(
            "mise ERROR Config files in /x/mise.toml are not trusted.\nTrust them with `mise trust`."
        ));
        assert!(!is_untrusted_error("mise ERROR tool not found"));
    }

    #[test]
    fn secret_looking_names_are_masked() {
        for name in [
            "API_TOKEN",
            "db_password",
            "STRIPE_SECRET",
            "AWS_ACCESS_KEY",
            "GH_API_KEY",
        ] {
            assert!(looks_secret(name), "{name}");
        }
        for name in ["APP_ENV", "NODE_ENV", "KEYBOARD_LAYOUT", "PORT"] {
            assert!(!looks_secret(name), "{name}");
        }
    }

    #[test]
    fn preview_shows_what_trusting_would_allow_without_running_it() {
        let preview = preview_config(
            Path::new("/p/mise.toml"),
            r#"
                [tools]
                node = "22"
                python = { version = "3.13" }
                [env]
                API_TOKEN = "s3cret"
                _.file = ".env"
                _.source = "./scripts/setup.sh"
                [hooks]
                enter = "echo hi"
                postinstall = ["npm ci", { run = "make" }]
                [tasks.test]
                run = "cargo test"
                [tasks]
                lint = "eslint ."
            "#,
        );
        assert_eq!(
            preview.tools,
            [
                ("node".into(), "22".into()),
                ("python".into(), "3.13".into())
            ]
        );
        assert_eq!(preview.env_names, ["API_TOKEN"]);
        assert!(
            preview
                .env_directives
                .contains(&"_.source = ./scripts/setup.sh".to_owned())
        );
        assert!(preview.hooks.contains(&("enter".into(), "echo hi".into())));
        assert!(
            preview
                .hooks
                .contains(&("postinstall".into(), "npm ci ; make".into()))
        );
        assert!(
            preview
                .tasks
                .contains(&("test".into(), "cargo test".into()))
        );
        assert!(preview.tasks.contains(&("lint".into(), "eslint .".into())));
        assert!(
            !format!("{preview:?}").contains("s3cret"),
            "values must not leak"
        );
        assert_eq!(preview.parse_error, None);
    }

    #[test]
    fn unparseable_configs_report_why() {
        let preview = preview_config(Path::new("/p/mise.toml"), "[tools\nnode=");
        assert!(preview.parse_error.is_some());
    }
}
