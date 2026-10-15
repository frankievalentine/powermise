//! Compiles AI Setups picks into a mise config fragment.
//!
//! The output is plain mise configuration — `[tools]` plus
//! `[bootstrap.packages]` — that powermise writes to
//! `~/.config/mise/conf.d/powermise-<name>.toml`. mise does everything else:
//! `mise bootstrap` installs it, `mise ls` shows it, and deleting the file
//! removes it from the config.
use std::collections::{BTreeMap, BTreeSet};

use powermise_domain::{Platform, ToolCatalog, ToolProvider, ToolProviderKind};
use powermise_registry::ResolvedSelection;
use serde::Serialize;
use thiserror::Error;

/// A deterministic mise configuration compiled from a resolved selection.
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompiledMiseConfig {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BootstrapConfig::is_empty")]
    pub bootstrap: BootstrapConfig,
}

impl CompiledMiseConfig {
    /// Serializes this configuration using the TOML serializer.
    pub fn to_toml(&self) -> Result<String, CompilerError> {
        toml::to_string(self).map_err(CompilerError::Serialize)
    }
}

/// The bootstrap section of a mise configuration.
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct BootstrapConfig {
    /// `"manager:package" = "version"`, as mise's `[bootstrap.packages]` expects.
    pub packages: BTreeMap<String, String>,
}

impl BootstrapConfig {
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// How one resolved tool or app is installed, for previews.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Install {
    pub id: String,
    pub kind: ToolProviderKind,
    /// The `[tools]` key or `[bootstrap.packages]` key it compiles to.
    pub key: String,
    pub version: String,
}

/// The fragment plus a per-item description of it.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Compiled {
    pub config: CompiledMiseConfig,
    pub installs: Vec<Install>,
    /// Tools left out on this machine, with why (for example Homebrew
    /// formulas on an Intel Mac). Reported, never fatal.
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Skipped {
    pub id: String,
    pub reason: String,
}

/// What the machine a setup is compiled for can install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileOptions {
    /// mise installs Homebrew formulas only on Apple Silicon Macs and on
    /// Linux; on an Intel Mac `brew:` entries are skipped. Casks still work.
    pub homebrew_formulas: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            homebrew_formulas: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("selected {kind} `{id}` does not exist in the {kind} catalog")]
    MissingEntry { kind: &'static str, id: String },
    #[error(
        "selected {kind} `{id}` has no compatible provider for {target:?}{reason_suffix}",
        reason_suffix = format_unsupported_reason(unsupported_reason)
    )]
    NoCompatibleProvider {
        kind: &'static str,
        id: String,
        target: Platform,
        unsupported_reason: Option<String>,
    },
    #[error("`{key}` is requested at both `{existing_version}` and `{replacement_version}`")]
    PackageConflict {
        key: String,
        existing_version: String,
        replacement_version: String,
    },
    #[error("could not serialize mise configuration: {0}")]
    Serialize(#[source] toml::ser::Error),
}

fn format_unsupported_reason(unsupported_reason: &Option<String>) -> String {
    unsupported_reason
        .as_deref()
        .map(|reason| format!(": {reason}"))
        .unwrap_or_default()
}

/// Provider kinds in the order the compiler prefers them. mise tools first;
/// then what the OS already has; then Homebrew, which mise installs itself
/// and which needs no admin rights once its folder exists; apt last, since
/// every apt install needs `sudo`.
const PREFERENCE: [ToolProviderKind; 7] = [
    ToolProviderKind::MiseTool,
    ToolProviderKind::System,
    ToolProviderKind::BootstrapBrew,
    ToolProviderKind::BootstrapBrewCask,
    ToolProviderKind::BootstrapApt,
    ToolProviderKind::Winget,
    ToolProviderKind::Scoop,
];

/// The provider the compiler installs a tool with on `target`.
pub fn chosen_provider(providers: &[ToolProvider], target: Platform) -> Option<&ToolProvider> {
    chosen_provider_with(providers, target, CompileOptions::default())
}

/// [`chosen_provider`] for a machine with `options`' limits.
pub fn chosen_provider_with(
    providers: &[ToolProvider],
    target: Platform,
    options: CompileOptions,
) -> Option<&ToolProvider> {
    PREFERENCE
        .into_iter()
        .filter(|kind| options.homebrew_formulas || *kind != ToolProviderKind::BootstrapBrew)
        .find_map(|kind| {
            providers.iter().find(|provider| {
                provider.kind == kind && provider.supported_platforms.contains(&target)
            })
        })
}

