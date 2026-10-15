use std::{collections::BTreeSet, fs, path::Path};

use powermise_domain::{
    AppCatalog, Catalog, ConfigProvider, IntegrationCatalog, McpServerCatalog, Platform,
    SELECTION_SCHEMA_VERSION, Selection, ToolCatalog, ToolProvider, ToolProviderKind,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("could not parse {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("{0}")]
    Validation(String),
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedSelection {
    pub harness_ids: Vec<String>,
    /// CLI tooling plus, when opted in, the desktop companion tools that have
    /// a provider for the target.
    pub tool_ids: Vec<String>,
    /// The desktop companion tools included in `tool_ids`.
    pub desktop_app_ids: Vec<String>,
    /// Desktop companion tools the selection opted into that have no provider
    /// for the target. Reported, never fatal.
    pub unavailable_desktop_ids: Vec<String>,
    pub integration_ids: Vec<String>,
    pub mcp_server_ids: Vec<String>,
}

pub fn load_catalog(path: &Path) -> Result<Catalog, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&input).map_err(|source| RegistryError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Reads a selection file, upgrading older schema versions in memory.
pub fn load_selection(path: &Path) -> Result<Selection, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let mut selection: Selection =
        toml::from_str(&input).map_err(|source| RegistryError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    if !selection.upgrade() {
        return Err(RegistryError::Validation(format!(
            "`{}` uses selection schema version {}; this powermise reads up to {SELECTION_SCHEMA_VERSION}",
            path.display(),
            selection.schema_version
        )));
    }
    Ok(selection)
}

pub fn load_tool_catalog(path: &Path) -> Result<ToolCatalog, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&input).map_err(|source| RegistryError::Parse {
        path: path.display().to_string(),
        source,
    })
}

pub fn load_app_catalog(path: &Path) -> Result<AppCatalog, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&input).map_err(|source| RegistryError::Parse {
        path: path.display().to_string(),
        source,
    })
}

pub fn load_integration_catalog(path: &Path) -> Result<IntegrationCatalog, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&input).map_err(|source| RegistryError::Parse {
        path: path.display().to_string(),
        source,
    })
}

pub fn load_mcp_catalog(path: &Path) -> Result<McpServerCatalog, RegistryError> {
    let input = fs::read_to_string(path).map_err(|source| RegistryError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&input).map_err(|source| RegistryError::Parse {
        path: path.display().to_string(),
        source,
    })
}

