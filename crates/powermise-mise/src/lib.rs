//! A typed adapter for the installed `mise`: read its `--json` output and run
//! its commands. powermise never edits mise's own configuration; the only
//! file it writes is an AI setup's `conf.d/powermise-<name>.toml` fragment.
use std::{io, path::PathBuf};

mod agents;
mod client;
mod import_sources;
mod imports;
mod machine;
mod models;
mod project;
mod skills;
mod updater;

pub use agents::{AgentCli, AgentPlugin, gemini_extensions, parse_plugin_list};
pub use client::{
    CORE_TOOLS, MiseClient, MiseCommand, OutputLine, RunOutcome, Stream, run_program,
    validate_import_package, validate_package_spec,
};
pub use import_sources::{
    BrewItem, Brewfile, DefaultPackage, HomebrewInventory, MasApp, WindowsPackage, brewfile_paths,
    choco_installed, default_packages, homebrew_inventory, homebrew_prefixes, parse_brewfile,
    parse_choco_config, parse_mas_list, parse_package_list, parse_scoop_export,
    parse_winget_export, scoop_installed,
};
pub use imports::{
    DetectedImport, GlobalPackage, HOMEBREW_PREFIXES, ImportSource, ImportVersion,
    InstalledProgram, ShellConflict, detect_imports, global_packages, installed_programs,
    shell_conflicts,
};
pub use machine::{
    DotfileCheckpoint, DotfilesHistory, MachineItem, MachineSection, Plugin, Setting, TokenStatus,
    ToolAlias, directory_size, parse_checkpoints, parse_machine_status, parse_plugins,
    parse_settings, parse_token_status, parse_tool_aliases, trusted_configs,
};
pub use models::{
    BootstrapPlan, BootstrapStatus, ConfigFile, ConfigSource, Doctor, MiseDirs, MiseVersion,
    OutdatedReport, OutdatedTool, PackageManagerStatus, PackageStatus, PlanResource,
    PlanResourceId, PlanSummary, RegistryEntry, RemoteVersion, SecurityCheck, ShellInfo, Task,
    ToolInfo, ToolInventory, ToolStatus, ToolVersion, VersionInfo,
};
pub use project::{
    ConfigPreview, EnvReport, EnvVar, Generator, IdiomaticFile, LockChange, PROJECT_CONFIG_FILES,
    SecretsReport, TaskDetails, TaskInput, TrustStatus, VarDeclaration, VarEntry, declared_vars,
    idiomatic_files, is_untrusted_error, looks_secret, parse_task_details, parse_trust_show,
    parse_vars, preview_config, preview_project_configs, project_environments,
};
pub use skills::{
    InstalledSkill, SKILLS_AGENTS, SKILLS_CLI, SkillsAgent, detected_skills_agents,
    installed_skills, skills_add, skills_remove, skills_update,
};
pub use updater::{MiseUpdater, mise_updater};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MiseAdapterError {
    #[error("could not execute mise at `{executable}`: {source}")]
    Execute {
        executable: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "mise at `{executable}` failed with status {status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    Failed {
        executable: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("could not parse the JSON from `{command}`: {source}")]
    Json {
        command: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("`{value}` is not a valid argument for mise")]
    InvalidArgument { value: String },
    #[error("mise won't read this project's config until it's trusted")]
    Untrusted { stderr: String },
}