/// The `[bootstrap.packages]` manager prefix for a system package provider.
fn package_manager(kind: &ToolProviderKind) -> Option<&'static str> {
    match kind {
        ToolProviderKind::BootstrapApt => Some("apt"),
        ToolProviderKind::BootstrapBrew => Some("brew"),
        ToolProviderKind::BootstrapBrewCask => Some("brew-cask"),
        ToolProviderKind::Winget => Some("winget"),
        ToolProviderKind::Scoop => Some("scoop"),
        ToolProviderKind::MiseTool | ToolProviderKind::System => None,
    }
}

/// The version to declare. Homebrew formulae and casks don't honor version
/// pins in mise (versioned formulae such as `postgresql@17` carry the
/// version in the name), so declaring one would only suggest a guarantee
/// mise won't keep.
fn declared_version(kind: &ToolProviderKind, version: &str) -> String {
    match kind {
        ToolProviderKind::BootstrapBrew | ToolProviderKind::BootstrapBrewCask => "latest".into(),
        _ => version.into(),
    }
}

/// Compiles a resolved selection into a mise config fragment for `target`.
pub fn compile(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    target: Platform,
) -> Result<Compiled, CompilerError> {
    compile_with(selection, tool_catalog, target, CompileOptions::default())
}

/// [`compile`] for a machine with `options`' limits.
pub fn compile_with(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    target: Platform,
    options: CompileOptions,
) -> Result<Compiled, CompilerError> {
    let mut compiled = Compiled::default();
    for tool_id in selection.tool_ids.iter().collect::<BTreeSet<_>>() {
        let tool = tool_catalog
            .tools
            .iter()
            .find(|tool| &tool.id == tool_id)
            .ok_or_else(|| CompilerError::MissingEntry {
                kind: "tool",
                id: tool_id.clone(),
            })?;
        add(
            &mut compiled,
            "tool",
            &tool.id,
            &tool.providers,
            tool.unsupported_reason.as_ref(),
            target,
            options,
        )?;
    }
    Ok(compiled)
}

/// Compiles only the mise configuration (no per-item descriptions).
pub fn compile_mise(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    target: Platform,
) -> Result<CompiledMiseConfig, CompilerError> {
    compile(selection, tool_catalog, target).map(|compiled| compiled.config)
}

