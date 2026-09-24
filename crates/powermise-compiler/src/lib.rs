use std::collections::{BTreeMap, BTreeSet};

use powermise_domain::{
    AppCatalog, ConfigProvider, IntegrationCatalog, McpServerCatalog, Operation, OperationAction,
    OperationGraph, OperationGraphError, OperationOwnership, OperationProvider,
    OperationReversibility, OperationVerification, Plan, Platform, ResourceKind, ReversibilityKind,
    ToolCatalog, ToolProvider, ToolProviderKind, VerificationKind,
};
use powermise_providers::native_check_command;
use powermise_registry::ResolvedSelection;
use serde::Serialize;
use thiserror::Error;

/// A deterministic mise configuration compiled from a resolved profile selection.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct CompiledMiseConfig {
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
    pub packages: BTreeMap<String, String>,
}

impl BootstrapConfig {
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("selected {kind} `{id}` does not exist in the {kind} catalog")]
    MissingEntry { kind: ResourceKind, id: String },
    #[error(
        "selected {kind} `{id}` has no compatible mise provider for {target:?}{reason_suffix}",
        reason_suffix = format_unsupported_reason(unsupported_reason)
    )]
    NoCompatibleProvider {
        kind: ResourceKind,
        id: String,
        target: Platform,
        unsupported_reason: Option<String>,
    },
    #[error(
        "selected {kind} `{id}` has no config-adapter provider for {target:?}{reason_suffix}",
        reason_suffix = format_unsupported_reason(unsupported_reason)
    )]
    NoCompatibleConfigProvider {
        kind: ResourceKind,
        id: String,
        target: Platform,
        unsupported_reason: Option<String>,
    },
    #[error(
        "conflicting {provider_kind:?} providers for package `{package}`: `{existing_version}` and `{replacement_version}`"
    )]
    PackageConflict {
        provider_kind: ToolProviderKind,
        package: String,
        existing_version: String,
        replacement_version: String,
    },
    #[error("could not build operation graph: {0}")]
    OperationGraph(#[from] OperationGraphError),
    #[error("could not serialize mise configuration: {0}")]
    Serialize(#[source] toml::ser::Error),
}

fn format_unsupported_reason(unsupported_reason: &Option<String>) -> String {
    unsupported_reason
        .as_deref()
        .map(|reason| format!(": {reason}"))
        .unwrap_or_default()
}

/// An installable that must be provisioned through a native package manager.
struct NativeInstallable {
    kind: ResourceKind,
    provider: OperationProvider,
    package: String,
    version: String,
}

/// Compiles a resolved profile selection into a deterministic mise configuration.
///
/// Installables that only have a native provider for `target` are skipped; they
/// are surfaced as native operations by [`compile_plan`].
pub fn compile_mise(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    app_catalog: &AppCatalog,
    target: Platform,
) -> Result<CompiledMiseConfig, CompilerError> {
    compile_installables(selection, tool_catalog, app_catalog, target).map(|(config, _)| config)
}

/// A single lazy-install stub: the `mise-tool` package backing a resolved tool
/// and the executable name it places on `PATH`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StubEntry {
    pub tool_id: String,
    pub package: String,
    pub command: String,
}

/// Compiles the lazy-install stub entries for a resolved selection: every tool
/// whose provider for `target` is `mise-tool`. Tools installed through native
/// providers (Homebrew bootstrap, winget, scoop) or without a compatible
/// provider are skipped — they are real `PATH` binaries already.
pub fn compile_stubs(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    target: Platform,
) -> Result<Vec<StubEntry>, CompilerError> {
    let mut entries = Vec::new();
    for tool_id in selection.tool_ids.iter().collect::<BTreeSet<_>>() {
        let tool = tool_catalog
            .tools
            .iter()
            .find(|tool| tool.id == **tool_id)
            .ok_or_else(|| CompilerError::MissingEntry {
                kind: ResourceKind::Tool,
                id: (*tool_id).clone(),
            })?;
        let Some(provider) = select_provider(&tool.providers, target) else {
            continue;
        };
        if provider.kind != ToolProviderKind::MiseTool {
            continue;
        }
        entries.push(StubEntry {
            tool_id: (*tool_id).clone(),
            package: provider.package.clone(),
            command: provider
                .command
                .clone()
                .unwrap_or_else(|| default_stub_command(&provider.package)),
        });
    }
    Ok(entries)
}