pub fn validate_catalog(catalog: &Catalog) -> Result<(), RegistryError> {
    if catalog.schema_version != 1
        || catalog.catalog.trim().is_empty()
        || catalog.catalog_revision.trim().is_empty()
    {
        return Err(RegistryError::Validation(
            "catalog metadata is incomplete or uses an unsupported schema".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for harness in &catalog.harnesses {
        if harness.id.trim().is_empty()
            || harness.title.trim().is_empty()
            || harness.description.trim().is_empty()
            || harness.kind.trim().is_empty()
            || harness.category.trim().is_empty()
            || harness.source_file.trim().is_empty()
            || harness.supported_platforms.is_empty()
        {
            return Err(RegistryError::Validation(format!(
                "harness `{}` has incomplete metadata",
                harness.id
            )));
        }
        if !ids.insert(&harness.id) {
            return Err(RegistryError::Validation(format!(
                "duplicate harness ID `{}`",
                harness.id
            )));
        }
    }
    for harness in &catalog.harnesses {
        if let Some(parent) = &harness.extends
            && !ids.contains(parent)
        {
            return Err(RegistryError::Validation(format!(
                "harness `{}` extends unknown harness `{parent}`",
                harness.id
            )));
        }
    }
    Ok(())
}

pub fn validate_selection(
    selection: &Selection,
    catalog: &Catalog,
    target: Platform,
) -> Result<(), RegistryError> {
    if selection.schema_version != SELECTION_SCHEMA_VERSION
        || selection.name.trim().is_empty()
        || selection.catalog != catalog.catalog
        || selection.harnesses.is_empty()
    {
        return Err(RegistryError::Validation(
            "selection metadata is incomplete or references a different catalog".into(),
        ));
    }
    let mut requested = BTreeSet::new();
    for id in &selection.harnesses {
        if !requested.insert(id) {
            return Err(RegistryError::Validation(format!(
                "selection `{}` repeats harness `{id}`",
                selection.name
            )));
        }
        let harness = catalog
            .harnesses
            .iter()
            .find(|harness| harness.id == *id)
            .ok_or_else(|| {
                RegistryError::Validation(format!(
                    "selection `{}` references unknown harness `{id}`",
                    selection.name
                ))
            })?;
        if !harness.supported_platforms.contains(&target) {
            return Err(RegistryError::Validation(format!(
                "harness `{id}` is unsupported on {target:?}"
            )));
        }
    }
    Ok(())
}

pub fn validate_harness_tool_references(
    catalog: &Catalog,
    tool_catalog: &ToolCatalog,
) -> Result<(), RegistryError> {
    if tool_catalog.schema_version != 1
        || tool_catalog.catalog != catalog.catalog
        || tool_catalog.catalog_revision != catalog.catalog_revision
    {
        return Err(RegistryError::Validation(
            "tool catalog metadata is incomplete or references a different catalog".into(),
        ));
    }
    let tool_ids: BTreeSet<&str> = tool_catalog
        .tools
        .iter()
        .map(|tool| tool.id.as_str())
        .collect();
    if tool_ids.len() != tool_catalog.tools.len() {
        return Err(RegistryError::Validation(
            "tool catalog contains duplicate tool IDs".into(),
        ));
    }
    for tool in &tool_catalog.tools {
        if tool.id.trim().is_empty()
            || tool.title.trim().is_empty()
            || tool.description.trim().is_empty()
        {
            return Err(RegistryError::Validation(
                "tool catalog has incomplete tool metadata".into(),
            ));
        }
        match (&tool.providers[..], tool.unsupported_reason.as_deref()) {
            ([], Some(reason)) if !reason.trim().is_empty() => {}
            ([], _) => {
                return Err(RegistryError::Validation(format!(
                    "tool `{}` has no providers and no unsupported reason",
                    tool.id
                )));
            }
            (_, Some(_)) => {
                return Err(RegistryError::Validation(format!(
                    "tool `{}` has both providers and an unsupported reason",
                    tool.id
                )));
            }
            (_, None) => {}
        }
        for provider in &tool.providers {
            if provider.package.trim().is_empty()
                || provider.version.trim().is_empty()
                || provider.supported_platforms.is_empty()
            {
                return Err(RegistryError::Validation(format!(
                    "tool `{}` has an incomplete provider",
                    tool.id
                )));
            }
            if provider.version.eq_ignore_ascii_case("latest")
                && provider.kind != ToolProviderKind::BootstrapApt
            {
                return Err(RegistryError::Validation(format!(
                    "tool `{}` provider `{}` uses `latest`; \
                     catalog providers must pin an explicit resolved version \
                     (only bootstrap-apt tracks the distribution channel)",
                    tool.id, provider.package
                )));
            }
        }
    }
    for harness in &catalog.harnesses {
        for tool_id in harness
            .required_tool_ids
            .iter()
            .chain(harness.desktop_tool_ids.iter())
        {
            if !tool_ids.contains(tool_id.as_str()) {
                return Err(RegistryError::Validation(format!(
                    "harness `{}` references unknown tool `{tool_id}`",
                    harness.id
                )));
            }
        }
        if harness.desktop_required {
            for tool_id in &harness.desktop_tool_ids {
                let Some(tool) = tool_catalog.tools.iter().find(|tool| tool.id == *tool_id) else {
                    continue; // dangling reference already reported above
                };
                for platform in &harness.supported_platforms {
                    if !tool
                        .providers
                        .iter()
                        .any(|provider| provider.supported_platforms.contains(platform))
                    {
                        return Err(RegistryError::Validation(format!(
                            "desktop-first harness `{}` requires tool `{tool_id}` \
                             which has no provider for {platform:?}",
                            harness.id
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_catalog_metadata(
    owner: &str,
    schema_version: u32,
    catalog: &str,
    catalog_revision: &str,
    expected_catalog: &Catalog,
) -> Result<(), RegistryError> {
    if schema_version != 1
        || catalog != expected_catalog.catalog
        || catalog_revision != expected_catalog.catalog_revision
    {
        return Err(RegistryError::Validation(format!(
            "{owner} catalog metadata is incomplete or references a different catalog"
        )));
    }
    Ok(())
}

fn validate_tool_provider(owner: &str, provider: &ToolProvider) -> Result<(), RegistryError> {
    if provider.package.trim().is_empty()
        || provider.version.trim().is_empty()
        || provider.supported_platforms.is_empty()
    {
        return Err(RegistryError::Validation(format!(
            "{owner} has an incomplete provider"
        )));
    }
    if provider.version.eq_ignore_ascii_case("latest") {
        return Err(RegistryError::Validation(format!(
            "{owner} provider `{}` uses `latest`; \
             catalog providers must pin an explicit resolved version",
            provider.package
        )));
    }
    Ok(())
}

fn validate_config_provider(owner: &str, provider: &ConfigProvider) -> Result<(), RegistryError> {
    if provider.config_target.trim().is_empty() || provider.supported_platforms.is_empty() {
        return Err(RegistryError::Validation(format!(
            "{owner} has an incomplete config-adapter provider"
        )));
    }
    Ok(())
}

fn validate_provided_or_rejected(
    owner: &str,
    has_providers: bool,
    unsupported_reason: Option<&String>,
) -> Result<(), RegistryError> {
    match (has_providers, unsupported_reason) {
        (false, Some(reason)) if !reason.trim().is_empty() => Ok(()),
        (false, _) => Err(RegistryError::Validation(format!(
            "{owner} has no providers and no unsupported reason"
        ))),
        (true, Some(_)) => Err(RegistryError::Validation(format!(
            "{owner} has both providers and an unsupported reason"
        ))),
        (true, None) => Ok(()),
    }
}

pub fn validate_app_catalog(catalog: &Catalog, apps: &AppCatalog) -> Result<(), RegistryError> {
    validate_catalog_metadata(
        "app",
        apps.schema_version,
        &apps.catalog,
        &apps.catalog_revision,
        catalog,
    )?;
    let mut ids = BTreeSet::new();
    for app in &apps.apps {
        if app.id.trim().is_empty()
            || app.title.trim().is_empty()
            || app.description.trim().is_empty()
        {
            return Err(RegistryError::Validation(format!(
                "app `{}` has incomplete metadata",
                app.id
            )));
        }
        if !ids.insert(&app.id) {
            return Err(RegistryError::Validation(format!(
                "duplicate app ID `{}`",
                app.id
            )));
        }
        validate_provided_or_rejected(
            &format!("app `{}`", app.id),
            !app.providers.is_empty(),
            app.unsupported_reason.as_ref(),
        )?;
        for provider in &app.providers {
            validate_tool_provider(&format!("app `{}`", app.id), provider)?;
        }
    }
    Ok(())
}

pub fn validate_integration_catalog(
    catalog: &Catalog,
    integrations: &IntegrationCatalog,
) -> Result<(), RegistryError> {
    validate_catalog_metadata(
        "integration",
        integrations.schema_version,
        &integrations.catalog,
        &integrations.catalog_revision,
        catalog,
    )?;
    let mut ids = BTreeSet::new();
    for integration in &integrations.integrations {
        if integration.id.trim().is_empty()
            || integration.title.trim().is_empty()
            || integration.description.trim().is_empty()
        {
            return Err(RegistryError::Validation(format!(
                "integration `{}` has incomplete metadata",
                integration.id
            )));
        }
        if !ids.insert(&integration.id) {
            return Err(RegistryError::Validation(format!(
                "duplicate integration ID `{}`",
                integration.id
            )));
        }
        validate_provided_or_rejected(
            &format!("integration `{}`", integration.id),
            !integration.providers.is_empty(),
            integration.unsupported_reason.as_ref(),
        )?;
        for provider in &integration.providers {
            validate_config_provider(&format!("integration `{}`", integration.id), provider)?;
        }
    }
    Ok(())
}

pub fn validate_mcp_catalog(
    catalog: &Catalog,
    mcp: &McpServerCatalog,
) -> Result<(), RegistryError> {
    validate_catalog_metadata(
        "mcp server",
        mcp.schema_version,
        &mcp.catalog,
        &mcp.catalog_revision,
        catalog,
    )?;
    let mut ids = BTreeSet::new();
    for server in &mcp.mcp_servers {
        if server.id.trim().is_empty()
            || server.title.trim().is_empty()
            || server.description.trim().is_empty()
        {
            return Err(RegistryError::Validation(format!(
                "mcp server `{}` has incomplete metadata",
                server.id
            )));
        }
        if !ids.insert(&server.id) {
            return Err(RegistryError::Validation(format!(
                "duplicate mcp server ID `{}`",
                server.id
            )));
        }
        validate_provided_or_rejected(
            &format!("mcp server `{}`", server.id),
            !server.providers.is_empty(),
            server.unsupported_reason.as_ref(),
        )?;
        for provider in &server.providers {
            validate_config_provider(&format!("mcp server `{}`", server.id), provider)?;
        }
    }
    Ok(())
}

pub fn validate_harness_integration_references(
    catalog: &Catalog,
    integrations: &IntegrationCatalog,
) -> Result<(), RegistryError> {
    let ids: BTreeSet<&str> = integrations
        .integrations
        .iter()
        .map(|integration| integration.id.as_str())
        .collect();
    for harness in &catalog.harnesses {
        for integration_id in &harness.integration_ids {
            if !ids.contains(integration_id.as_str()) {
                return Err(RegistryError::Validation(format!(
                    "harness `{}` references unknown integration `{integration_id}`",
                    harness.id
                )));
            }
        }
    }
    Ok(())
}

pub fn validate_harness_mcp_references(
    catalog: &Catalog,
    mcp: &McpServerCatalog,
) -> Result<(), RegistryError> {
    let ids: BTreeSet<&str> = mcp
        .mcp_servers
        .iter()
        .map(|server| server.id.as_str())
        .collect();
    for harness in &catalog.harnesses {
        for mcp_server_id in &harness.mcp_server_ids {
            if !ids.contains(mcp_server_id.as_str()) {
                return Err(RegistryError::Validation(format!(
                    "harness `{}` references unknown mcp server `{mcp_server_id}`",
                    harness.id
                )));
            }
        }
    }
    Ok(())
}

pub fn validate_registry(
    catalog: &Catalog,
    tools: &ToolCatalog,
    integrations: &IntegrationCatalog,
    mcp: &McpServerCatalog,
    apps: &AppCatalog,
) -> Result<(), RegistryError> {
    validate_catalog(catalog)?;
    validate_harness_tool_references(catalog, tools)?;
    validate_app_catalog(catalog, apps)?;
    validate_integration_catalog(catalog, integrations)?;
    validate_mcp_catalog(catalog, mcp)?;
    validate_harness_integration_references(catalog, integrations)?;
    validate_harness_mcp_references(catalog, mcp)?;
    Ok(())
}

pub fn resolve_selection(
    selection: &Selection,
    catalog: &Catalog,
    tools: &ToolCatalog,
    integrations: &IntegrationCatalog,
    mcp: &McpServerCatalog,
    target: Platform,
) -> Result<ResolvedSelection, RegistryError> {
    validate_selection(selection, catalog, target)?;
    let mut harness_ids = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for id in &selection.harnesses {
        resolve_harness(catalog, id, target, &mut visiting, &mut harness_ids)?;
    }
    let harnesses = harness_ids
        .iter()
        .filter_map(|id| catalog.harnesses.iter().find(|harness| &harness.id == id))
        .collect::<Vec<_>>();

    // Desktop companion tools are included when the selection opts in or when
    // the harness is desktop-first (`desktop_required`). A companion joins the resolved tool set
    // only when some provider supports the target; opted-in companions
    // without one are reported as unavailable instead of failing the compile.
    let mut desktop_app_ids = BTreeSet::new();
    let mut unavailable_desktop_ids = Vec::new();
    let desktop_opt_in = selection.preferences.include_desktop_apps;
    for harness in &harnesses {
        if harness.desktop_tool_ids.is_empty() {
            continue;
        }
        let wants_companions = desktop_opt_in || harness.desktop_required;
        for tool_id in &harness.desktop_tool_ids {
            if !wants_companions || desktop_app_ids.contains(tool_id) {
                continue;
            }
            let supported = tools
                .tools
                .iter()
                .find(|tool| &tool.id == tool_id)
                .is_some_and(|tool| {
                    tool.providers
                        .iter()
                        .any(|provider| provider.supported_platforms.contains(&target))
                });
            if supported {
                desktop_app_ids.insert(tool_id.clone());
            } else {
                unavailable_desktop_ids.push(tool_id.clone());
            }
        }
    }

    let mut tool_ids: BTreeSet<String> = harnesses
        .iter()
        .flat_map(|harness| harness.required_tool_ids.iter())
        .cloned()
        .collect();
    tool_ids.extend(desktop_app_ids.iter().cloned());
    let tool_ids = tool_ids.into_iter().collect();
    let integration_ids = resolve_config_ids(
        "integration",
        harnesses
            .iter()
            .flat_map(|harness| harness.integration_ids.iter())
            .cloned(),
        &integrations
            .integrations
            .iter()
            .map(|entry| {
                (
                    &entry.id,
                    &entry.providers[..],
                    entry.unsupported_reason.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        target,
    )?;
    let mcp_server_ids = resolve_config_ids(
        "mcp server",
        harnesses
            .iter()
            .flat_map(|harness| harness.mcp_server_ids.iter())
            .cloned(),
        &mcp.mcp_servers
            .iter()
            .map(|entry| {
                (
                    &entry.id,
                    &entry.providers[..],
                    entry.unsupported_reason.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        target,
    )?;
    Ok(ResolvedSelection {
        harness_ids: harness_ids.into_iter().collect(),
        tool_ids,
        desktop_app_ids: desktop_app_ids.into_iter().collect(),
        unavailable_desktop_ids,
        integration_ids,
        mcp_server_ids,
    })
}

fn resolve_config_ids<'a>(
    kind: &str,
    requested: impl Iterator<Item = String>,
    entries: &[(&'a String, &'a [ConfigProvider], Option<&'a str>)],
    target: Platform,
) -> Result<Vec<String>, RegistryError> {
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for id in requested {
        if !seen.insert(id.clone()) {
            continue;
        }
        let (_, providers, unsupported_reason) = entries
            .iter()
            .find(|(entry_id, _, _)| *entry_id == &id)
            .ok_or_else(|| {
                RegistryError::Validation(format!("{kind} `{id}` is not defined in the registry"))
            })?;
        if !providers
            .iter()
            .any(|provider| provider.supported_platforms.contains(&target))
        {
            return Err(RegistryError::Validation(format!(
                "{kind} `{id}` has no config-adapter provider for {target:?}{}",
                unsupported_reason
                    .map(|reason| format!(": {reason}"))
                    .unwrap_or_default()
            )));
        }
        resolved.push(id);
    }
    Ok(resolved)
}

fn resolve_harness(
    catalog: &Catalog,
    id: &str,
    target: Platform,
    visiting: &mut BTreeSet<String>,
    resolved: &mut BTreeSet<String>,
) -> Result<(), RegistryError> {
    if resolved.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_owned()) {
        return Err(RegistryError::Validation(format!(
            "harness inheritance cycle includes `{id}`"
        )));
    }
    let harness = catalog
        .harnesses
        .iter()
        .find(|harness| harness.id == id)
        .ok_or_else(|| RegistryError::Validation(format!("unknown harness `{id}`")))?;
    if !harness.supported_platforms.contains(&target) {
        return Err(RegistryError::Validation(format!(
            "harness `{id}` is unsupported on {target:?}"
        )));
    }
    if let Some(parent) = &harness.extends {
        resolve_harness(catalog, parent, target, visiting, resolved)?;
    }
    visiting.remove(id);
    resolved.insert(id.to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        toml::from_str(include_str!("../../../catalog/profiles/harnesses.toml")).unwrap()
    }

    fn selection() -> Selection {
        toml::from_str(include_str!("../../../selections/engineering-agents.toml")).unwrap()
    }

    fn integrations() -> IntegrationCatalog {
        toml::from_str(include_str!("../../../catalog/profiles/integrations.toml")).unwrap()
    }

    fn mcp() -> McpServerCatalog {
        toml::from_str(include_str!("../../../catalog/profiles/mcp.toml")).unwrap()
    }

    fn tools() -> ToolCatalog {
        toml::from_str(include_str!("../../../catalog/profiles/tools.toml")).unwrap()
    }

    fn apps() -> AppCatalog {
        toml::from_str(include_str!("../../../catalog/profiles/apps.toml")).unwrap()
    }

    #[test]
    fn version_1_selections_load_as_version_2_and_unknown_versions_fail() {
        let directory = std::env::temp_dir().join(format!(
            "powermise-registry-selection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        // Written by the retired CLI: roles and convergence preferences that
        // version 2 dropped.
        let old = directory.join("old.toml");
        fs::write(
            &old,
            r#"schema_version = 1
name = "old"
catalog = "powermise-profiles"
harnesses = ["claude"]

[machine_roles.macbook]
include_apps = ["cursor"]

[preferences]
auto_prune = true
update_system_packages = true
include_desktop_apps = true
"#,
        )
        .unwrap();
        let selection = load_selection(&old).unwrap();
        assert_eq!(selection.schema_version, SELECTION_SCHEMA_VERSION);
        assert!(selection.preferences.include_desktop_apps);
        assert!(validate_selection(&selection, &catalog(), Platform::Darwin).is_ok());
        let saved = toml::to_string(&selection).unwrap();
        assert!(saved.contains("schema_version = 2"), "{saved}");
        assert!(!saved.contains("machine_roles") && !saved.contains("auto_prune"));

        let future = directory.join("future.toml");
        fs::write(
            &future,
            "schema_version = 9\nname = \"future\"\ncatalog = \"powermise-profiles\"\nharnesses = [\"claude\"]\n",
        )
        .unwrap();
        let error = load_selection(&future).unwrap_err();
        assert!(error.to_string().contains("schema version 9"), "{error}");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn selection_rejects_a_harness_on_an_unsupported_target() {
        let catalog = catalog();
        // `conductor` is darwin-only (its desktop channel is a Homebrew cask).
        let selection: Selection = toml::from_str(
            r#"
schema_version = 2
name = "conductor-only"
catalog = "powermise-profiles"
harnesses = ["conductor"]
"#,
        )
        .unwrap();

        validate_catalog(&catalog).unwrap();
        let error = validate_selection(&selection, &catalog, Platform::Win32).unwrap_err();

        assert!(error.to_string().contains("conductor"));
    }

    #[test]
    fn resolution_includes_inherited_harness_tools() {
        let catalog = catalog();
        let selection = selection();
        let integrations = integrations();
        let mcp = mcp();

        let resolved = resolve_selection(
            &selection,
            &catalog,
            &tools(),
            &integrations,
            &mcp,
            Platform::Darwin,
        )
        .unwrap();

        assert_eq!(resolved.harness_ids, ["base", "claude", "codex", "gemini"]);
        assert!(resolved.tool_ids.contains(&"git".to_owned()));
        assert!(resolved.tool_ids.contains(&"codex".to_owned()));
        assert_eq!(
            resolved.integration_ids,
            [
                "claude-github",
                "claude-mem",
                "codex-gmail",
                "gemini-workspace"
            ]
        );
        assert_eq!(
            resolved.mcp_server_ids,
            [
                "claude-context7",
                "claude-sequential-thinking",
                "codex-context7",
                "codex-sequential-thinking",
                "gemini-context7",
                "gemini-sequential-thinking",
            ]
        );
    }

    #[test]
    fn desktop_first_harnesses_install_companions_by_default() {
        let catalog = catalog();
        let tools = tools();
        let selection: Selection = toml::from_str(
            r#"
schema_version = 2
name = "conductor-only"
catalog = "powermise-profiles"
harnesses = ["conductor"]
"#,
        )
        .unwrap();

        // No include_desktop_apps preference: the desktop-first harness still
        // brings its companion.
        let resolved = resolve_selection(
            &selection,
            &catalog,
            &tools,
            &integrations(),
            &mcp(),
            Platform::Darwin,
        )
        .unwrap();
        assert_eq!(resolved.desktop_app_ids, ["conductor"]);
        assert!(resolved.tool_ids.contains(&"conductor".to_owned()));
        assert!(resolved.unavailable_desktop_ids.is_empty());
    }

    #[test]
    fn desktop_first_harness_must_have_providers_on_every_claimed_platform() {
        let catalog = catalog();
        let mut tools = tools();
        // Drop the scoop provider from zed: the harness claims win32 and is
        // desktop-first, so validation must fail loudly.
        tools
            .tools
            .iter_mut()
            .find(|tool| tool.id == "zed")
            .unwrap()
            .providers
            .retain(|provider| provider.supported_platforms.contains(&Platform::Darwin));

        let error =
            validate_registry(&catalog, &tools, &integrations(), &mcp(), &apps()).unwrap_err();
        assert!(error.to_string().contains("desktop-first harness `zed`"));
    }

    #[test]
    fn desktop_apps_resolve_only_when_opted_in_and_available() {
        let catalog = catalog();
        let tools = tools();
        let mut selection = selection();

        let resolved = resolve_selection(
            &selection,
            &catalog,
            &tools,
            &integrations(),
            &mcp(),
            Platform::Darwin,
        )
        .unwrap();
        assert!(resolved.desktop_app_ids.is_empty());
        assert!(!resolved.tool_ids.contains(&"claude-app".to_owned()));

        selection.preferences.include_desktop_apps = true;

        let darwin = resolve_selection(
            &selection,
            &catalog,
            &tools,
            &integrations(),
            &mcp(),
            Platform::Darwin,
        )
        .unwrap();
        assert_eq!(darwin.desktop_app_ids, ["claude-app", "codex-app"]);
        assert!(darwin.tool_ids.contains(&"claude-app".to_owned()));
        assert!(darwin.unavailable_desktop_ids.is_empty());

        let linux = resolve_selection(
            &selection,
            &catalog,
            &tools,
            &integrations(),
            &mcp(),
            Platform::Linux,
        )
        .unwrap();
        assert!(linux.desktop_app_ids.is_empty());
        assert_eq!(linux.unavailable_desktop_ids, ["claude-app", "codex-app"]);
        assert!(
            !linux.tool_ids.contains(&"claude-app".to_owned()),
            "unavailable desktop apps must never enter the tool set"
        );
    }

    #[test]
    fn unpinned_latest_providers_are_rejected_except_apt() {
        let catalog = catalog();
        let mut pinned_catalog = tools();
        pinned_catalog
            .tools
            .iter_mut()
            .find(|tool| tool.id == "git")
            .unwrap()
            .providers
            .insert(
                0,
                ToolProvider {
                    kind: ToolProviderKind::BootstrapBrew,
                    package: "git".into(),
                    version: "latest".into(),
                    command: None,
                    supported_platforms: vec![Platform::Darwin],
                },
            );

        let error = validate_registry(&catalog, &pinned_catalog, &integrations(), &mcp(), &apps())
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must pin an explicit resolved version")
        );

        // The apt provider's deliberate distro-channel "latest" stays valid.
        let mut apt_catalog = tools();
        apt_catalog
            .tools
            .iter_mut()
            .find(|tool| tool.id == "git")
            .unwrap()
            .providers
            .iter_mut()
            .find(|provider| provider.kind == ToolProviderKind::BootstrapApt)
            .unwrap()
            .version = "latest".into();
        validate_registry(&catalog, &apt_catalog, &integrations(), &mcp(), &apps()).unwrap();
    }

    #[test]
    fn curated_tool_catalog_covers_every_harness_requirement() {
        let catalog = catalog();
        let tools: ToolCatalog =
            toml::from_str(include_str!("../../../catalog/profiles/tools.toml")).unwrap();

        validate_harness_tool_references(&catalog, &tools).unwrap();
    }

    #[test]
    fn registry_validates_curated_catalog_slices() {
        let catalog = catalog();
        let tools: ToolCatalog =
            toml::from_str(include_str!("../../../catalog/profiles/tools.toml")).unwrap();
        let integrations = integrations();
        let mcp = mcp();
        let apps = apps();

        validate_registry(&catalog, &tools, &integrations, &mcp, &apps).unwrap();
    }

    #[test]
    fn harness_references_an_unknown_integration() {
        let catalog = catalog();
        let mut integrations = integrations();
        integrations
            .integrations
            .retain(|entry| entry.id != "claude-github");

        let error = validate_harness_integration_references(&catalog, &integrations).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unknown integration `claude-github`")
        );
    }

    #[test]
    fn harness_references_an_unknown_mcp_server() {
        let catalog = catalog();
        let mut mcp = mcp();
        mcp.mcp_servers
            .retain(|entry| entry.id != "codex-sequential-thinking");

        let error = validate_harness_mcp_references(&catalog, &mcp).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unknown mcp server `codex-sequential-thinking`")
        );
    }

    #[test]
    fn resolution_rejects_an_integration_without_a_target_provider() {
        let catalog = catalog();
        let selection = selection();
        let mut integrations = integrations();
        for entry in &mut integrations.integrations {
            entry.providers.clear();
            entry.unsupported_reason = Some("no verified adapter on any target".into());
        }

        let error = resolve_selection(
            &selection,
            &catalog,
            &tools(),
            &integrations,
            &mcp(),
            Platform::Darwin,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("integration `claude-github` has no config-adapter provider")
        );
    }
}
