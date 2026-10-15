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

/// Environment names a project has config for (`mise.<env>.toml` and the
/// other config locations), for `MISE_ENV`. `local` files aren't environments.
pub fn project_environments(project: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut scan = |directory: &Path, prefixes: &[&str]| {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(file) = entry.file_name().into_string() else {
                continue;
            };
            for prefix in prefixes {
                let name = file
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.strip_suffix(".toml"))
                    // `mise.staging.local.toml` is staging's local override.
                    .map(|rest| rest.trim_end_matches(".local"));
                if let Some(name) = name
                    && !name.is_empty()
                    && name != "local"
                    && name.chars().all(|character| {
                        character.is_ascii_alphanumeric() || "-_".contains(character)
                    })
                {
                    names.push(name.to_owned());
                }
            }
        }
    };
    scan(project, &["mise.", ".mise."]);
    scan(&project.join("mise"), &["config."]);
    scan(&project.join(".mise"), &["config."]);
    scan(&project.join(".config/mise"), &["config."]);
    names.sort();
    names.dedup();
    names
}

/// A version file another tool uses (`.nvmrc`, `.python-version`, …), which
/// mise reads only for tools listed in `idiomatic_version_file_enable_tools`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdiomaticFile {
    pub file: String,
    pub tool: String,
    pub version: String,
}

/// One-line version files (the first line that isn't a comment), by tool,
/// as mise's core tools read them (<https://mise.jdx.dev/core-tools.html>),
/// plus `.terraform-version` from the registry.
const VERSION_FILES: &[(&str, &str)] = &[
    (".nvmrc", "node"),
    (".node-version", "node"),
    (".python-version", "python"),
    (".python-versions", "python"),
    (".ruby-version", "ruby"),
    (".go-version", "go"),
    (".java-version", "java"),
    (".bun-version", "bun"),
    (".deno-version", "deno"),
    (".exenv-version", "elixir"),
    (".swift-version", "swift"),
    (".zig-version", "zig"),
    (".terraform-version", "terraform"),
];

/// Idiomatic version files in a project folder, with the version they ask
/// for: one-line files, and the version mise reads from `package.json`,
/// `go.mod`/`go.work`, `Gemfile`, `rust-toolchain.toml`, `global.json`, and
/// `.sdkmanrc`.
pub fn idiomatic_files(project: &Path) -> Vec<IdiomaticFile> {
    let read = |name: &str| fs::read_to_string(project.join(name)).ok();
    let mut found: Vec<IdiomaticFile> = Vec::new();
    let mut push = |file: &str, tool: &str, version: String| {
        found.push(IdiomaticFile {
            file: file.into(),
            tool: tool.into(),
            version,
        });
    };
    for (file, tool) in VERSION_FILES {
        if let Some(version) = read(file).and_then(|text| {
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
        }) {
            push(file, tool, version);
        }
    }
    // package.json: `devEngines.runtime` (object, or the first of an array)
    // for node, bun, or deno; Bun also reads `devEngines.packageManager` and
    // `packageManager` (`bun@1.3.14`). `engines` isn't used.
    if let Some(json) =
        read("package.json").and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    {
        let entry = |value: Option<&serde_json::Value>| -> Option<(String, String)> {
            let value = value?;
            let value = value
                .as_array()
                .and_then(|array| array.first())
                .unwrap_or(value);
            Some((
                value.get("name")?.as_str()?.to_owned(),
                value.get("version")?.as_str()?.to_owned(),
            ))
        };
        let mut seen = Vec::new();
        if let Some((name, version)) = entry(json.pointer("/devEngines/runtime"))
            && ["node", "bun", "deno"].contains(&name.as_str())
        {
            seen.push(name.clone());
            push("package.json", &name, version);
        }
        if !seen.iter().any(|name| name == "bun") {
            let bun = entry(json.pointer("/devEngines/packageManager"))
                .filter(|(name, _)| name == "bun")
                .map(|(_, version)| version)
                .or_else(|| {
                    json.get("packageManager")?
                        .as_str()?
                        .strip_prefix("bun@")
                        .map(|version| version.split('+').next().unwrap_or(version).to_owned())
                });
            if let Some(version) = bun {
                push("package.json", "bun", version);
            }
        }
    }
    // go.mod / go.work: `toolchain goX.Y.Z`, else `go X.Y.Z` with a patch.
    for file in ["go.mod", "go.work"] {
        let Some(text) = read(file) else { continue };
        let line = |prefix: &str| {
            text.lines()
                .map(str::trim)
                .find_map(|line| line.strip_prefix(prefix).map(|rest| rest.trim().to_owned()))
        };
        let version = line("toolchain go")
            .or_else(|| line("go ").filter(|version| version.matches('.').count() >= 2));
        if let Some(version) = version {
            push(file, "go", version);
            break;
        }
    }
    // Gemfile: `ruby "3.3.0"` (Bundler's `ruby file:` is the .ruby-version above).
    if let Some(version) = read("Gemfile").and_then(|text| {
        text.lines().map(str::trim).find_map(|line| {
            let rest = line.strip_prefix("ruby ")?.trim();
            let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            rest[1..].split(quote).next().map(str::to_owned)
        })
    }) {
        push("Gemfile", "ruby", version);
    }
    // rust-toolchain.toml: `[toolchain] channel`.
    if let Some(channel) = read("rust-toolchain.toml")
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| {
            table
                .get("toolchain")?
                .get("channel")?
                .as_str()
                .map(str::to_owned)
        })
    {
        push("rust-toolchain.toml", "rust", channel);
    }
    // global.json: `sdk.version`.
    if let Some(version) = read("global.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.pointer("/sdk/version")?.as_str().map(str::to_owned))
    {
        push("global.json", "dotnet", version);
    }
    // .sdkmanrc: `java=21.0.9-tem` (mise maps SDKMAN's vendors to its own).
    if let Some(version) = read(".sdkmanrc").and_then(|text| {
        text.lines().map(str::trim).find_map(|line| {
            line.strip_prefix("java=")
                .map(|rest| rest.trim().to_owned())
        })
    }) {
        push(".sdkmanrc", "java", version);
    }
    found
}