/// Derives a stub command name from a mise package identifier, stripping the
/// backend prefix and any path segments (`npm:@openai/codex` becomes `codex`).
/// Catalog entries should declare an explicit `command` when the executable
/// differs from this derivation.
fn default_stub_command(package: &str) -> String {
    let without_backend = package.split_once(':').map_or(package, |(_, rest)| rest);
    without_backend
        .rsplit('/')
        .next()
        .unwrap_or(without_backend)
        .to_string()
}

fn compile_installables(
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    app_catalog: &AppCatalog,
    target: Platform,
) -> Result<(CompiledMiseConfig, Vec<NativeInstallable>), CompilerError> {
    let mut config = CompiledMiseConfig {
        tools: BTreeMap::new(),
        bootstrap: BootstrapConfig::default(),
    };
    let mut natives = Vec::new();

    for tool_id in selection.tool_ids.iter().collect::<BTreeSet<_>>() {
        let tool = tool_catalog
            .tools
            .iter()
            .find(|tool| tool.id == **tool_id)
            .ok_or_else(|| CompilerError::MissingEntry {
                kind: ResourceKind::Tool,
                id: (*tool_id).clone(),
            })?;
        compile_installable(
            &mut config,
            &mut natives,
            &tool.providers,
            tool.unsupported_reason.as_ref(),
            ResourceKind::Tool,
            tool.id.as_str(),
            target,
        )?;
    }
    for app_id in selection.app_ids.iter().collect::<BTreeSet<_>>() {
        let app = app_catalog
            .apps
            .iter()
            .find(|app| app.id == **app_id)
            .ok_or_else(|| CompilerError::MissingEntry {
                kind: ResourceKind::Application,
                id: (*app_id).clone(),
            })?;
        compile_installable(
            &mut config,
            &mut natives,
            &app.providers,
            app.unsupported_reason.as_ref(),
            ResourceKind::Application,
            app.id.as_str(),
            target,
        )?;
    }

    Ok((config, natives))
}

fn compile_installable(
    config: &mut CompiledMiseConfig,
    natives: &mut Vec<NativeInstallable>,
    providers: &[ToolProvider],
    unsupported_reason: Option<&String>,
    kind: ResourceKind,
    id: &str,
    target: Platform,
) -> Result<(), CompilerError> {
    if let Some(provider) = select_provider(providers, target) {
        return apply_provider(config, provider);
    }
    if let Some((provider, native_provider)) = select_native_provider(providers, target) {
        natives.push(NativeInstallable {
            kind,
            provider: native_provider,
            package: provider.package.clone(),
            version: provider.version.clone(),
        });
        return Ok(());
    }
    Err(CompilerError::NoCompatibleProvider {
        kind,
        id: id.into(),
        target,
        unsupported_reason: unsupported_reason.cloned(),
    })
}

fn apply_provider(
    config: &mut CompiledMiseConfig,
    provider: &ToolProvider,
) -> Result<(), CompilerError> {
    match provider.kind {
        ToolProviderKind::MiseTool => insert_package(
            &mut config.tools,
            provider.package.clone(),
            &provider.version,
            ToolProviderKind::MiseTool,
        ),
        ToolProviderKind::BootstrapApt => insert_package(
            &mut config.bootstrap.packages,
            format!("apt:{}", provider.package),
            &provider.version,
            ToolProviderKind::BootstrapApt,
        ),
        ToolProviderKind::BootstrapBrew => insert_package(
            &mut config.bootstrap.packages,
            format!("brew:{}", provider.package),
            &provider.version,
            ToolProviderKind::BootstrapBrew,
        ),
        ToolProviderKind::BootstrapBrewCask => insert_package(
            &mut config.bootstrap.packages,
            format!("brew-cask:{}", provider.package),
            &provider.version,
            ToolProviderKind::BootstrapBrewCask,
        ),
        ToolProviderKind::Winget | ToolProviderKind::Scoop => {
            unreachable!("select_provider only returns supported mise provider kinds")
        }
    }
}

