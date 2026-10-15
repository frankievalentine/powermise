//! A typed client for the installed `mise` executable. Reads run mise's JSON
//! commands; changes are expressed as [`MiseCommand`]s that render the exact
//! command line shown to people and run with streamed output. powermise never
//! edits mise configuration itself.
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use serde::de::DeserializeOwned;

use crate::{
    MiseAdapterError,
    models::{
        BootstrapPlan, BootstrapStatus, ConfigFile, Doctor, OutdatedReport, RegistryEntry,
        RemoteVersion, Task, ToolInventory, VersionInfo,
    },
    project::{
        EnvReport, TaskDetails, TrustStatus, is_untrusted_error, parse_task_details,
        parse_trust_show,
    },
};

/// The installed mise, run from a fixed working directory so project-local
/// configs only join when powermise asks for a project.
#[derive(Clone, Debug)]
pub struct MiseClient {
    executable: PathBuf,
    /// The directory reads run in when no project is given (the home directory,
    /// so the global view isn't shaped by wherever powermise was launched).
    home: PathBuf,
}

impl MiseClient {
    pub fn new(executable: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            home: home.into(),
        }
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    fn base(&self, cwd: Option<&Path>) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(cwd.unwrap_or(&self.home))
            .stdin(Stdio::null())
            // Plain output: no colors or spinners in captured text.
            .env("NO_COLOR", "1")
            .env("MISE_COLOR", "0")
            // `MISE_YES` makes mise answer its own prompts — including
            // silently trusting an untrusted config on a plain read. Trust is
            // the person's decision, so never inherit it from their shell.
            .env_remove("MISE_YES");
        command
    }

    fn read_json<T: DeserializeOwned>(
        &self,
        arguments: &[&str],
        cwd: Option<&Path>,
    ) -> Result<T, MiseAdapterError> {
        let output = self.base(cwd).args(arguments).output().map_err(|source| {
            MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            }
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if is_untrusted_error(&stderr) {
                return Err(MiseAdapterError::Untrusted { stderr });
            }
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr,
            });
        }
        serde_json::from_slice(&output.stdout).map_err(|source| MiseAdapterError::Json {
            command: format!("mise {}", arguments.join(" ")),
            source,
        })
    }

    /// `mise version --json`.
    pub fn version_info(&self) -> Result<VersionInfo, MiseAdapterError> {
        self.read_json(&["version", "--json"], None)
    }

    /// `mise ls --json`, from the home directory or a project.
    pub fn tools(&self, project: Option<&Path>) -> Result<ToolInventory, MiseAdapterError> {
        self.read_json(&["ls", "--json"], project)
    }

    /// `mise outdated --bump --json`: includes newer versions beyond pins.
    pub fn outdated(&self, project: Option<&Path>) -> Result<OutdatedReport, MiseAdapterError> {
        self.read_json(&["outdated", "--bump", "--json"], project)
    }

    /// `mise registry --json`: every tool mise knows how to install.
    pub fn registry(&self) -> Result<Vec<RegistryEntry>, MiseAdapterError> {
        self.read_json(&["registry", "--json"], None)
    }

    /// `mise ls-remote --json <tool>`: versions available to install.
    pub fn remote_versions(&self, tool: &str) -> Result<Vec<RemoteVersion>, MiseAdapterError> {
        validate_argument(tool)?;
        self.read_json(&["ls-remote", "--json", tool], None)
    }

    /// `mise config ls --json` for a project (or the global view).
    pub fn config_files(
        &self,
        project: Option<&Path>,
    ) -> Result<Vec<ConfigFile>, MiseAdapterError> {
        self.read_json(&["config", "ls", "--json"], project)
    }

    /// `mise tasks ls --json` for a project.
    pub fn tasks(&self, project: &Path) -> Result<Vec<Task>, MiseAdapterError> {
        self.read_json(&["tasks", "ls", "--json"], Some(project))
    }

    /// `mise env --json` for a project.
    pub fn env(
        &self,
        project: &Path,
    ) -> Result<std::collections::BTreeMap<String, String>, MiseAdapterError> {
        self.read_json(&["env", "--json"], Some(project))
    }

    /// `mise trust --show` for a project: never fails on an untrusted config.
    pub fn trust_status(&self, project: &Path) -> Result<TrustStatus, MiseAdapterError> {
        let output = self
            .base(Some(project))
            .args(["trust", "--show"])
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        // No config in the folder at all reads as trusted: there's nothing to trust.
        Ok(parse_trust_show(&stdout).unwrap_or(TrustStatus {
            path: project.to_path_buf(),
            trusted: true,
        }))
    }

    /// `mise env --json-extended`: each variable with its value and source file.
    pub fn env_extended(&self, project: &Path) -> Result<EnvReport, MiseAdapterError> {
        self.read_json(&["env", "--json-extended"], Some(project))
    }

    /// Names of variables mise marks `redact = true` (`mise env --json --redacted`).
    pub fn redacted_env_names(&self, project: &Path) -> Result<Vec<String>, MiseAdapterError> {
        let redacted: std::collections::BTreeMap<String, serde_json::Value> =
            self.read_json(&["env", "--json", "--redacted"], Some(project))?;
        Ok(redacted.into_keys().collect())
    }

    /// `mise tasks info --json <task>`, reduced to what a run form needs.
    pub fn task_details(
        &self,
        project: &Path,
        task: &str,
    ) -> Result<TaskDetails, MiseAdapterError> {
        validate_argument(task)?;
        let json: serde_json::Value =
            self.read_json(&["tasks", "info", "--json", task], Some(project))?;
        Ok(parse_task_details(&json))
    }

    /// `mise bootstrap plan --json`: everything applying the global config
    /// would change — including parts that aren't from powermise.
    pub fn bootstrap_plan(&self) -> Result<BootstrapPlan, MiseAdapterError> {
        self.read_json(&["bootstrap", "plan", "--json"], None)
    }

    /// `mise bootstrap status --json`: the state of every declared package and tool.
    pub fn bootstrap_status(&self) -> Result<BootstrapStatus, MiseAdapterError> {
        self.read_json(&["bootstrap", "status", "--json"], None)
    }

    /// `mise doctor --json`.
    pub fn doctor(&self) -> Result<Doctor, MiseAdapterError> {
        self.read_json(&["doctor", "--json"], None)
    }

    /// Runs a change, streaming each output line to `on_line` until it exits
    /// or `cancel` is set (which kills it).
    pub fn run(
        &self,
        command: &MiseCommand,
        cancel: &AtomicBool,
        mut on_line: impl FnMut(OutputLine),
    ) -> Result<RunOutcome, MiseAdapterError> {
        let mut process = self.base(command.cwd.as_deref());
        process
            .args(&command.arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own process group, so cancelling also stops the downloads and
        // extractors mise starts.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut process, 0);
        let mut child = process
            .spawn()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;

        let (sender, receiver) = mpsc::channel();
        let readers = [
            child
                .stdout
                .take()
                .map(|pipe| forward(pipe, Stream::Stdout, sender.clone())),
            child
                .stderr
                .take()
                .map(|pipe| forward(pipe, Stream::Stderr, sender.clone())),
        ];
        drop(sender);

        let mut cancelled = false;
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => on_line(line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                // Both pipes closed: the process has exited.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    for reader in readers.into_iter().flatten() {
                        let _ = reader.join();
                    }
                    break;
                }
            }
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                terminate(&mut child);
                // Don't wait on the pipes: a grandchild can hold them open.
                // The detached reader threads end when it exits.
                break;
            }
        }
        let status = child.wait().map_err(|source| MiseAdapterError::Execute {
            executable: self.executable.clone(),
            source,
        })?;
        Ok(RunOutcome {
            success: status.success() && !cancelled,
            code: status.code(),
            cancelled,
        })
    }
}