/// One tool's pending lockfile change (`mise lock --dry-run --json`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LockChange {
    pub name: String,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub lockfile: Option<String>,
    #[serde(default)]
    pub old_versions: Vec<String>,
    #[serde(default)]
    pub new_versions: Vec<String>,
}

/// One `[vars]` entry from `mise vars ls`: `name  value  source`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarEntry {
    pub name: String,
    pub value: String,
    pub source: Option<String>,
}

/// Parses `mise vars ls --no-header`: the name, padded by at least two
/// spaces; then the value; then the file it comes from as the last word
/// (a path), when there is one.
pub fn parse_vars(output: &str) -> Vec<VarEntry> {
    let looks_like_path = |word: &str| {
        word.starts_with('~') || word.starts_with('/') || word.get(1..3) == Some(":\\")
    };
    output
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once("  ")?;
            let rest = rest.trim();
            let (value, source) = match rest.rsplit_once(' ') {
                Some((value, last)) if looks_like_path(last) => {
                    (value.trim_end(), Some(last.to_owned()))
                }
                _ if looks_like_path(rest) => ("", Some(rest.to_owned())),
                _ => (rest, None),
            };
            Some(VarEntry {
                name: name.trim().to_owned(),
                value: value.to_owned(),
                source,
            })
        })
        .collect()
}

/// A `[vars.<name>]` table that takes a per-machine value: it has a
/// `prompt`, a `default`, or is `required`. Plain `[vars] name = "value"`
/// entries are fixed by the config and aren't listed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarDeclaration {
    pub name: String,
    pub prompt: Option<String>,
    pub default: Option<String>,
    pub required: bool,
    pub file: PathBuf,
}

/// Reads the `[vars]` tables of a project's config files without executing
/// anything.
pub fn declared_vars(project: &Path) -> Vec<VarDeclaration> {
    let mut declarations = Vec::new();
    for name in PROJECT_CONFIG_FILES {
        let file = project.join(name);
        let Ok(table) = fs::read_to_string(&file)
            .ok()
            .map(|contents| contents.parse::<toml::Table>())
            .transpose()
        else {
            continue;
        };
        let Some(vars) = table
            .as_ref()
            .and_then(|table| table.get("vars"))
            .and_then(toml::Value::as_table)
        else {
            continue;
        };
        for (var, value) in vars {
            let Some(spec) = value.as_table() else {
                continue;
            };
            let text = |key: &str| {
                spec.get(key)
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            };
            let required = spec
                .get("required")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let (prompt, default) = (text("prompt"), text("default"));
            if prompt.is_some() || default.is_some() || required {
                declarations.push(VarDeclaration {
                    name: var.clone(),
                    prompt,
                    default,
                    required,
                    file: file.clone(),
                });
            }
        }
    }
    declarations
}

/// The names of a project's secrets and where they come from — never values.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsReport {
    pub source: Option<String>,
    pub keys: Vec<String>,
}