/// Compiles a resolved selection into a deterministic, provider-aware operation plan.
pub fn compile_plan(
    selection_name: &str,
    selection: &ResolvedSelection,
    tool_catalog: &ToolCatalog,
    app_catalog: &AppCatalog,
    integration_catalog: &IntegrationCatalog,
    mcp_catalog: &McpServerCatalog,
    target: Platform,
) -> Result<Plan, CompilerError> {
    let (config, natives) = compile_installables(selection, tool_catalog, app_catalog, target)?;
    let mut operations = operations_from_config(selection_name, &config);
    operations.extend(native_operations(selection_name, &natives));
    operations.extend(compile_config_adapter_operations(
        selection_name,
        ResourceKind::Integration,
        selection.integration_ids.iter().cloned(),
        &integration_catalog
            .integrations
            .iter()
            .map(|entry| ConfigAdapterEntry {
                id: &entry.id,
                providers: &entry.providers,
                unsupported_reason: entry.unsupported_reason.as_ref(),
            })
            .collect::<Vec<_>>(),
        target,
    )?);
    operations.extend(compile_config_adapter_operations(
        selection_name,
        ResourceKind::McpServer,
        selection.mcp_server_ids.iter().cloned(),
        &mcp_catalog
            .mcp_servers
            .iter()
            .map(|entry| ConfigAdapterEntry {
                id: &entry.id,
                providers: &entry.providers,
                unsupported_reason: entry.unsupported_reason.as_ref(),
            })
            .collect::<Vec<_>>(),
        target,
    )?);

    OperationGraph::try_new(operations)
        .map(|operation_graph| Plan::new(selection_name, target, operation_graph))
        .map_err(Into::into)
}

/// Builds a deterministic operation plan from an already compiled mise configuration.
pub fn compile_plan_from_config(
    selection_name: &str,
    config: &CompiledMiseConfig,
    target: Platform,
) -> Result<Plan, CompilerError> {
    let operations = operations_from_config(selection_name, config);
    OperationGraph::try_new(operations)
        .map(|operation_graph| Plan::new(selection_name, target, operation_graph))
        .map_err(Into::into)
}

fn operations_from_config(selection_name: &str, config: &CompiledMiseConfig) -> Vec<Operation> {
    let ownership = || OperationOwnership {
        owner: "powermise".into(),
        selection: selection_name.into(),
    };
    let reversibility = || OperationReversibility {
        kind: ReversibilityKind::DeclarationOnly,
        note: "removing the generated declaration does not uninstall the resource".into(),
    };
    let mut operations = Vec::with_capacity(config.tools.len() + config.bootstrap.packages.len());

    operations.extend(config.tools.iter().map(|(package, version)| Operation {
        id: format!("mise-tool:{package}"),
        provider: OperationProvider::MiseTools,
        resource_kind: ResourceKind::Tool,
        resource_id: package.clone(),
        action: OperationAction::EnsurePresent,
        requested_state: version.clone(),
        depends_on: Vec::new(),
        ownership: ownership(),
        verification: OperationVerification {
            kind: VerificationKind::MiseStatus,
            expected: format!("{package}@{version}"),
        },
        reversibility: reversibility(),
    }));
    operations.extend(
        config
            .bootstrap
            .packages
            .iter()
            .map(|(package, version)| Operation {
                id: format!("mise-bootstrap:{package}"),
                provider: OperationProvider::MiseBootstrap,
                resource_kind: ResourceKind::Package,
                resource_id: package.clone(),
                action: OperationAction::EnsurePresent,
                requested_state: version.clone(),
                depends_on: Vec::new(),
                ownership: ownership(),
                verification: OperationVerification {
                    kind: VerificationKind::MiseStatus,
                    expected: format!("{package}@{version}"),
                },
                reversibility: reversibility(),
            }),
    );

    operations
}

