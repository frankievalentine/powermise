//! Compiles AI Setups picks into a mise config fragment.
//!
//! The output is plain mise configuration — `[tools]`, `[bootstrap.packages]`,
//! and `[env] _.path` for Homebrew formulas — that powermise writes to
//! `~/.config/mise/conf.d/powermise-<name>.toml`. mise does everything else:
//! `mise bootstrap` installs it, `mise ls` shows it, and deleting the file
//! removes it from the config.
use std::collections::{BTreeMap, BTreeSet};

mod mcp;
pub use mcp::{DotfileMerge, McpFile, compile_mcp, compile_mcp_plan, edit_id, files_dir_name};

use powermise_domain::{MacArch, Platform, ToolCatalog, ToolProvider, ToolProviderKind};
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
    /// Merge entries for agents' MCP configs, keyed `"<target>/<id>"`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dotfiles: BTreeMap<String, DotfileMerge>,
    /// `[env] _.path`: folders mise puts on PATH, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

impl CompiledMiseConfig {
    /// Serializes this configuration using the TOML serializer, with
    /// `paths` written the way mise's docs write them (`[env] _.path`).
    pub fn to_toml(&self) -> Result<String, CompilerError> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "BTreeMap::is_empty")]
            tools: &'a BTreeMap<String, String>,
            #[serde(skip_serializing_if = "BootstrapConfig::is_empty")]
            bootstrap: &'a BootstrapConfig,
            #[serde(skip_serializing_if = "BTreeMap::is_empty")]
            dotfiles: &'a BTreeMap<String, DotfileMerge>,
        }
        let mut out = toml::to_string(&Body {
            tools: &self.tools,
            bootstrap: &self.bootstrap,
            dotfiles: &self.dotfiles,
        })
        .map_err(CompilerError::Serialize)?;
        out.push_str(&env_path_toml(&self.paths));
        Ok(out)
    }
}

/// `[env] _.path = [...]` for `paths`, or nothing when there are none.
pub fn env_path_toml(paths: &[String]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let quoted: Vec<String> = paths
        .iter()
        .map(|path| toml::Value::String(path.clone()).to_string())
        .collect();
    format!("\n[env]\n_.path = [{}]\n", quoted.join(", "))
}

/// Where mise's `brew` manager installs on `target`: Apple Silicon Macs and
/// Linux (WSL included). It doesn't run on Intel Macs or Windows.
pub fn homebrew_prefix(target: Platform) -> Option<&'static str> {
    match target {
        Platform::Darwin => Some("/opt/homebrew"),
        Platform::Linux | Platform::Wsl => Some("/home/linuxbrew/.linuxbrew"),
        Platform::Win32 => None,
    }
}

/// The folders that put `packages`' Homebrew formulas on PATH on `target`:
/// each formula's own `<prefix>/opt/<formula>/bin`. mise installs them
/// without touching PATH, and `<prefix>/bin` would be the wrong fix: it also
/// holds their dependencies' commands (Homebrew's `python3`, `node`), and
/// `_.path` goes ahead of mise's own tools. The `opt` link exists for
/// keg-only formulas too, and follows upgrades.
pub fn homebrew_formula_paths<'a>(
    packages: impl IntoIterator<Item = &'a str>,
    target: Platform,
) -> Vec<String> {
    let Some(prefix) = homebrew_prefix(target) else {
        return Vec::new();
    };
    packages
        .into_iter()
        .filter_map(|key| key.strip_prefix("brew:"))
        // A tap's formula (`owner/tap/name`) is linked by its name.
        .map(|formula| formula.rsplit('/').next().unwrap_or(formula))
        .map(|formula| format!("{prefix}/opt/{formula}/bin"))
        .collect()
}

/// The bootstrap section of a mise configuration.
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct BootstrapConfig {
    /// `"manager:package" = "version"`, as mise's `[bootstrap.packages]`
    /// expects — or a table, for `macos-app` downloads.
    pub packages: BTreeMap<String, PackageSpec>,
}

