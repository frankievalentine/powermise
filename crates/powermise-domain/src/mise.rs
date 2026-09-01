use std::{
    collections::BTreeMap,
    error::Error,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseDryRunRequest {
    pub environment: String,
    pub config_file_name: String,
    pub config_contents: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseDryRunOutput {
    pub stdout: String,
    pub stderr: String,
}

/// A request to converge a selection environment with a real
/// `mise bootstrap --yes` run against the machine's mise configuration
/// directory. This installs tools and packages and must only be invoked from
/// explicit apply flows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseBootstrapApplyRequest {
    pub environment: String,
    /// The real mise configuration directory containing the generated
    /// `config.powermise-<selection>.toml` to converge.
    pub config_dir: PathBuf,
}

/// The captured output of a real `mise bootstrap --yes` convergence run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseBootstrapApplyOutput {
    pub stdout: String,
    pub stderr: String,
}

/// A package entry reported by `mise bootstrap status --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MiseBootstrapPackage {
    /// The bootstrap source, for example `brew` or `brew-cask`.
    pub source: String,
    pub package: String,
    #[serde(default, rename = "requested_version")]
    pub requested_version: String,
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "installed_version")]
    pub installed_version: String,
}

/// A tool entry reported by `mise bootstrap status --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MiseBootstrapTool {
    pub tool: String,
    #[serde(default, rename = "requested_version")]
    pub requested_version: String,
    #[serde(default, rename = "resolved_version")]
    pub resolved_version: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub installed: bool,
}

/// The parsed result of `mise bootstrap status --json`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiseBootstrapStatus {
    pub packages: Vec<MiseBootstrapPackage>,
    pub tools: Vec<MiseBootstrapTool>,
}

impl MiseBootstrapStatus {
    /// Renders the concrete follow-up actions mise would take to converge.
    pub fn follow_up_actions(&self) -> Vec<String> {
        let mut actions = self
            .packages
            .iter()
            .filter(|package| package.state != "installed")
            .map(|package| {
                format!("{}:{} {}", package.source, package.package, package.requested_version)
            })
            .collect::<Vec<_>>();
        actions.extend(
            self.tools
                .iter()
                .filter(|tool| !tool.installed || tool.state != "installed")
                .map(|tool| {
                    if tool.resolved_version.is_empty() {
                        format!("{}@{}", tool.tool, tool.requested_version)
                    } else {
                        format!("{}@{}", tool.tool, tool.resolved_version)
                    }
                }),
        );
        actions
    }
}

#[derive(Default, Deserialize)]
struct PackagesBySource {
    #[serde(flatten)]
    sources: BTreeMap<String, PackageSource>,
}

#[derive(Default, Deserialize)]
struct PackageSource {
    #[serde(default)]
    available: bool,
    #[serde(default)]
    packages: Vec<PackageEntry>,
}

#[derive(Default, Deserialize)]
struct PackageEntry {
    package: String,
    #[serde(default, rename = "requested_version")]
    requested_version: String,
    #[serde(default)]
    state: String,
    #[serde(default, rename = "installed_version")]
    installed_version: String,
}

#[derive(Deserialize)]
struct StatusDocument {
    #[serde(default)]
    packages: PackagesBySource,
    #[serde(default)]
    tools: Vec<MiseBootstrapTool>,
}

impl<'de> serde::Deserialize<'de> for MiseBootstrapStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let document = StatusDocument::deserialize(deserializer)?;
        let mut packages = Vec::new();
        for (source, entry) in document.packages.sources {
            let _ = entry.available;
            for package in entry.packages {
                packages.push(MiseBootstrapPackage {
                    source: source.clone(),
                    package: package.package,
                    requested_version: package.requested_version,
                    state: package.state,
                    installed_version: package.installed_version,
                });
            }
        }
        Ok(Self {
            packages,
            tools: document.tools,
        })
    }
}

pub trait MiseRunner {
    type Error: Error + Send + Sync + 'static;

    fn bootstrap_dry_run(
        &self,
        request: &MiseDryRunRequest,
    ) -> Result<MiseDryRunOutput, Self::Error>;

    fn bootstrap_status(
        &self,
        request: &MiseDryRunRequest,
    ) -> Result<MiseBootstrapStatus, Self::Error>;

    /// Converges the environment by running a real `mise bootstrap --yes`
    /// against the machine's `request.config_dir`. Unlike the sandboxed dry
    /// run and status probes, this installs tools and packages for real.
    fn bootstrap_apply(
        &self,
        request: &MiseBootstrapApplyRequest,
    ) -> Result<MiseBootstrapApplyOutput, Self::Error>;

    /// Runs a command inside the selection environment, inheriting the terminal.
    fn exec(
        &self,
        environment: &str,
        command: &[String],
    ) -> Result<std::process::ExitStatus, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parses_mise_bootstrap_json_and_lists_follow_ups() {
        let document = serde_json::json!({
            "packages": {
                "brew": {
                    "available": true,
                    "packages": [
                        { "package": "jq", "requested_version": "latest", "state": "missing", "installed_version": "" }
                    ]
                },
                "brew-cask": {
                    "available": true,
                    "packages": [
                        { "package": "cursor", "requested_version": "latest", "state": "installed", "installed_version": "1.0" }
                    ]
                }
            },
            "tools": [
                { "tool": "node", "requested_version": "24", "resolved_version": "24.19.0", "state": "missing", "installed": false },
                { "tool": "jq", "requested_version": "latest", "resolved_version": "1.8.2", "state": "installed", "installed": true }
            ]
        });

        let status: MiseBootstrapStatus =
            serde_json::from_value(document).expect("status should parse");
        assert_eq!(status.packages.len(), 2);
        assert_eq!(status.packages[0].source, "brew");
        assert_eq!(status.packages[0].state, "missing");
        assert_eq!(status.packages[1].state, "installed");
        assert_eq!(status.tools.len(), 2);
        assert_eq!(
            status.follow_up_actions(),
            [
                "brew:jq latest",
                "node@24.19.0"
            ]
        );
    }

    #[test]
    fn status_tolerates_missing_sections() {
        let status: MiseBootstrapStatus =
            serde_json::from_str("{}").expect("empty status should parse");
        assert!(status.packages.is_empty());
        assert!(status.tools.is_empty());
        assert!(status.follow_up_actions().is_empty());
    }
}