/// Stops a running command and everything it started.
fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child leads its own process group (see `run`).
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

fn forward(
    pipe: impl Read + Send + 'static,
    stream: Stream,
    sender: mpsc::Sender<OutputLine>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for text in BufReader::new(pipe).lines().map_while(Result::ok) {
            if sender.send(OutputLine { stream, text }).is_err() {
                break;
            }
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// One line of a running command's output.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct OutputLine {
    pub stream: Stream,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RunOutcome {
    pub success: bool,
    pub code: Option<i32>,
    pub cancelled: bool,
}

/// A mise command that changes state, built from validated parts so its
/// rendered form is exactly what runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseCommand {
    arguments: Vec<String>,
    cwd: Option<PathBuf>,
    /// Arguments of the form `KEY=value` whose value is hidden when the
    /// command is displayed (environment values can be secrets).
    masked: Vec<usize>,
}

impl MiseCommand {
    fn new(arguments: Vec<String>) -> Self {
        Self {
            arguments,
            cwd: None,
            masked: Vec::new(),
        }
    }

    /// Runs the command inside a project directory.
    pub fn in_project(mut self, project: impl Into<PathBuf>) -> Self {
        self.cwd = Some(project.into());
        self
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// The command line as a person would type it.
    pub fn display(&self) -> String {
        let mut line = String::from("mise");
        for (index, argument) in self.arguments.iter().enumerate() {
            line.push(' ');
            if self.masked.contains(&index) {
                let key = argument
                    .split_once('=')
                    .map_or(argument.as_str(), |(key, _)| key);
                line.push_str(&format!("{key}=•••"));
            } else if argument.contains(char::is_whitespace) || argument.is_empty() {
                line.push_str(&format!("'{argument}'"));
            } else {
                line.push_str(argument);
            }
        }
        if let Some(cwd) = &self.cwd {
            format!("cd {} && {line}", shell_quote(cwd))
        } else {
            line
        }
    }

    /// `mise use -g <tool>@<version>`: install and set as the global default.
    pub fn use_global(tool: &str, version: &str) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec![
            "use".into(),
            "-g".into(),
            tool_spec(tool, version)?,
        ]))
    }

    /// `mise unuse -g <tool>`: remove from the global config (and prune it).
    pub fn unuse_global(tool: &str) -> Result<Self, MiseAdapterError> {
        validate_argument(tool)?;
        Ok(Self::new(vec!["unuse".into(), "-g".into(), tool.into()]))
    }

    /// `mise uninstall <tool>@<version>`: remove one installed version.
    pub fn uninstall(tool: &str, version: &str) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec![
            "uninstall".into(),
            tool_spec(tool, version)?,
        ]))
    }

    /// `mise upgrade --bump [tool]`: upgrade one tool, or all when `None`.
    /// `--bump` moves past pinned versions and updates the config to match.
    pub fn upgrade(tool: Option<&str>) -> Result<Self, MiseAdapterError> {
        let mut arguments = vec!["upgrade".into(), "--bump".into()];
        if let Some(tool) = tool {
            validate_argument(tool)?;
            arguments.push(tool.into());
        }
        Ok(Self::new(arguments))
    }

    /// `mise install`: install everything the current config requests.
    pub fn install_all() -> Self {
        Self::new(vec!["install".into()])
    }

    /// `mise use <tool>@<version>` in a project: adds it to the project's config.
    pub fn use_in_project(
        tool: &str,
        version: &str,
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec!["use".into(), tool_spec(tool, version)?]).in_project(project))
    }

    /// `mise trust` in a project, after the person reviewed what it allows.
    pub fn trust(project: impl Into<PathBuf>) -> Self {
        Self::new(vec!["trust".into()]).in_project(project)
    }

    /// `mise run <task> -- <args…>` in a project. Arguments follow `--` so one
    /// like `--force` always reaches the task, never `mise run` itself.
    pub fn run_task(
        task: &str,
        args: &[String],
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        validate_argument(task)?;
        let mut arguments = vec!["run".into(), task.into()];
        if !args.is_empty() {
            arguments.push("--".into());
            arguments.extend(args.iter().cloned());
        }
        Ok(Self::new(arguments).in_project(project))
    }

    /// `mise set [--file <file>] KEY=value` in a project; the value is hidden
    /// when displayed. Without `file`, mise writes to the project's `mise.toml`.
    pub fn set_env(
        key: &str,
        value: &str,
        file: Option<&Path>,
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        validate_env_key(key)?;
        let project = project.into();
        let mut arguments = vec!["set".into()];
        if let Some(file) = file {
            arguments.extend(["--file".into(), project_file(&project, file)?]);
        }
        arguments.push(format!("{key}={value}"));
        let masked = arguments.len() - 1;
        let mut command = Self::new(arguments).in_project(project);
        command.masked.push(masked);
        Ok(command)
    }

    /// `mise unset [--file <file>] KEY` in a project: removes it from the file
    /// that defines it (default `mise.toml`).
    pub fn unset_env(
        key: &str,
        file: Option<&Path>,
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        validate_env_key(key)?;
        let project = project.into();
        let mut arguments = vec!["unset".into()];
        if let Some(file) = file {
            arguments.extend(["--file".into(), project_file(&project, file)?]);
        }
        arguments.push(key.into());
        Ok(Self::new(arguments).in_project(project))
    }

    /// `mise bootstrap --yes --only packages,tools`: installs what the config
    /// declares. Limited to packages and tools — what an AI setup contains — so
    /// applying one never touches dotfiles, services, or other bootstrap parts.
    /// `--yes` because the person already reviewed the plan in the app.
    pub fn bootstrap_packages_and_tools() -> Self {
        Self::new(vec![
            "bootstrap".into(),
            "--yes".into(),
            "--only".into(),
            "packages,tools".into(),
        ])
    }

    /// `mise prune --tools`: deletes tool versions no config uses anymore.
    pub fn prune_tools() -> Self {
        Self::new(vec!["prune".into(), "--tools".into()])
    }

    /// `mise self-update --yes`: update mise itself (the person already
    /// confirmed in the app).
    pub fn self_update() -> Self {
        Self::new(vec!["self-update".into(), "--yes".into()])
    }
}

