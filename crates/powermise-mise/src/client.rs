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
    machine::{
        DotfileCheckpoint, DotfilesHistory, ImportSource, MachineSection, Plugin, Setting,
        TokenStatus, ToolAlias, parse_checkpoints, parse_machine_status, parse_plugins,
        parse_settings, parse_token_status, parse_tool_aliases,
    },
    models::{
        BootstrapPlan, BootstrapStatus, ConfigFile, Doctor, OutdatedReport, RegistryEntry,
        RemoteVersion, Task, ToolInfo, ToolInventory, VersionInfo,
    },
    project::SecretsReport,
    project::{
        EnvReport, Generator, LockChange, TaskDetails, TrustStatus, VarEntry, is_untrusted_error,
        parse_task_details, parse_trust_show, parse_vars,
    },
};

/// The installed mise, run from a fixed working directory so project-local
/// configs only join when powermise asks for a project.
#[derive(Clone, Debug)]
pub struct MiseClient {
    executable: PathBuf,
    /// `MISE_ENV` for reads, to preview an environment's config.
    environment: Option<String>,
    /// The directory reads run in when no project is given (the home directory,
    /// so the global view isn't shaped by wherever powermise was launched).
    home: PathBuf,
}

impl MiseClient {
    pub fn new(executable: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            environment: None,
            home: home.into(),
        }
    }

    /// The same mise, reading as if `MISE_ENV=<environment>` were set — to
    /// preview what an environment's config changes. Names are letters,
    /// digits, `-`, and `_`.
    pub fn with_environment(mut self, environment: Option<&str>) -> Result<Self, MiseAdapterError> {
        if let Some(name) = environment
            && (name.is_empty()
                || !name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character)))
        {
            return Err(MiseAdapterError::InvalidArgument { value: name.into() });
        }
        self.environment = environment.map(str::to_owned);
        Ok(self)
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
        if let Some(environment) = &self.environment {
            command.env("MISE_ENV", environment);
        }
        command
    }

    fn read_text(
        &self,
        arguments: &[&str],
        cwd: Option<&Path>,
    ) -> Result<String, MiseAdapterError> {
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
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// `mise lock --dry-run --json` in a project: what `mise lock` would
    /// change in its lockfile. Empty means the lockfile is current.
    pub fn lock_changes(&self, project: &Path) -> Result<Vec<LockChange>, MiseAdapterError> {
        self.read_json(&["lock", "--dry-run", "--json"], Some(project))
    }

    /// `mise secrets ls --json` in a project (experimental, 2026.10): where
    /// secrets come from and their names — never their values.
    pub fn secrets(&self, project: &Path) -> Result<SecretsReport, MiseAdapterError> {
        let json: serde_json::Value =
            self.read_json(&["secrets", "ls", "--json"], Some(project))?;
        Ok(SecretsReport::from_json(&json))
    }

    /// `mise vars ls --no-header` in a project (2026.10.6+).
    pub fn vars(&self, project: &Path) -> Result<Vec<VarEntry>, MiseAdapterError> {
        Ok(parse_vars(&self.read_text(
            &["vars", "ls", "--no-header"],
            Some(project),
        )?))
    }

    /// `mise generate <kind>` without `--write`: the file it would add.
    pub fn generate_preview(
        &self,
        project: &Path,
        generator: Generator,
        task: Option<&str>,
    ) -> Result<String, MiseAdapterError> {
        let mut arguments = vec!["generate", generator.subcommand()];
        if let (true, Some(task)) = (generator.takes_task(), task) {
            validate_argument(task)?;
            arguments.extend(["--task", task]);
        }
        self.read_text(&arguments, Some(project))
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

    /// `mise outdated --global --bump --json` (2026.10.7+): only tools the
    /// global config asks for, ignoring any project config in the home folder.
    pub fn outdated_global(&self) -> Result<OutdatedReport, MiseAdapterError> {
        self.read_json(&["outdated", "--global", "--bump", "--json"], None)
    }

    /// `mise outdated --local --bump --json` in a project: only the tools the
    /// project's own config files ask for.
    pub fn outdated_local(&self, project: &Path) -> Result<OutdatedReport, MiseAdapterError> {
        self.read_json(&["outdated", "--local", "--bump", "--json"], Some(project))
    }

    /// `mise tool --json <tool>`: backend, options, and download verification.
    pub fn tool_info(&self, tool: &str) -> Result<ToolInfo, MiseAdapterError> {
        validate_argument(tool)?;
        self.read_json(&["tool", "--json", tool], None)
    }

    /// `mise backends ls`: the backends this mise can install from (one per line).
    pub fn backends(&self) -> Result<Vec<String>, MiseAdapterError> {
        let output = self
            .base(None)
            .args(["backends", "ls"])
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(MiseAdapterError::Failed {
                executable: self.executable.clone(),
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect())
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

    /// `mise bootstrap status --json`, flattened into sections.
    pub fn machine_status(&self) -> Result<Vec<MachineSection>, MiseAdapterError> {
        let json: serde_json::Value = self.read_json(&["bootstrap", "status", "--json"], None)?;
        Ok(parse_machine_status(&json))
    }

    /// The checkpoint history from `mise dotfiles status --json` (2026.10+).
    pub fn dotfiles_history(&self) -> Result<DotfilesHistory, MiseAdapterError> {
        let json: serde_json::Value = self.read_json(&["dotfiles", "status", "--json"], None)?;
        Ok(json
            .get("history")
            .cloned()
            .and_then(|history| serde_json::from_value(history).ok())
            .unwrap_or_default())
    }

    /// `mise x -- <cli> plugin list --json --available` for Claude Code or
    /// Codex: `(installed, available)`.
    pub fn agent_plugins(
        &self,
        cli: crate::AgentCli,
    ) -> Result<(Vec<crate::AgentPlugin>, Vec<crate::AgentPlugin>), MiseAdapterError> {
        let json: serde_json::Value = self.read_json(
            &[
                "x",
                "--",
                cli.program(),
                "plugin",
                "list",
                "--json",
                "--available",
            ],
            None,
        )?;
        Ok(crate::parse_plugin_list(&json))
    }

    /// Skills that active tools ship (`mise skills ls --json`, 2026.10+):
    /// `(skill, tool, version)`.
    pub fn tool_skills(&self) -> Result<Vec<(String, String, String)>, MiseAdapterError> {
        let json: serde_json::Value = self.read_json(&["skills", "ls", "--json"], None)?;
        let text = |entry: &serde_json::Value, keys: &[&str]| {
            keys.iter()
                .find_map(|key| entry.get(*key)?.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        Ok(json
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                (
                    text(entry, &["skill", "name"]),
                    text(entry, &["tool"]),
                    text(entry, &["version"]),
                )
            })
            .collect())
    }

    /// Each `[dotfiles]` edit entry and its state (`applied`, `differs`, …)
    /// from `mise dotfiles status --json`: `(target, edit, state)`, such as
    /// `("~/.claude.json", "merge:powermise-work", "applied")`.
    pub fn dotfile_edits(&self) -> Result<Vec<(String, String, String)>, MiseAdapterError> {
        let json: serde_json::Value = self.read_json(&["dotfiles", "status", "--json"], None)?;
        let text = |entry: &serde_json::Value, key: &str| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(json
            .get("edits")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .map(|entry| {
                (
                    text(entry, "path"),
                    text(entry, "edit"),
                    text(entry, "state"),
                )
            })
            .collect())
    }

    /// The repository dotfile checkpoints sync with (`mise dotfiles origin`,
    /// 2026.10+), or `None` when none is connected.
    pub fn dotfiles_origin(&self) -> Result<Option<String>, MiseAdapterError> {
        let output = self.read_text(&["dotfiles", "origin"], None)?;
        let line = output
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default();
        Ok((!line.is_empty() && !line.starts_with("no setup repository")).then(|| line.to_owned()))
    }

    /// The newest `limit` dotfile checkpoints (`mise dotfiles history --json`, 2026.10+).
    pub fn dotfile_checkpoints(
        &self,
        limit: u32,
    ) -> Result<Vec<DotfileCheckpoint>, MiseAdapterError> {
        let limit = limit.to_string();
        let json: serde_json::Value =
            self.read_json(&["dotfiles", "history", "--json", "--limit", &limit], None)?;
        Ok(parse_checkpoints(&json))
    }

    /// Version aliases mise knows for `tool` (built in and your own).
    pub fn tool_aliases(&self, tool: &str) -> Result<Vec<ToolAlias>, MiseAdapterError> {
        Ok(
            parse_tool_aliases(&self.read_text(&["tool-alias", "ls", "--no-header"], None)?)
                .into_iter()
                .filter(|alias| alias.tool == tool)
                .collect(),
        )
    }

    /// Installed legacy asdf/vfox plugins (`mise plugins ls --urls`).
    pub fn plugins(&self) -> Result<Vec<Plugin>, MiseAdapterError> {
        Ok(parse_plugins(
            &self.read_text(&["plugins", "ls", "--urls"], None)?,
        ))
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
        on_line: impl FnMut(OutputLine),
    ) -> Result<RunOutcome, MiseAdapterError> {
        let mut process = self.base(command.cwd.as_deref());
        process.args(&command.arguments);
        process.envs(command.env.iter().map(|(key, value)| (key, value)));
        stream(process, &self.executable, cancel, on_line)
    }

    /// `mise settings ls --all --json-extended`: every setting, flattened,
    /// with its effective value and, when set, the file it comes from.
    pub fn settings(&self) -> Result<Vec<Setting>, MiseAdapterError> {
        let json: serde_json::Value =
            self.read_json(&["settings", "ls", "--all", "--json-extended"], None)?;
        Ok(parse_settings(&json))
    }

    /// `mise ls --prunable --json`: installed versions no config uses, which
    /// `mise prune --tools` would delete.
    pub fn prunable(&self) -> Result<ToolInventory, MiseAdapterError> {
        self.read_json(&["ls", "--prunable", "--json"], None)
    }

    /// `mise token github`: whether mise has a GitHub token, masked, and
    /// where it comes from. Never asks for the unmasked token.
    pub fn github_token(&self) -> Result<TokenStatus, MiseAdapterError> {
        let output = self
            .base(None)
            .args(["token", "github"])
            .output()
            .map_err(|source| MiseAdapterError::Execute {
                executable: self.executable.clone(),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_token_status(&stdout).ok_or_else(|| MiseAdapterError::Failed {
            executable: self.executable.clone(),
            status: output.status.code(),
            stdout: stdout.into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Runs a program that isn't mise (a package manager installing mise
/// itself), with the same streaming and cancellation as mise commands.
pub fn run_program(
    program: &Path,
    arguments: &[String],
    cwd: &Path,
    cancel: &AtomicBool,
    on_line: impl FnMut(OutputLine),
) -> Result<RunOutcome, MiseAdapterError> {
    let mut process = Command::new(program);
    process
        .args(arguments)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .env("NO_COLOR", "1")
        // Homebrew: don't spend minutes updating every formula first.
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env_remove("MISE_YES");
    stream(process, program, cancel, on_line)
}

/// Spawns `process`, streams its output lines, and kills it (and everything
/// it started) when `cancel` is set.
fn stream(
    mut process: Command,
    executable: &Path,
    cancel: &AtomicBool,
    mut on_line: impl FnMut(OutputLine),
) -> Result<RunOutcome, MiseAdapterError> {
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Its own process group, so cancelling also stops the downloads and
    // extractors mise starts.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut process, 0);
    let mut child = process
        .spawn()
        .map_err(|source| MiseAdapterError::Execute {
            executable: executable.to_path_buf(),
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
        executable: executable.to_path_buf(),
        source,
    })?;
    Ok(RunOutcome {
        success: status.success() && !cancelled,
        code: status.code(),
        cancelled,
    })
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Environment variables for the command (shown before it).
    env: Vec<(String, String)>,
}

impl MiseCommand {
    fn new(arguments: Vec<String>) -> Self {
        Self {
            arguments,
            cwd: None,
            masked: Vec::new(),
            env: Vec::new(),
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
        let mut line: String = self
            .env
            .iter()
            .map(|(key, value)| format!("{key}={value} "))
            .collect();
        line.push_str("mise");
        for (index, argument) in self.arguments.iter().enumerate() {
            line.push(' ');
            if self.masked.contains(&index) {
                let key = argument
                    .split_once('=')
                    .map_or(argument.as_str(), |(key, _)| key);
                line.push_str(&format!("{key}=•••"));
            } else if argument.is_empty()
                || argument
                    .contains(|c: char| c.is_whitespace() || "*?[]$&;|<>(){}!'\"\\`~#".contains(c))
            {
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

    /// `mise upgrade --global --bump [tool]` (2026.10.7+): only tools in the
    /// global config.
    pub fn upgrade_global(tool: Option<&str>) -> Result<Self, MiseAdapterError> {
        let mut command = Self::upgrade(tool)?;
        command.arguments.insert(1, "--global".into());
        Ok(command)
    }

    /// `mise upgrade --local --bump [tool]` in a project: only tools the
    /// project's own config files ask for, updating those files to match.
    pub fn upgrade_project(
        tool: Option<&str>,
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        let mut command = Self::upgrade(tool)?;
        command.arguments.insert(1, "--local".into());
        Ok(command.in_project(project))
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

    /// `mise prune --tools --yes`: deletes tool versions no config uses
    /// anymore. `--yes` because mise won't prune without a terminal to
    /// confirm in, and the person already reviewed the list in the app.
    pub fn prune_tools() -> Self {
        Self::new(vec!["prune".into(), "--tools".into(), "--yes".into()])
    }

    /// `mise settings set <key> <value>`, in the global config.
    pub fn set_setting(key: &str, value: &str) -> Result<Self, MiseAdapterError> {
        validate_setting_key(key)?;
        if value.starts_with('-') || value.chars().any(char::is_control) {
            return Err(MiseAdapterError::InvalidArgument {
                value: value.into(),
            });
        }
        Ok(Self::new(vec![
            "settings".into(),
            "set".into(),
            key.into(),
            value.into(),
        ]))
    }

    /// `mise settings unset <key>`: back to the default, in the global config.
    pub fn unset_setting(key: &str) -> Result<Self, MiseAdapterError> {
        validate_setting_key(key)?;
        Ok(Self::new(vec![
            "settings".into(),
            "unset".into(),
            key.into(),
        ]))
    }

    /// `mise bootstrap --dry-run [--only <part>]`: what applying would do.
    pub fn bootstrap_preview(only: Option<&str>) -> Result<Self, MiseAdapterError> {
        bootstrap_with("--dry-run", only)
    }

    /// `mise bootstrap --yes [--only <part>]`: set the machine up as the
    /// config describes. `--yes` because the person confirmed in the app.
    pub fn bootstrap_apply(only: Option<&str>) -> Result<Self, MiseAdapterError> {
        bootstrap_with("--yes", only)
    }

    /// `mise dotfiles save`: checkpoint the tracked files now.
    pub fn dotfiles_save() -> Self {
        Self::new(vec!["dotfiles".into(), "save".into()])
    }

    /// `mise dotfiles sync`: push checkpoints and fetch other machines' changes.
    pub fn dotfiles_sync() -> Self {
        Self::new(vec!["dotfiles".into(), "sync".into()])
    }

    /// `mise dotfiles rollback --to <id> --all [--dry-run | --yes]`: return
    /// every tracked file to a checkpoint (or show what that would change).
    pub fn dotfiles_rollback(checkpoint: u64, dry_run: bool) -> Self {
        Self::new(vec![
            "dotfiles".into(),
            "rollback".into(),
            "--to".into(),
            checkpoint.to_string(),
            "--all".into(),
            if dry_run { "--dry-run" } else { "--yes" }.into(),
        ])
    }

    /// `mise dotfiles undo --yes`: reverse the latest tracked-file operation.
    pub fn dotfiles_undo() -> Self {
        Self::new(vec!["dotfiles".into(), "undo".into(), "--yes".into()])
    }

    /// `mise dotfiles origin set <url>`: connect the repository to sync with.
    pub fn dotfiles_origin_set(url: &str) -> Result<Self, MiseAdapterError> {
        let valid =
            (url.starts_with("https://") || url.starts_with("git@") || url.starts_with("ssh://"))
                && !url
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control());
        if !valid {
            return Err(MiseAdapterError::InvalidArgument { value: url.into() });
        }
        Ok(Self::new(vec![
            "dotfiles".into(),
            "origin".into(),
            "set".into(),
            url.into(),
        ]))
    }

    /// `mise tool-alias set <tool> <alias> <version>`, in the global config.
    pub fn set_tool_alias(
        tool: &str,
        alias: &str,
        version: &str,
    ) -> Result<Self, MiseAdapterError> {
        for value in [tool, alias, version] {
            validate_argument(value)?;
        }
        Ok(Self::new(vec![
            "tool-alias".into(),
            "set".into(),
            tool.into(),
            alias.into(),
            version.into(),
        ]))
    }

    /// `mise tool-alias unset <tool> <alias>`.
    pub fn unset_tool_alias(tool: &str, alias: &str) -> Result<Self, MiseAdapterError> {
        validate_argument(tool)?;
        validate_argument(alias)?;
        Ok(Self::new(vec![
            "tool-alias".into(),
            "unset".into(),
            tool.into(),
            alias.into(),
        ]))
    }

    /// `mise fmt` in a project: tidy the formatting of its config files.
    pub fn format_config(project: impl Into<PathBuf>) -> Self {
        Self::new(vec!["fmt".into()]).in_project(project)
    }

    /// `mise bootstrap packages use -g --yes <manager:package>`: install a
    /// system package and record it in the global config.
    pub fn use_package(spec: &str) -> Result<Self, MiseAdapterError> {
        validate_package_spec(spec)?;
        Ok(Self::new(vec![
            "bootstrap".into(),
            "packages".into(),
            "use".into(),
            "-g".into(),
            "--yes".into(),
            spec.into(),
        ]))
    }

    /// `mise lock` in a project: write or refresh its lockfile.
    pub fn lock(project: impl Into<PathBuf>) -> Self {
        Self::new(vec!["lock".into()]).in_project(project)
    }

    /// `mise config set --file <file> <key> <value> [--type <type>]` in a
    /// project: edit one value in one of its config files.
    pub fn config_set(
        project: impl Into<PathBuf>,
        file: &Path,
        key: &str,
        value: &str,
        value_type: Option<&str>,
    ) -> Result<Self, MiseAdapterError> {
        let project = project.into();
        let valid_key = !key.is_empty()
            && key.split('.').all(|part| {
                !part.is_empty()
                    && part.chars().all(|character| {
                        character.is_ascii_alphanumeric() || "-_".contains(character)
                    })
            });
        if !valid_key {
            return Err(MiseAdapterError::InvalidArgument { value: key.into() });
        }
        if value.starts_with('-') || value.chars().any(char::is_control) {
            return Err(MiseAdapterError::InvalidArgument {
                value: value.into(),
            });
        }
        let mut arguments = vec![
            "config".into(),
            "set".into(),
            "--file".into(),
            project_file(&project, file)?,
        ];
        if let Some(value_type) = value_type {
            if !["string", "integer", "float", "bool", "list"].contains(&value_type) {
                return Err(MiseAdapterError::InvalidArgument {
                    value: value_type.into(),
                });
            }
            arguments.extend(["--type".into(), value_type.into()]);
        }
        arguments.extend([key.into(), value.into()]);
        Ok(Self::new(arguments).in_project(project))
    }

    /// `mise config set --global tools.<tool>.auto_update <on>` (2026.10+):
    /// let mise upgrade one global tool in the background.
    pub fn tool_auto_update(tool: &str, on: bool) -> Result<Self, MiseAdapterError> {
        validate_argument(tool)?;
        if tool.contains('.') {
            return Err(MiseAdapterError::InvalidArgument { value: tool.into() });
        }
        Ok(Self::new(vec![
            "config".into(),
            "set".into(),
            "--global".into(),
            format!("tools.{tool}.auto_update"),
            on.to_string(),
        ]))
    }

    /// `mise generate <kind> --write [--task <task>]` in a project.
    pub fn generate(
        project: impl Into<PathBuf>,
        generator: Generator,
        task: Option<&str>,
    ) -> Result<Self, MiseAdapterError> {
        let mut arguments = vec![
            "generate".into(),
            generator.subcommand().into(),
            "--write".into(),
        ];
        if let (true, Some(task)) = (generator.takes_task(), task) {
            validate_argument(task)?;
            arguments.extend(["--task".into(), task.into()]);
        }
        Ok(Self::new(arguments).in_project(project))
    }

    /// `mise watch <task>` in a project: rerun the task whenever its sources
    /// change, until stopped.
    pub fn watch(task: &str, project: impl Into<PathBuf>) -> Result<Self, MiseAdapterError> {
        validate_argument(task)?;
        Ok(Self::new(vec!["watch".into(), task.into()]).in_project(project))
    }

    /// `mise vars <name>=<value>` in a project: save a value on this machine.
    pub fn set_var(
        name: &str,
        value: &str,
        project: impl Into<PathBuf>,
    ) -> Result<Self, MiseAdapterError> {
        validate_env_key(name)?;
        if value.chars().any(char::is_control) {
            return Err(MiseAdapterError::InvalidArgument {
                value: value.into(),
            });
        }
        Ok(Self::new(vec!["vars".into(), format!("{name}={value}")]).in_project(project))
    }

    /// `mise vars unset <name>` in a project: forget the saved value.
    pub fn unset_var(name: &str, project: impl Into<PathBuf>) -> Result<Self, MiseAdapterError> {
        validate_env_key(name)?;
        Ok(Self::new(vec!["vars".into(), "unset".into(), name.into()]).in_project(project))
    }

    /// `mise settings add <key> <value>`: append to a list setting.
    pub fn add_setting(key: &str, value: &str) -> Result<Self, MiseAdapterError> {
        let mut command = Self::set_setting(key, value)?;
        command.arguments[1] = "add".into();
        Ok(command)
    }

    /// `mise untrust <path>`: revoke trust for a config file or folder.
    pub fn untrust(path: &Path) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec!["untrust".into(), absolute_path(path)?]))
    }

    /// `mise cache prune --yes`: delete cache files not used recently.
    pub fn cache_prune() -> Self {
        Self::new(vec!["cache".into(), "prune".into(), "--yes".into()])
    }

    /// `mise cache clear --yes`: delete the whole download and metadata cache.
    pub fn cache_clear() -> Self {
        Self::new(vec!["cache".into(), "clear".into(), "--yes".into()])
    }

    /// `mise reshim`: rebuild the shims for every installed tool.
    pub fn reshim() -> Self {
        Self::new(vec!["reshim".into()])
    }

    /// `mise link <tool>@<version> <path>`: use an install made outside mise.
    pub fn link(tool: &str, version: &str, path: &Path) -> Result<Self, MiseAdapterError> {
        Ok(Self::new(vec![
            "link".into(),
            tool_spec(tool, version)?,
            absolute_path(path)?,
        ]))
    }

    /// `mise sync <tool> --<source>`: lets mise use versions another version
    /// manager already installed, without reinstalling them.
    pub fn sync(source: ImportSource) -> Self {
        Self::new(vec![
            "sync".into(),
            source.tool().into(),
            format!("--{}", source.flag()),
        ])
    }

    /// `mise x -- <program> <arguments>`: an agent's own CLI, with mise's
    /// tools on `PATH`. Callers pass fixed program names and validated ids.
    pub(crate) fn exec(program: &str, arguments: &[&str]) -> Self {
        let mut all = vec!["x".into(), "--".into(), program.into()];
        all.extend(arguments.iter().map(|argument| (*argument).to_owned()));
        Self::new(all)
    }

    /// `mise x <tool> -- <program> <arguments>`: a tool mise installs on
    /// first use (the skills CLI). Callers pass validated arguments.
    pub(crate) fn exec_tool(tool: &str, program: &str, arguments: &[&str]) -> Self {
        let mut all = vec!["x".into(), tool.into(), "--".into(), program.into()];
        all.extend(arguments.iter().map(|argument| (*argument).to_owned()));
        Self::new(all)
    }

    /// Sets an environment variable for the command.
    pub(crate) fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// `mise dotfiles apply --yes <targets>`: apply only the `[dotfiles]`
    /// entries for these files (2026.10+).
    pub fn dotfiles_apply(targets: &[PathBuf]) -> Self {
        let mut arguments = vec!["dotfiles".into(), "apply".into(), "--yes".into()];
        arguments.extend(targets.iter().map(|target| target.display().to_string()));
        Self::new(arguments)
    }

    /// `mise skills sync --global`: link the active tools' skills into
    /// `~/.claude/skills` (the `skills.dir` setting).
    pub fn skills_sync_global() -> Self {
        Self::new(vec!["skills".into(), "sync".into(), "--global".into()])
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

/// `manager:package` for the managers Discover installs with.
pub fn validate_package_spec(spec: &str) -> Result<(), MiseAdapterError> {
    let valid = spec.split_once(':').is_some_and(|(manager, package)| {
        ["brew", "apt"].contains(&manager)
            && !package.is_empty()
            && !package.starts_with('-')
            && package
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._+-@".contains(character))
    });
    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidArgument { value: spec.into() })
    }
}

/// Bootstrap parts `--only` accepts (2026.7.18 and 2026.10).
const BOOTSTRAP_PARTS: &[&str] = &[
    "plugins",
    "packages",
    "accounts",
    "files",
    "services",
    "firewall",
    "compose",
    "repos",
    "dotfiles",
    "mise-shell-activate",
    "macos-defaults",
    "macos-launchd-agents",
    "linux-systemd-units",
    "user",
    "tools",
];

fn bootstrap_with(mode: &str, only: Option<&str>) -> Result<MiseCommand, MiseAdapterError> {
    let mut arguments = vec!["bootstrap".into(), mode.into()];
    if let Some(part) = only {
        if !BOOTSTRAP_PARTS.contains(&part) {
            return Err(MiseAdapterError::InvalidArgument { value: part.into() });
        }
        arguments.extend(["--only".into(), part.into()]);
    }
    Ok(MiseCommand::new(arguments))
}

/// A path passed to mise: absolute, so it can't be read as a flag.
fn absolute_path(path: &Path) -> Result<String, MiseAdapterError> {
    let text = path.to_string_lossy();
    if path.is_absolute() && !text.chars().any(char::is_control) {
        Ok(text.into_owned())
    } else {
        Err(MiseAdapterError::InvalidArgument {
            value: text.into_owned(),
        })
    }
}

/// Setting keys as mise lists them: dotted lowercase words.
fn validate_setting_key(key: &str) -> Result<(), MiseAdapterError> {
    let valid = !key.is_empty()
        && key.split('.').all(|part| {
            !part.is_empty()
                && part.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        });
    if valid {
        Ok(())
    } else {
        Err(MiseAdapterError::InvalidArgument { value: key.into() })
    }
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
    fn settings_prune_and_sync_commands() {
        assert_eq!(
            MiseCommand::set_setting("github.credential_command", "gh auth token")
                .unwrap()
                .display(),
            "mise settings set github.credential_command 'gh auth token'"
        );
        assert_eq!(
            MiseCommand::unset_setting("jobs").unwrap().display(),
            "mise settings unset jobs"
        );
        for bad in ["", "Jobs", "jobs;rm", "--all", "node..x"] {
            assert!(MiseCommand::set_setting(bad, "1").is_err(), "{bad}");
        }
        assert!(MiseCommand::set_setting("jobs", "--yes").is_err());
        assert_eq!(
            MiseCommand::prune_tools().display(),
            "mise prune --tools --yes"
        );
        assert_eq!(
            MiseCommand::cache_clear().display(),
            "mise cache clear --yes"
        );
        assert_eq!(
            MiseCommand::link("node", "20.11.1", Path::new("/opt/node-20"))
                .unwrap()
                .display(),
            "mise link node@20.11.1 /opt/node-20"
        );
        assert!(MiseCommand::link("node", "20", Path::new("relative")).is_err());
        assert!(MiseCommand::untrust(Path::new("-rf")).is_err());
        assert_eq!(
            MiseCommand::use_package("brew:wget").unwrap().display(),
            "mise bootstrap packages use -g --yes brew:wget"
        );
        assert!(MiseCommand::use_package("pip:requests").is_err());
        assert!(MiseCommand::use_package("brew:-rf").is_err());
        assert_eq!(
            MiseCommand::bootstrap_apply(Some("dotfiles"))
                .unwrap()
                .display(),
            "mise bootstrap --yes --only dotfiles"
        );
        assert_eq!(
            MiseCommand::bootstrap_preview(None).unwrap().display(),
            "mise bootstrap --dry-run"
        );
        assert!(MiseCommand::bootstrap_apply(Some("task")).is_err());
        assert!(MiseCommand::dotfiles_origin_set("file:///etc").is_err());
        assert_eq!(
            MiseCommand::dotfiles_rollback(3, true).display(),
            "mise dotfiles rollback --to 3 --all --dry-run"
        );
        assert_eq!(
            MiseCommand::tool_auto_update("jq", true).unwrap().display(),
            "mise config set --global tools.jq.auto_update true"
        );
        assert_eq!(
            MiseCommand::config_set(
                "/code/app",
                Path::new("/code/app/mise.toml"),
                "tools.node",
                "22",
                None
            )
            .unwrap()
            .display(),
            "cd /code/app && mise config set --file mise.toml tools.node 22"
        );
        assert!(
            MiseCommand::config_set("/code/app", Path::new("/etc/passwd"), "a", "b", None).is_err()
        );
        assert!(
            MiseCommand::config_set("/code/app", Path::new("mise.toml"), "a b", "c", None).is_err()
        );
        assert_eq!(
            MiseCommand::generate("/code/app", crate::Generator::GithubAction, Some("ci"))
                .unwrap()
                .display(),
            "cd /code/app && mise generate github-action --write --task ci"
        );
        assert_eq!(
            MiseCommand::add_setting("idiomatic_version_file_enable_tools", "node")
                .unwrap()
                .display(),
            "mise settings add idiomatic_version_file_enable_tools node"
        );
        assert_eq!(
            MiseCommand::set_var("region", "eu west", "/code/app")
                .unwrap()
                .display(),
            "cd /code/app && mise vars 'region=eu west'"
        );
        assert!(
            MiseClient::new("/bin/mise", "/home")
                .with_environment(Some("staging;rm"))
                .is_err()
        );
        assert_eq!(
            MiseCommand::upgrade_global(Some("node")).unwrap().display(),
            "mise upgrade --global --bump node"
        );
        assert_eq!(
            MiseCommand::upgrade_project(None, "/code/app")
                .unwrap()
                .display(),
            "cd /code/app && mise upgrade --local --bump"
        );
        assert_eq!(
            MiseCommand::sync(crate::ImportSource::Nvm).display(),
            "mise sync node --nvm"
        );
        assert_eq!(
            MiseCommand::sync(crate::ImportSource::RubyBrew).display(),
            "mise sync ruby --brew"
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