impl SecretsReport {
    /// Reads `mise secrets ls --json` (experimental, so read defensively:
    /// keys may be names or objects with a `name`).
    pub fn from_json(json: &serde_json::Value) -> Self {
        let source = match json.get("source") {
            Some(serde_json::Value::String(text)) => Some(text.clone()),
            Some(serde_json::Value::Null) | None => None,
            Some(other) => Some(other.to_string()),
        };
        let keys = json
            .get("keys")
            .and_then(serde_json::Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(|key| {
                        key.as_str().map(str::to_owned).or_else(|| {
                            key.get("name")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { source, keys }
    }
}

/// Files `mise generate` can add to a project.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Generator {
    GithubAction,
    GitPreCommit,
    Devcontainer,
}

impl Generator {
    pub fn subcommand(self) -> &'static str {
        match self {
            Self::GithubAction => "github-action",
            Self::GitPreCommit => "git-pre-commit",
            Self::Devcontainer => "devcontainer",
        }
    }

    /// Whether it runs a project task (`--task`).
    pub fn takes_task(self) -> bool {
        matches!(self, Self::GithubAction | Self::GitPreCommit)
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("powermise-project-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn environments_come_from_every_config_location() {
        let project = scratch("envs");
        for file in [
            "mise.staging.toml",
            ".mise.production.toml",
            "mise.local.toml",
            "mise.staging.local.toml",
            "mise.toml",
        ] {
            fs::write(project.join(file), "").unwrap();
        }
        fs::create_dir_all(project.join(".config/mise")).unwrap();
        fs::write(project.join(".config/mise/config.test.toml"), "").unwrap();
        assert_eq!(
            project_environments(&project),
            ["production", "staging", "test"]
        );
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn idiomatic_files_read_the_first_version_line() {
        let project = scratch("idiomatic");
        fs::write(project.join(".nvmrc"), "# comment\n22\n").unwrap();
        fs::write(project.join(".python-version"), "3.12.4\n").unwrap();
        let files = idiomatic_files(&project);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].tool, "node");
        assert_eq!(files[0].version, "22");
        assert_eq!(files[1].version, "3.12.4");
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn idiomatic_files_read_versions_inside_project_files() {
        let project = scratch("idiomatic-structured");
        let write = |name: &str, text: &str| fs::write(project.join(name), text).unwrap();
        write(
            "package.json",
            r#"{"devEngines":{"runtime":[{"name":"node","version":"26.10.0"}]},"packageManager":"bun@1.3.14+sha512.abc","engines":{"node":">=20"}}"#,
        );
        write("go.mod", "module x\n\ngo 1.27\n\ntoolchain go1.27.3\n");
        write(
            "Gemfile",
            "source \"https://rubygems.org\"\nruby \"3.4.1\"\n",
        );
        write(
            "rust-toolchain.toml",
            "[toolchain]\nchannel = \"1.90.0\"\ncomponents = [\"clippy\"]\n",
        );
        write(
            "global.json",
            r#"{"sdk":{"version":"8.0.425","rollForward":"disable"}}"#,
        );
        write(".sdkmanrc", "# sdk env\njava=21.0.9-tem\n");
        write(".zig-version", "0.15.1\n");
        let files = idiomatic_files(&project);
        let _ = fs::remove_dir_all(&project);
        let found: Vec<_> = files
            .iter()
            .map(|file| {
                (
                    file.file.as_str(),
                    file.tool.as_str(),
                    file.version.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (".zig-version", "zig", "0.15.1"),
                ("package.json", "node", "26.10.0"),
                ("package.json", "bun", "1.3.14"),
                ("go.mod", "go", "1.27.3"),
                ("Gemfile", "ruby", "3.4.1"),
                ("rust-toolchain.toml", "rust", "1.90.0"),
                ("global.json", "dotnet", "8.0.425"),
                (".sdkmanrc", "java", "21.0.9-tem"),
            ]
        );

        // `go 1.27` without a patch or toolchain isn't a version request.
        let project = scratch("idiomatic-go");
        fs::write(project.join("go.mod"), "module x\n\ngo 1.27\n").unwrap();
        assert!(idiomatic_files(&project).is_empty());
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn declared_vars_are_the_tables_that_take_a_machine_value() {
        let project = scratch("vars");
        fs::write(
            project.join("mise.toml"),
            "[vars]\nregion = \"us-east-1\"\n\n[vars.api_url]\ndefault = \"http://localhost\"\nprompt = \"API URL?\"\n\n[vars.token]\nrequired = true\n",
        )
        .unwrap();
        let vars = declared_vars(&project);
        let names: Vec<_> = vars.iter().map(|var| var.name.as_str()).collect();
        assert_eq!(names, ["api_url", "token"]);
        assert_eq!(vars[0].prompt.as_deref(), Some("API URL?"));
        assert!(vars[1].required);
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn vars_lines_parse() {
        // Captured from mise 2026.10.10.
        let output = "region              us-east-1 ~/m3/mise.toml\n\
                      long_variable_name  x         ~/m3/mise.toml\n\
                      with_space          two words ~/m3/mise.toml\n";
        let vars = parse_vars(output);
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0].name, "region");
        assert_eq!(vars[0].value, "us-east-1");
        assert_eq!(vars[0].source.as_deref(), Some("~/m3/mise.toml"));
        assert_eq!(vars[1].value, "x");
        assert_eq!(vars[2].value, "two words");
    }
}
