//! A typed adapter for the installed `mise`: read its `--json` output and run
//! its commands. powermise never edits mise's own configuration; the only
//! file it writes is an AI setup's `conf.d/powermise-<name>.toml` fragment.
use std::{io, path::PathBuf};

mod client;
mod machine;
mod models;
mod project;

pub use client::{MiseClient, MiseCommand, OutputLine, RunOutcome, Stream, run_program};
pub use machine::{
    DetectedImport, ImportSource, Setting, TokenStatus, detect_imports, directory_size,
    parse_settings, parse_token_status,
};
pub use models::{
    BootstrapPlan, BootstrapStatus, ConfigFile, ConfigSource, Doctor, MiseVersion, OutdatedReport,
    OutdatedTool, PackageManagerStatus, PackageStatus, PlanResource, PlanResourceId, PlanSummary,
    RegistryEntry, RemoteVersion, ShellInfo, Task, ToolInventory, ToolStatus, ToolVersion,
    VersionInfo,
};
pub use project::{
    ConfigPreview, EnvReport, EnvVar, PROJECT_CONFIG_FILES, TaskDetails, TaskInput, TrustStatus,
    is_untrusted_error, looks_secret, parse_task_details, parse_trust_show, preview_config,
    preview_project_configs,
};

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