/// One `[bootstrap.packages]` value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PackageSpec {
    Version(String),
    /// `macos-app`: <https://mise.jdx.dev/bootstrap/packages/macos-app.html>.
    Download {
        version: String,
        url: String,
        sha256: String,
        artifact: String,
    },
}

impl PackageSpec {
    pub fn version(&self) -> &str {
        match self {
            Self::Version(version) | Self::Download { version, .. } => version,
        }
    }
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
    /// MCP config sources to write beside the fragment.
    pub mcp_files: Vec<McpFile>,
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
    /// The Mac's architecture, for `macos-app` downloads.
    pub mac_arch: MacArch,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            homebrew_formulas: true,
            mac_arch: MacArch::Arm64,
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
    #[error("could not render an MCP config: {0}")]
    Render(String),
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
const PREFERENCE: [ToolProviderKind; 8] = [
    ToolProviderKind::MiseTool,
    ToolProviderKind::System,
    ToolProviderKind::BootstrapBrew,
    ToolProviderKind::BootstrapBrewCask,
    ToolProviderKind::MacosApp,
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
                provider.kind == kind
                    && provider.supported_platforms.contains(&target)
                    && provider.arch.is_none_or(|arch| arch == options.mac_arch)
            })
        })
}

/// The `[bootstrap.packages]` manager prefix for a system package provider.
fn package_manager(kind: &ToolProviderKind) -> Option<&'static str> {
    match kind {
        ToolProviderKind::BootstrapApt => Some("apt"),
        ToolProviderKind::BootstrapBrew => Some("brew"),
        ToolProviderKind::BootstrapBrewCask => Some("brew-cask"),
        ToolProviderKind::MacosApp => Some("macos-app"),
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

/// A Microsoft Store product ID (`9PLM9XGG6VKS`), which winget installs from
/// the Store. Store apps have no versions to pin, so they're declared
/// `latest` (any installed version satisfies it).
pub fn is_store_id(package: &str) -> bool {
    package.len() == 12
        && (package.starts_with('9') || package.starts_with("XP"))
        && package
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
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
    compiled.config.paths = homebrew_formula_paths(
        compiled
            .config
            .bootstrap
            .packages
            .keys()
            .map(String::as_str),
        target,
    );
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

/// Adds `key`, or checks that an earlier pick asked for the same version.
fn insert<V: Clone>(
    table: &mut BTreeMap<String, V>,
    key: String,
    value: V,
    version: impl Fn(&V) -> &str,
) -> Result<(), CompilerError> {
    match table.get(&key) {
        Some(existing) if version(existing) != version(&value) => {
            Err(CompilerError::PackageConflict {
                existing_version: version(existing).into(),
                replacement_version: version(&value).into(),
                key,
            })
        }
        Some(_) => Ok(()),
        None => {
            table.insert(key, value);
            Ok(())
        }
    }
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
    let key = match package_manager(&provider.kind) {
        None => {
            insert(
                &mut compiled.config.tools,
                provider.package.clone(),
                version.clone(),
                |v| v.as_str(),
            )?;
            provider.package.clone()
        }
        Some(manager) => {
            let key = format!("{manager}:{}", provider.package);
            let spec = match (&provider.url, &provider.sha256, &provider.artifact) {
                (Some(url), Some(sha256), Some(artifact))
                    if provider.kind == ToolProviderKind::MacosApp =>
                {
                    PackageSpec::Download {
                        version: version.clone(),
                        url: url.clone(),
                        sha256: sha256.clone(),
                        artifact: artifact.clone(),
                    }
                }
                _ => PackageSpec::Version(version.clone()),
            };
            insert(
                &mut compiled.config.bootstrap.packages,
                key.clone(),
                spec,
                PackageSpec::version,
            )?;
            key
        }
    };
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
            url: None,
            sha256: None,
            artifact: None,
            arch: None,
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
            mcp_clients: vec![],
        }
    }

    use Platform::{Darwin, Linux, Win32, Wsl};
    use ToolProviderKind::{
        BootstrapApt, BootstrapBrew, BootstrapBrewCask, MacosApp, MiseTool, Scoop, System, Winget,
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
            "[tools]\n\"npm:@openai/codex\" = \"0.51.0\"\nzulu = \"22\"\n\n[bootstrap.packages]\n\"brew-cask:beta\" = \"latest\"\n\"brew:alpha\" = \"latest\"\n\n[env]\n_.path = [\"/opt/homebrew/opt/alpha/bin\"]\n"
        );
    }

    #[test]
    fn homebrew_formulas_go_on_path_by_their_own_folders() {
        let catalog = tools(vec![
            tool(
                "jq",
                vec![provider(BootstrapBrew, "jq", "1", &[Darwin, Linux, Wsl])],
            ),
            tool(
                "pg",
                vec![provider(
                    BootstrapBrew,
                    "postgresql@17",
                    "1",
                    &[Darwin, Linux, Wsl],
                )],
            ),
            tool(
                "tapped",
                vec![provider(
                    BootstrapBrew,
                    "owner/tap/thing",
                    "1",
                    &[Darwin, Linux, Wsl],
                )],
            ),
            tool(
                "cask",
                vec![provider(BootstrapBrewCask, "zed", "1", &[Darwin])],
            ),
        ]);
        let paths = |target| {
            compile(&selection(&["jq", "pg", "tapped"]), &catalog, target)
                .unwrap()
                .config
                .paths
        };
        assert_eq!(
            paths(Darwin),
            [
                "/opt/homebrew/opt/jq/bin",
                "/opt/homebrew/opt/thing/bin",
                "/opt/homebrew/opt/postgresql@17/bin",
            ]
        );
        assert_eq!(paths(Linux)[0], "/home/linuxbrew/.linuxbrew/opt/jq/bin");
        assert_eq!(paths(Wsl)[0], "/home/linuxbrew/.linuxbrew/opt/jq/bin");
        // Casks are apps, not commands; nothing goes on PATH for them.
        let casks = compile(&selection(&["cask"]), &catalog, Darwin).unwrap();
        assert!(casks.config.paths.is_empty());
        assert!(!casks.config.to_toml().unwrap().contains("[env]"));
        // What mise reads: `[env] _.path`.
        let toml: toml::Table = compile(&selection(&["jq"]), &catalog, Darwin)
            .unwrap()
            .config
            .to_toml()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            toml["env"]["_"]["path"][0].as_str(),
            Some("/opt/homebrew/opt/jq/bin")
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
        assert_eq!(
            darwin.config.bootstrap.packages["brew:jq"].version(),
            "latest"
        );
        let linux = compile(&selection(&["apted"]), &catalog, Linux).unwrap();
        assert_eq!(
            linux.config.bootstrap.packages["apt:jq"].version(),
            "1.7.1-3"
        );
        let windows = compile(&selection(&["winget", "scoop"]), &catalog, Win32).unwrap();
        assert_eq!(
            windows.config.bootstrap.packages["winget:jqlang.jq"].version(),
            "1.8.1"
        );
        assert_eq!(
            windows.config.bootstrap.packages["scoop:zed"].version(),
            "0.200.0"
        );
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
            mac_arch: MacArch::X64,
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
    fn macos_app_downloads_pick_the_macs_architecture() {
        let download = |arch, sha: &str| ToolProvider {
            url: Some(format!("https://example.com/orca-{sha}.dmg")),
            sha256: Some(sha.repeat(64)),
            artifact: Some("Orca.app".into()),
            arch: Some(arch),
            ..provider(MacosApp, "orca", "1.4.224", &[Darwin])
        };
        let catalog = tools(vec![tool(
            "orca",
            vec![download(MacArch::Arm64, "a"), download(MacArch::X64, "b")],
        )]);
        let intel = CompileOptions {
            homebrew_formulas: false,
            mac_arch: MacArch::X64,
        };
        let compiled = compile_with(&selection(&["orca"]), &catalog, Darwin, intel).unwrap();
        let toml = compiled.config.to_toml().unwrap();
        assert!(
            toml.contains("[bootstrap.packages.\"macos-app:orca\"]"),
            "{toml}"
        );
        assert!(
            toml.contains("orca-b.dmg") && !toml.contains("orca-a.dmg"),
            "{toml}"
        );
        assert!(toml.contains("artifact = \"Orca.app\""), "{toml}");
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