/// Builds the native package-manager operations for installables without a mise provider.
fn native_operations(selection_name: &str, natives: &[NativeInstallable]) -> Vec<Operation> {
    natives
        .iter()
        .map(|native| Operation {
            id: format!("native-{}:{}", native.provider, native.package),
            provider: native.provider,
            resource_kind: native.kind,
            resource_id: native.package.clone(),
            action: OperationAction::EnsurePresent,
            requested_state: native.version.clone(),
            depends_on: Vec::new(),
            ownership: OperationOwnership {
                owner: "powermise".into(),
                selection: selection_name.into(),
            },
            verification: OperationVerification {
                kind: VerificationKind::Command,
                expected: native_check_command(native.provider, &native.package).unwrap_or_else(
                    |_| format!("{} present for `{}`", native.provider, native.package),
                ),
            },
            reversibility: OperationReversibility {
                kind: ReversibilityKind::NotAutomated,
                note: format!(
                    "manual reversal: uninstall `{}` with your package manager",
                    native.package
                ),
            },
        })
        .collect()
}

struct ConfigAdapterEntry<'a> {
    id: &'a String,
    providers: &'a [ConfigProvider],
    unsupported_reason: Option<&'a String>,
}

fn compile_config_adapter_operations(
    selection_name: &str,
    kind: ResourceKind,
    requested: impl Iterator<Item = String>,
    entries: &[ConfigAdapterEntry<'_>],
    target: Platform,
) -> Result<Vec<Operation>, CompilerError> {
    let mut operations = Vec::new();
    for id in requested {
        let entry = entries
            .iter()
            .find(|entry| *entry.id == id)
            .ok_or_else(|| CompilerError::MissingEntry {
                kind,
                id: id.clone(),
            })?;
        let provider = entry
            .providers
            .iter()
            .find(|provider| provider.supported_platforms.contains(&target))
            .ok_or_else(|| CompilerError::NoCompatibleConfigProvider {
                kind,
                id: entry.id.clone(),
                target,
                unsupported_reason: entry.unsupported_reason.cloned(),
            })?;
        operations.push(Operation {
            id: format!("config-adapter:{}:{}", kind, entry.id),
            provider: OperationProvider::ConfigAdapter,
            resource_kind: kind,
            resource_id: entry.id.clone(),
            action: OperationAction::Configure,
            requested_state: provider.config_target.clone(),
            depends_on: Vec::new(),
            ownership: OperationOwnership {
                owner: "powermise".into(),
                selection: selection_name.into(),
            },
            verification: OperationVerification {
                kind: VerificationKind::AdapterCheck,
                expected: format!("{} configured", entry.id),
            },
            reversibility: OperationReversibility {
                kind: ReversibilityKind::RestorableSnapshot,
                note: "managed configuration block is restored from a Powermise snapshot".into(),
            },
        });
    }
    Ok(operations)
}

fn select_provider(providers: &[ToolProvider], target: Platform) -> Option<&ToolProvider> {
    [
        ToolProviderKind::MiseTool,
        ToolProviderKind::BootstrapApt,
        ToolProviderKind::BootstrapBrew,
        ToolProviderKind::BootstrapBrewCask,
    ]
    .into_iter()
    .find_map(|kind| {
        providers.iter().find(|provider| {
            provider.kind == kind && provider.supported_platforms.contains(&target)
        })
    })
}

fn select_native_provider(
    providers: &[ToolProvider],
    target: Platform,
) -> Option<(&ToolProvider, OperationProvider)> {
    [
        (ToolProviderKind::Winget, OperationProvider::Winget),
        (ToolProviderKind::Scoop, OperationProvider::Scoop),
    ]
    .into_iter()
    .find_map(|(kind, provider)| {
        providers
            .iter()
            .find(|candidate| {
                candidate.kind == kind && candidate.supported_platforms.contains(&target)
            })
            .map(|candidate| (candidate, provider))
    })
}

fn insert_package(
    packages: &mut BTreeMap<String, String>,
    package: String,
    version: &str,
    provider_kind: ToolProviderKind,
) -> Result<(), CompilerError> {
    match packages.get(&package) {
        Some(existing_version) if existing_version != version => {
            Err(CompilerError::PackageConflict {
                provider_kind,
                package,
                existing_version: existing_version.clone(),
                replacement_version: version.into(),
            })
        }
        Some(_) => Ok(()),
        None => {
            packages.insert(package, version.into());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use powermise_domain::{
        App, Integration, McpServer, Tool, ToolCatalog, ToolProvider, ToolProviderKind,
    };

    fn provider(
        kind: ToolProviderKind,
        package: &str,
        version: &str,
        supported_platforms: Vec<Platform>,
    ) -> ToolProvider {
        ToolProvider {
            kind,
            package: package.into(),
            version: version.into(),
            command: None,
            supported_platforms,
        }
    }

    fn provider_with_command(
        kind: ToolProviderKind,
        package: &str,
        version: &str,
        supported_platforms: Vec<Platform>,
        command: &str,
    ) -> ToolProvider {
        ToolProvider {
            command: Some(command.into()),
            ..provider(kind, package, version, supported_platforms)
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

    fn catalog(tools: Vec<Tool>) -> ToolCatalog {
        ToolCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            tools,
        }
    }

    fn empty_app_catalog() -> AppCatalog {
        AppCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            apps: vec![],
        }
    }

    fn empty_integration_catalog() -> IntegrationCatalog {
        IntegrationCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            integrations: vec![],
        }
    }

    fn empty_mcp_catalog() -> McpServerCatalog {
        McpServerCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            mcp_servers: vec![],
        }
    }

    fn config_provider(target: Platform) -> ConfigProvider {
        ConfigProvider {
            kind: powermise_domain::ConfigProviderKind::ConfigAdapter,
            config_target: "claude".into(),
            supported_platforms: vec![target],
        }
    }

    fn empty_selection() -> ResolvedSelection {
        ResolvedSelection {
            harness_ids: vec![],
            tool_ids: vec![],
            desktop_app_ids: vec![],
            unavailable_desktop_ids: vec![],
            app_ids: vec![],
            integration_ids: vec![],
            mcp_server_ids: vec![],
        }
    }

    #[test]
    fn emits_deterministic_toml_in_sorted_key_order() {
        let catalog = catalog(vec![
            tool(
                "z-tool",
                vec![provider(
                    ToolProviderKind::MiseTool,
                    "zulu",
                    "22",
                    vec![Platform::Darwin],
                )],
            ),
            tool(
                "a-tool",
                vec![provider(
                    ToolProviderKind::BootstrapBrew,
                    "alpha",
                    "latest",
                    vec![Platform::Darwin],
                )],
            ),
            tool(
                "b-tool",
                vec![provider(
                    ToolProviderKind::BootstrapBrewCask,
                    "beta",
                    "2.0",
                    vec![Platform::Darwin],
                )],
            ),
            tool(
                "m-tool",
                vec![provider(
                    ToolProviderKind::MiseTool,
                    "mise",
                    "2025.1.0",
                    vec![Platform::Darwin],
                )],
            ),
        ]);
        let mut selection = empty_selection();
        selection.tool_ids = vec![
            "z-tool".into(),
            "b-tool".into(),
            "a-tool".into(),
            "m-tool".into(),
        ];

        let output = compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Darwin)
            .unwrap()
            .to_toml()
            .unwrap();

        assert_eq!(
            output,
            "[tools]\nmise = \"2025.1.0\"\nzulu = \"22\"\n\n[bootstrap.packages]\n\"brew-cask:beta\" = \"2.0\"\n\"brew:alpha\" = \"latest\"\n"
        );
    }

    #[test]
    fn compiles_machine_role_apps_into_the_mise_config() {
        let catalog = catalog(vec![]);
        let app_catalog = AppCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            apps: vec![App {
                id: "cursor".into(),
                title: "Cursor".into(),
                description: "editor".into(),
                providers: vec![provider(
                    ToolProviderKind::BootstrapBrewCask,
                    "cursor",
                    "latest",
                    vec![Platform::Darwin],
                )],
                unsupported_reason: None,
            }],
        };
        let mut selection = empty_selection();
        selection.app_ids = vec!["cursor".into()];

        let config = compile_mise(&selection, &catalog, &app_catalog, Platform::Darwin).unwrap();

        assert_eq!(
            config.bootstrap.packages.get("brew-cask:cursor"),
            Some(&"latest".into())
        );
    }

    #[test]
    fn compiles_stub_entries_only_for_mise_tools_on_the_target() {
        let catalog = catalog(vec![
            tool(
                "claude-code",
                vec![provider_with_command(
                    ToolProviderKind::MiseTool,
                    "claude-code",
                    "latest",
                    vec![Platform::Darwin, Platform::Linux],
                    "claude",
                )],
            ),
            tool(
                "codex",
                vec![provider(
                    ToolProviderKind::MiseTool,
                    "npm:@openai/codex",
                    "latest",
                    vec![Platform::Linux],
                )],
            ),
            tool(
                "git",
                vec![provider(
                    ToolProviderKind::BootstrapBrew,
                    "git",
                    "latest",
                    vec![Platform::Darwin, Platform::Linux],
                )],
            ),
        ]);
        let mut selection = empty_selection();
        selection.tool_ids = vec!["claude-code".into(), "codex".into(), "git".into()];

        let darwin = compile_stubs(&selection, &catalog, Platform::Darwin).unwrap();
        assert_eq!(
            darwin,
            vec![StubEntry {
                tool_id: "claude-code".into(),
                package: "claude-code".into(),
                command: "claude".into(),
            }],
            "native-provider and unsupported-on-target tools get no stub"
        );

        let linux = compile_stubs(&selection, &catalog, Platform::Linux).unwrap();
        assert_eq!(
            linux,
            vec![
                StubEntry {
                    tool_id: "claude-code".into(),
                    package: "claude-code".into(),
                    command: "claude".into(),
                },
                StubEntry {
                    tool_id: "codex".into(),
                    package: "npm:@openai/codex".into(),
                    command: "codex".into(),
                },
            ],
            "a missing explicit command falls back to deriving it from the package"
        );
    }

    #[test]
    fn serializes_apt_providers_into_bootstrap_packages() {
        let catalog = catalog(vec![tool(
            "git",
            vec![
                provider(
                    ToolProviderKind::BootstrapBrew,
                    "git",
                    "2.55.0",
                    vec![Platform::Darwin],
                ),
                provider(
                    ToolProviderKind::BootstrapApt,
                    "git",
                    "latest",
                    vec![Platform::Linux, Platform::Wsl],
                ),
            ],
        )]);
        let mut selection = empty_selection();
        selection.tool_ids = vec!["git".into()];

        let linux = compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Linux)
            .unwrap()
            .to_toml()
            .unwrap();
        assert!(
            linux.contains("\"apt:git\" = \"latest\""),
            "linux compilation must prefer the distro apt channel, got: {linux}"
        );
        assert!(
            !linux.contains("brew:"),
            "apt must win the linux priority over brew"
        );

        let darwin = compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Darwin)
            .unwrap()
            .to_toml()
            .unwrap();
        assert!(darwin.contains("\"brew:git\" = \"2.55.0\""));
        assert!(!darwin.contains("apt:"), "apt must never appear off-Linux");
    }

    #[test]
    fn prefers_mise_provider_over_bootstrap_brew() {
        let catalog = catalog(vec![tool(
            "node",
            vec![
                provider(
                    ToolProviderKind::BootstrapBrew,
                    "node",
                    "23",
                    vec![Platform::Darwin],
                ),
                provider(
                    ToolProviderKind::MiseTool,
                    "node",
                    "22",
                    vec![Platform::Darwin],
                ),
            ],
        )]);
        let mut selection = empty_selection();
        selection.tool_ids = vec!["node".into()];

        let config =
            compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Darwin).unwrap();

        assert_eq!(config.tools.get("node"), Some(&"22".into()));
        assert!(config.bootstrap.packages.is_empty());
    }

    #[test]
    fn routes_native_only_tools_to_native_operations() {
        let catalog = catalog(vec![
            tool(
                "windows-git",
                vec![provider(
                    ToolProviderKind::Winget,
                    "Git.Git",
                    "latest",
                    vec![Platform::Win32],
                )],
            ),
            tool(
                "windows-scoop",
                vec![provider(
                    ToolProviderKind::Scoop,
                    "scoop-package",
                    "latest",
                    vec![Platform::Win32],
                )],
            ),
        ]);
        let mut selection = empty_selection();
        selection.tool_ids = vec!["windows-git".into(), "windows-scoop".into()];

        let config = compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Win32)
            .expect("native-only tools are skipped from the mise config");
        assert!(config.tools.is_empty());
        assert!(config.bootstrap.packages.is_empty());

        let plan = compile_plan(
            "engineering-agents",
            &selection,
            &catalog,
            &empty_app_catalog(),
            &empty_integration_catalog(),
            &empty_mcp_catalog(),
            Platform::Win32,
        )
        .expect("native-only tools should compile into native operations");
        let operations = plan.operations.operations();

        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect::<Vec<_>>(),
            ["native-scoop:scoop-package", "native-winget:Git.Git"]
        );
        assert_eq!(operations[0].provider, OperationProvider::Scoop);
        assert_eq!(operations[1].provider, OperationProvider::Winget);
        assert_eq!(operations[1].resource_id, "Git.Git");
        assert_eq!(operations[1].verification.kind, VerificationKind::Command);
        assert_eq!(
            operations[1].verification.expected,
            "winget list --id Git.Git --exact"
        );
        assert!(operations.iter().all(|operation| {
            operation.ownership.owner == "powermise"
                && operation.reversibility.kind == ReversibilityKind::NotAutomated
        }));
    }

    #[test]
    fn rejects_tools_without_any_compatible_provider() {
        let catalog = catalog(vec![tool(
            "windows-only",
            vec![provider(
                ToolProviderKind::Winget,
                "windows-only",
                "latest",
                vec![Platform::Win32],
            )],
        )]);
        let mut selection = empty_selection();
        selection.tool_ids = vec!["windows-only".into()];

        let error =
            compile_mise(&selection, &catalog, &empty_app_catalog(), Platform::Darwin).unwrap_err();

        assert!(matches!(
            error,
            CompilerError::NoCompatibleProvider {
                kind: ResourceKind::Tool,
                id,
                target: Platform::Darwin,
                ..
            } if id == "windows-only"
        ));
    }

    #[test]
    fn rejects_missing_tools() {
        let mut selection = empty_selection();
        selection.tool_ids = vec!["missing".into()];

        let error = compile_mise(
            &selection,
            &catalog(vec![]),
            &empty_app_catalog(),
            Platform::Linux,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompilerError::MissingEntry {
                kind: ResourceKind::Tool,
                id,
            } if id == "missing"
        ));
    }

    #[test]
    fn compiles_provider_aware_operations_in_stable_order() {
        let catalog = catalog(vec![
            tool(
                "node",
                vec![provider(
                    ToolProviderKind::MiseTool,
                    "node",
                    "24",
                    vec![Platform::Darwin],
                )],
            ),
            tool(
                "github-cli",
                vec![provider(
                    ToolProviderKind::BootstrapBrew,
                    "gh",
                    "latest",
                    vec![Platform::Darwin],
                )],
            ),
        ]);
        let mut selection = empty_selection();
        selection.harness_ids = vec!["codex".into()];
        selection.tool_ids = vec!["node".into(), "github-cli".into()];

        let plan = compile_plan(
            "engineering-agents",
            &selection,
            &catalog,
            &empty_app_catalog(),
            &empty_integration_catalog(),
            &empty_mcp_catalog(),
            Platform::Darwin,
        )
        .expect("plan should compile");
        let operations = plan.operations.operations();

        assert_eq!(plan.schema_version, 1);
        assert_eq!(plan.selection, "engineering-agents");
        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect::<Vec<_>>(),
            ["mise-bootstrap:brew:gh", "mise-tool:node"]
        );
        assert_eq!(operations[0].provider, OperationProvider::MiseBootstrap);
        assert_eq!(operations[0].resource_id, "brew:gh");
        assert_eq!(operations[1].provider, OperationProvider::MiseTools);
        assert_eq!(operations[1].verification.expected, "node@24");
        assert!(operations.iter().all(|operation| {
            operation.ownership.owner == "powermise"
                && operation.ownership.selection == "engineering-agents"
                && operation.reversibility.kind == ReversibilityKind::DeclarationOnly
        }));
    }

    #[test]
    fn emits_config_adapter_operations_for_integrations_and_mcp_servers() {
        let selection = ResolvedSelection {
            harness_ids: vec!["claude".into()],
            tool_ids: vec![],
            desktop_app_ids: vec![],
            unavailable_desktop_ids: vec![],
            app_ids: vec![],
            integration_ids: vec!["claude-github".into()],
            mcp_server_ids: vec!["claude-context7".into()],
        };
        let integration_catalog = IntegrationCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            integrations: vec![Integration {
                id: "claude-github".into(),
                title: "Claude GitHub".into(),
                description: "integration".into(),
                providers: vec![config_provider(Platform::Darwin)],
                unsupported_reason: None,
            }],
        };
        let mcp_catalog = McpServerCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            mcp_servers: vec![McpServer {
                id: "claude-context7".into(),
                title: "Context7".into(),
                description: "mcp".into(),
                providers: vec![config_provider(Platform::Darwin)],
                unsupported_reason: None,
            }],
        };

        let plan = compile_plan(
            "engineering-agents",
            &selection,
            &catalog(vec![]),
            &empty_app_catalog(),
            &integration_catalog,
            &mcp_catalog,
            Platform::Darwin,
        )
        .expect("plan should compile");
        let operations = plan.operations.operations();

        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect::<Vec<_>>(),
            [
                "config-adapter:integration:claude-github",
                "config-adapter:mcp-server:claude-context7",
            ]
        );
        for operation in operations {
            assert_eq!(operation.provider, OperationProvider::ConfigAdapter);
            assert_eq!(operation.action, OperationAction::Configure);
            assert_eq!(operation.requested_state, "claude");
            assert_eq!(operation.verification.kind, VerificationKind::AdapterCheck);
            assert_eq!(
                operation.reversibility.kind,
                ReversibilityKind::RestorableSnapshot
            );
        }
    }

    #[test]
    fn rejects_integrations_without_a_target_config_adapter() {
        let mut selection = empty_selection();
        selection.integration_ids = vec!["claude-github".into()];
        let integration_catalog = IntegrationCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            integrations: vec![Integration {
                id: "claude-github".into(),
                title: "Claude GitHub".into(),
                description: "integration".into(),
                providers: vec![config_provider(Platform::Linux)],
                unsupported_reason: None,
            }],
        };

        let error = compile_plan(
            "engineering-agents",
            &selection,
            &catalog(vec![]),
            &empty_app_catalog(),
            &integration_catalog,
            &empty_mcp_catalog(),
            Platform::Darwin,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompilerError::NoCompatibleConfigProvider {
                kind: ResourceKind::Integration,
                id,
                target: Platform::Darwin,
                ..
            } if id == "claude-github"
        ));
    }

    #[test]
    fn rejects_missing_integrations() {
        let mut selection = empty_selection();
        selection.integration_ids = vec!["missing".into()];

        let error = compile_plan(
            "engineering-agents",
            &selection,
            &catalog(vec![]),
            &empty_app_catalog(),
            &empty_integration_catalog(),
            &empty_mcp_catalog(),
            Platform::Darwin,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompilerError::MissingEntry {
                kind: ResourceKind::Integration,
                id,
            } if id == "missing"
        ));
    }
}