/// A config file powermise may edit through mise: a `.toml` file inside the
/// project. Returned relative to the project, as mise resolves `--file`.
fn project_file(project: &Path, file: &Path) -> Result<String, MiseAdapterError> {
    let relative = file.strip_prefix(project).unwrap_or(file);
    let inside = relative.is_relative()
        && !relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir));
    if inside
        && relative
            .extension()
            .is_some_and(|extension| extension == "toml")
    {
        Ok(relative.to_string_lossy().into_owned())
    } else {
        Err(MiseAdapterError::InvalidArgument {
            value: file.display().to_string(),
        })
    }
}

/// Environment variable names: letters, digits, and underscores, not
/// starting with a digit.
fn validate_env_key(key: &str) -> Result<(), MiseAdapterError> {
    let valid = key
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidArgument { value: key.into() })
    }
}

fn tool_spec(tool: &str, version: &str) -> Result<String, MiseAdapterError> {
    validate_argument(tool)?;
    validate_argument(version)?;
    Ok(format!("{tool}@{version}"))
}

/// Tool names and versions come from people and from the registry. They are
/// passed as discrete arguments (never through a shell), so the only risk is
/// one being read as a flag.
fn validate_argument(value: &str) -> Result<(), MiseAdapterError> {
    let valid = !value.is_empty()
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control());
    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidArgument {
            value: value.into(),
        })
    }
}