fn add(
    compiled: &mut Compiled,
    kind: &'static str,
    id: &str,
    providers: &[ToolProvider],
    unsupported_reason: Option<&String>,
    target: Platform,
    options: CompileOptions,
) -> Result<(), CompilerError> {
    let Some(provider) = chosen_provider_with(providers, target, options) else {
        // Only this machine's limits rule it out: skip it, and say why.
        if chosen_provider(providers, target).is_some() {
            compiled.skipped.push(Skipped {
                id: id.into(),
                reason: "mise installs Homebrew formulas only on Apple Silicon Macs".into(),
            });
            return Ok(());
        }
        return Err(CompilerError::NoCompatibleProvider {
            kind,
            id: id.into(),
            target,
            unsupported_reason: unsupported_reason.cloned(),
        });
    };
    if provider.kind == ToolProviderKind::System {
        compiled.installs.push(Install {
            id: id.into(),
            kind: ToolProviderKind::System,
            key: provider.package.clone(),
            version: String::new(),
        });
        return Ok(());
    }
    let version = declared_version(&provider.kind, &provider.version);
    let (table, key) = match package_manager(&provider.kind) {
        None => (&mut compiled.config.tools, provider.package.clone()),
        Some(manager) => (
            &mut compiled.config.bootstrap.packages,
            format!("{manager}:{}", provider.package),
        ),
    };
    match table.get(&key) {
        Some(existing) if existing != &version => {
            return Err(CompilerError::PackageConflict {
                key,
                existing_version: existing.clone(),
                replacement_version: version,
            });
        }
        Some(_) => {}
        None => {
            table.insert(key.clone(), version.clone());
        }
    }
    compiled.installs.push(Install {
        id: id.into(),
        kind: provider.kind.clone(),
        key,
        version,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use powermise_domain::Tool;

    fn provider(
        kind: ToolProviderKind,
        package: &str,
        version: &str,
        platforms: &[Platform],
    ) -> ToolProvider {
        ToolProvider {
            kind,
            package: package.into(),
            version: version.into(),
            command: None,
            supported_platforms: platforms.to_vec(),
        }
    }

    fn tool(id: &str, providers: Vec<ToolProvider>) -> Tool {
        Tool {
            id: id.into(),
            title: id.into(),
            description: format!("{id} tool"),
            providers,
            unsupported_reason: None,
        }
    }

    fn tools(tools: Vec<Tool>) -> ToolCatalog {
        ToolCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            tools,
        }
    }

    fn selection(tool_ids: &[&str]) -> ResolvedSelection {
        ResolvedSelection {
            harness_ids: vec![],
            tool_ids: tool_ids.iter().map(|id| (*id).into()).collect(),
            desktop_app_ids: vec![],
            unavailable_desktop_ids: vec![],
            integration_ids: vec![],
            mcp_server_ids: vec![],
        }
    }

    use Platform::{Darwin, Linux, Win32};
    use ToolProviderKind::{
        BootstrapApt, BootstrapBrew, BootstrapBrewCask, MiseTool, Scoop, System, Winget,
    };

    #[test]
    fn emits_deterministic_mise_toml() {
        let catalog = tools(vec![
            tool("z", vec![provider(MiseTool, "zulu", "22", &[Darwin])]),
            tool(
                "a",
                vec![provider(BootstrapBrew, "alpha", "1.2", &[Darwin])],
            ),
            tool(
                "b",
                vec![provider(BootstrapBrewCask, "beta", "2.0", &[Darwin])],
            ),
            tool(
                "m",
                vec![provider(MiseTool, "npm:@openai/codex", "0.51.0", &[Darwin])],
            ),
        ]);
        let output = compile_mise(&selection(&["z", "b", "a", "m"]), &catalog, Darwin)
            .unwrap()
            .to_toml()
            .unwrap();
        assert_eq!(
            output,
            "[tools]\n\"npm:@openai/codex\" = \"0.51.0\"\nzulu = \"22\"\n\n[bootstrap.packages]\n\"brew-cask:beta\" = \"latest\"\n\"brew:alpha\" = \"latest\"\n"
        );
    }

    #[test]
    fn homebrew_is_declared_latest_and_pinning_managers_keep_their_pin() {
        let catalog = tools(vec![
            tool(
                "brewed",
                vec![provider(BootstrapBrew, "jq", "1.8.1", &[Darwin])],
            ),
            tool(
                "apted",
                vec![provider(BootstrapApt, "jq", "1.7.1-3", &[Linux])],
            ),
            tool(
                "winget",
                vec![provider(Winget, "jqlang.jq", "1.8.1", &[Win32])],
            ),
            tool("scoop", vec![provider(Scoop, "zed", "0.200.0", &[Win32])]),
        ]);
        let darwin = compile(&selection(&["brewed"]), &catalog, Darwin).unwrap();
        assert_eq!(darwin.config.bootstrap.packages["brew:jq"], "latest");
        let linux = compile(&selection(&["apted"]), &catalog, Linux).unwrap();
        assert_eq!(linux.config.bootstrap.packages["apt:jq"], "1.7.1-3");
        let windows = compile(&selection(&["winget", "scoop"]), &catalog, Win32).unwrap();
        assert_eq!(
            windows.config.bootstrap.packages["winget:jqlang.jq"],
            "1.8.1"
        );
        assert_eq!(windows.config.bootstrap.packages["scoop:zed"], "0.200.0");
    }

    #[test]
    fn prefers_mise_tools_then_system_packages_then_windows_managers() {
        let catalog = tools(vec![tool(
            "git",
            vec![
                provider(Winget, "Git.Git", "2.51.0", &[Win32]),
                provider(BootstrapBrew, "git", "latest", &[Darwin, Linux]),
                provider(BootstrapApt, "git", "latest", &[Linux]),
                provider(MiseTool, "git", "2.51.0", &[Darwin]),
            ],
        )]);
        let pick = |target| {
            compile(&selection(&["git"]), &catalog, target)
                .unwrap()
                .installs[0]
                .key
                .clone()
        };
        assert_eq!(pick(Darwin), "git");
        // Homebrew before apt: mise installs it without `sudo` once its
        // folder exists, while every apt install needs `sudo`.
        assert_eq!(pick(Linux), "brew:git");
        assert_eq!(pick(Win32), "winget:Git.Git");
    }

    #[test]
    fn apt_is_the_last_resort_on_linux() {
        let catalog = tools(vec![tool(
            "jq",
            vec![provider(BootstrapApt, "jq", "latest", &[Linux])],
        )]);
        let compiled = compile(&selection(&["jq"]), &catalog, Linux).unwrap();
        assert_eq!(compiled.installs[0].key, "apt:jq");
    }

    #[test]
    fn system_tools_install_nothing() {
        let catalog = tools(vec![tool(
            "git",
            vec![
                provider(System, "git", "system", &[Darwin]),
                provider(BootstrapBrew, "git", "2.51.0", &[Darwin]),
            ],
        )]);
        let compiled = compile(&selection(&["git"]), &catalog, Darwin).unwrap();
        assert_eq!(compiled.installs[0].kind, System);
        assert!(compiled.config.bootstrap.packages.is_empty());
        assert!(compiled.config.tools.is_empty());
    }

    #[test]
    fn intel_macs_skip_homebrew_formulas_but_keep_casks() {
        let catalog = tools(vec![
            tool(
                "gemini",
                vec![provider(BootstrapBrew, "gemini-cli", "0.46.0", &[Darwin])],
            ),
            tool(
                "node",
                vec![
                    provider(BootstrapBrew, "node", "26.8.1", &[Darwin]),
                    provider(MiseTool, "node", "26.8.1", &[Linux]),
                ],
            ),
            tool(
                "zed",
                vec![provider(BootstrapBrewCask, "zed", "0.200.0", &[Darwin])],
            ),
        ]);
        let intel = CompileOptions {
            homebrew_formulas: false,
        };
        let compiled = compile_with(
            &selection(&["gemini", "node", "zed"]),
            &catalog,
            Darwin,
            intel,
        )
        .unwrap();
        let installed: Vec<_> = compiled
            .installs
            .iter()
            .map(|install| install.key.as_str())
            .collect();
        assert_eq!(installed, ["brew-cask:zed"]);
        let skipped: Vec<_> = compiled
            .skipped
            .iter()
            .map(|skip| skip.id.as_str())
            .collect();
        assert_eq!(skipped, ["gemini", "node"]);
    }

    #[test]
    fn installs_describe_every_item() {
        let catalog = tools(vec![
            tool(
                "rg",
                vec![provider(MiseTool, "ripgrep", "15.1.0", &[Darwin])],
            ),
            tool(
                "cursor",
                vec![provider(BootstrapBrewCask, "cursor", "1.7", &[Darwin])],
            ),
        ]);
        let compiled = compile(&selection(&["rg", "cursor"]), &catalog, Darwin).unwrap();
        assert_eq!(
            compiled.installs,
            [
                Install {
                    id: "cursor".into(),
                    kind: BootstrapBrewCask,
                    key: "brew-cask:cursor".into(),
                    version: "latest".into()
                },
                Install {
                    id: "rg".into(),
                    kind: MiseTool,
                    key: "ripgrep".into(),
                    version: "15.1.0".into()
                },
            ]
        );
    }

    #[test]
    fn rejects_unknown_or_unsupported_tools() {
        let catalog = tools(vec![Tool {
            unsupported_reason: Some("vendor scripts only".into()),
            ..tool("hermes", vec![])
        }]);
        let unsupported = compile(&selection(&["hermes"]), &catalog, Darwin).unwrap_err();
        assert!(unsupported.to_string().contains("vendor scripts only"));
        assert!(matches!(
            compile(&selection(&["missing"]), &catalog, Darwin),
            Err(CompilerError::MissingEntry { kind: "tool", .. })
        ));
    }

    #[test]
    fn conflicting_versions_for_one_key_are_rejected() {
        let catalog = tools(vec![
            tool("one", vec![provider(MiseTool, "node", "22", &[Darwin])]),
            tool("two", vec![provider(MiseTool, "node", "24", &[Darwin])]),
        ]);
        assert!(matches!(
            compile(&selection(&["one", "two"]), &catalog, Darwin),
            Err(CompilerError::PackageConflict { .. })
        ));
    }
}