fn shell_quote(path: &Path) -> String {
    let text = path.display().to_string();
    if text
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-~".contains(character))
    {
        text
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_render_exactly_what_runs() {
        let command = MiseCommand::use_global("node", "22.11.0").unwrap();
        assert_eq!(command.arguments(), ["use", "-g", "node@22.11.0"]);
        assert_eq!(command.display(), "mise use -g node@22.11.0");
        assert_eq!(
            MiseCommand::upgrade(None).unwrap().display(),
            "mise upgrade --bump"
        );
        assert_eq!(
            MiseCommand::upgrade(Some("jq")).unwrap().display(),
            "mise upgrade --bump jq"
        );
        assert_eq!(
            MiseCommand::uninstall("jq", "1.7.1").unwrap().display(),
            "mise uninstall jq@1.7.1"
        );
        assert_eq!(
            MiseCommand::install_all()
                .in_project("/Users/demo/My Project")
                .display(),
            "cd '/Users/demo/My Project' && mise install"
        );
    }

    #[test]
    fn project_commands_render_and_hide_secrets() {
        let project = PathBuf::from("/Users/demo/code/app");
        assert_eq!(
            MiseCommand::run_task("deploy", &["staging".into(), "--dry-run".into()], &project)
                .unwrap()
                .display(),
            "cd /Users/demo/code/app && mise run deploy -- staging --dry-run"
        );
        assert_eq!(
            MiseCommand::run_task("test", &[], &project)
                .unwrap()
                .arguments(),
            ["run", "test"]
        );
        let set = MiseCommand::set_env("API_TOKEN", "s3cret", None, &project).unwrap();
        assert_eq!(set.arguments(), ["set", "API_TOKEN=s3cret"]);
        assert_eq!(
            set.display(),
            "cd /Users/demo/code/app && mise set API_TOKEN=•••"
        );
        let local_file = Path::new("/Users/demo/code/app/mise.local.toml");
        let local =
            MiseCommand::set_env("API_TOKEN", "s3cret", Some(local_file), &project).unwrap();
        assert_eq!(
            local.arguments(),
            ["set", "--file", "mise.local.toml", "API_TOKEN=s3cret"]
        );
        assert_eq!(
            local.display(),
            "cd /Users/demo/code/app && mise set --file mise.local.toml API_TOKEN=•••"
        );
        assert_eq!(
            MiseCommand::unset_env("DATABASE_URL", Some(local_file), &project)
                .unwrap()
                .arguments(),
            ["unset", "--file", "mise.local.toml", "DATABASE_URL"]
        );
        // Only .toml files inside the project.
        for outside in [
            "/Users/demo/.config/mise/config.toml",
            "../other/mise.toml",
            "/Users/demo/code/app/.env",
        ] {
            assert!(
                MiseCommand::unset_env("X", Some(Path::new(outside)), &project).is_err(),
                "{outside}"
            );
        }
        assert!(MiseCommand::set_env("BAD KEY", "x", None, &project).is_err());
        assert!(MiseCommand::set_env("1ABC", "x", None, &project).is_err());
        assert_eq!(
            MiseCommand::trust(&project).display(),
            "cd /Users/demo/code/app && mise trust"
        );
        assert_eq!(
            MiseCommand::use_in_project("node", "22", &project)
                .unwrap()
                .display(),
            "cd /Users/demo/code/app && mise use node@22"
        );
    }

    #[test]
    fn commands_never_inherit_mise_yes() {
        let client = MiseClient::new("mise", std::env::temp_dir());
        let command = client.base(None);
        let environment = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            environment.get(std::ffi::OsStr::new("MISE_YES")),
            Some(&None),
            "MISE_YES must be removed so reads can't auto-trust configs"
        );
    }

    #[test]
    fn arguments_that_look_like_flags_are_rejected() {
        for value in ["--force", "-g", "", "node 22", "a\nb"] {
            assert!(matches!(
                MiseCommand::unuse_global(value),
                Err(MiseAdapterError::InvalidArgument { .. })
            ));
        }
        assert!(MiseCommand::use_global("npm:@openai/codex", "0.51.0").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn run_streams_output_and_reports_exit() {
        // `sh` stands in for mise: the client only cares about process I/O.
        let client = MiseClient::new("sh", std::env::temp_dir());
        let command = MiseCommand::new(vec!["-c".into(), "echo one; echo two >&2; exit 3".into()]);
        let mut lines = Vec::new();
        let outcome = client
            .run(&command, &AtomicBool::new(false), |line| lines.push(line))
            .unwrap();
        assert_eq!(outcome.code, Some(3));
        assert!(!outcome.success);
        assert!(lines.contains(&OutputLine {
            stream: Stream::Stdout,
            text: "one".into()
        }));
        assert!(lines.contains(&OutputLine {
            stream: Stream::Stderr,
            text: "two".into()
        }));
    }

    #[cfg(unix)]
    #[test]
    fn run_cancels_by_killing_the_process() {
        let client = MiseClient::new("sh", std::env::temp_dir());
        let command = MiseCommand::new(vec!["-c".into(), "echo started; sleep 30".into()]);
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        let outcome = client
            .run(&command, &cancel, |_| cancel.store(true, Ordering::Relaxed))
            .unwrap();
        assert!(outcome.cancelled);
        assert!(!outcome.success);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
