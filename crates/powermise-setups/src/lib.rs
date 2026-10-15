//! AI Setups: pick harnesses from the curated catalog, and powermise compiles
//! them into one mise config fragment, `conf.d/powermise-<name>.toml`.
//!
//! Everything after that is mise: `mise bootstrap` installs the fragment's
//! packages and tools, `mise bootstrap status` reports on them, and deleting
//! the fragment removes the setup from mise's config. powermise keeps only
//! the setup files themselves (`<powermise>/selections/<name>.toml`).
mod catalog;
mod imported;
mod mcp_files;
mod selection;

pub use imported::{
    ImportedConfig, imported_path, inspect_imported, read_imported, write_imported,
};
pub use mcp_files::{RemovedServer, expand_home, files_dir, remove_mcp_files, write_mcp_files};
pub use selection::SelectionStore;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use powermise_compiler::{CompileOptions, Compiled, compile_mcp, compile_with};
use powermise_domain::{MacArch, Platform};
use powermise_registry::{
    ResolvedSelection, load_app_catalog, load_catalog, load_integration_catalog, load_mcp_catalog,
    load_selection, load_tool_catalog, resolve_selection, validate_catalog, validate_registry,
    validate_selection,
};
use powermise_state::{
    GeneratedFileChange, SetupFragment, StateError, fragment_path, inspect_generated_file,
};
use thiserror::Error;

/// Where the catalog files live.
pub struct CatalogPaths {
    pub harnesses: PathBuf,
    pub tools: PathBuf,
    pub apps: PathBuf,
    pub integrations: PathBuf,
    pub mcp: PathBuf,
}

/// powermise's own directory.
pub struct PowermiseLayout {
    /// For example `~/.config/powermise`.
    pub root: PathBuf,
    /// Saved setups: `<root>/selections/<name>.toml`.
    pub selections: PathBuf,
    /// App state (the embedded catalog copy, the project list): `<root>/state`.
    pub state: PathBuf,
}

impl PowermiseLayout {
    /// Resolves the directory from an explicit override, then
    /// `POWERMISE_HOME`, `XDG_CONFIG_HOME/powermise`, or `~/.config/powermise`.
    pub fn resolve(override_dir: Option<PathBuf>) -> Result<Self, SetupsError> {
        let root = match override_dir {
            Some(path) => path,
            None => env::var_os("POWERMISE_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    env::var_os("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("powermise"))
                })
                .or_else(|| home_dir().map(|home| home.join(".config/powermise")))
                .ok_or(SetupsError::NoConfigDir)?,
        };
        Ok(Self {
            selections: root.join("selections"),
            state: root.join("state"),
            root,
        })
    }

    pub fn selection_path(&self, name: &str) -> PathBuf {
        self.selections.join(format!("{name}.toml"))
    }
}

/// The home directory as mise sees it: the user profile on Windows (mise
/// ignores `HOME` there, which Git Bash and MSYS set to other places), and
/// `HOME` everywhere else.
pub fn home_dir() -> Option<PathBuf> {
    let (first, second) = if cfg!(windows) {
        ("USERPROFILE", "HOME")
    } else {
        ("HOME", "USERPROFILE")
    };
    env::var_os(first)
        .or_else(|| env::var_os(second))
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// The machine's mise config directory: an override, then `MISE_CONFIG_DIR`,
/// `XDG_CONFIG_HOME/mise`, or `~/.config/mise` — the order mise uses.
pub fn resolve_mise_config_dir(override_dir: Option<PathBuf>) -> Result<PathBuf, SetupsError> {
    override_dir
        .or_else(|| env::var_os("MISE_CONFIG_DIR").map(PathBuf::from))
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("mise")))
        .or_else(|| home_dir().map(|home| home.join(".config/mise")))
        .ok_or(SetupsError::NoConfigDir)
}

/// The platform powermise is running on. WSL is its own target, detected from
/// the kernel release.
pub fn host_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Darwin
    } else if cfg!(windows) {
        Platform::Win32
    } else if fs::read_to_string("/proc/sys/kernel/osrelease")
        .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft"))
    {
        Platform::Wsl
    } else {
        Platform::Linux
    }
}

/// What this machine can install when a setup is compiled for `target`.
/// Previews for other platforms assume their usual hardware; only the Mac
/// powermise runs on can turn out to be an Intel Mac, where mise skips
/// Homebrew formulas.
pub fn compile_options(target: Platform) -> CompileOptions {
    let intel_mac =
        target == Platform::Darwin && host_platform() == Platform::Darwin && !apple_silicon();
    CompileOptions {
        homebrew_formulas: !intel_mac,
        mac_arch: if intel_mac {
            MacArch::X64
        } else {
            MacArch::Arm64
        },
    }
}

/// Whether this Mac has Apple Silicon — also when powermise itself runs as
/// an Intel build under Rosetta.
fn apple_silicon() -> bool {
    if cfg!(target_arch = "aarch64") {
        return true;
    }
    std::process::Command::new("sysctl")
        .args(["-n", "hw.optional.arm64"])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "1")
}

/// Validates a setup name before it's used in a file name.
pub(crate) fn validate_selection_name(name: &str) -> Result<(), SetupsError> {
    Ok(powermise_state::validate_selection_name(name)?)
}

/// A setup compiled for a target platform.
pub struct CompiledSetup {
    pub resolved: ResolvedSelection,
    pub compiled: Compiled,
    pub fragment: SetupFragment,
}

/// Loads and validates the catalog, resolves the setup at `selection_path`,
/// and compiles its mise fragment for `target`. MCP servers run through the
/// mise at `mise` (by name when it's unknown).
pub fn compile_setup(
    paths: &CatalogPaths,
    selection_path: &Path,
    target: Platform,
    mise: Option<&Path>,
) -> Result<CompiledSetup, SetupsError> {
    let catalog = load_catalog(&paths.harnesses)?;
    validate_catalog(&catalog)?;
    let tools = load_tool_catalog(&paths.tools)?;
    let apps = load_app_catalog(&paths.apps)?;
    let integrations = load_integration_catalog(&paths.integrations)?;
    let mcp = load_mcp_catalog(&paths.mcp)?;
    validate_registry(&catalog, &tools, &integrations, &mcp, &apps)?;
    let selection = load_selection(selection_path)?;
    validate_selection(&selection, &catalog, target)?;
    let resolved = resolve_selection(&selection, &catalog, &tools, &integrations, &mcp, target)?;
    let mut compiled = compile_with(&resolved, &tools, target, compile_options(target))?;
    let mise_command = mise.map_or_else(|| "mise".to_owned(), |path| path.display().to_string());
    compile_mcp(
        &mut compiled,
        &selection.name,
        &resolved.mcp_server_ids,
        &resolved.mcp_clients,
        &mcp,
        target,
        &mise_command,
    )?;
    let toml = compiled.config.to_toml()?;
    let read = |path: &Path| {
        fs::read(path).map_err(|source| SetupsError::Io {
            path: path.to_path_buf(),
            source,
        })
    };
    let catalog_sources = [
        read(&paths.harnesses)?,
        read(&paths.tools)?,
        read(&paths.apps)?,
        read(&paths.integrations)?,
        read(&paths.mcp)?,
    ];
    let fragment = SetupFragment::new(
        &selection.name,
        &toml,
        &read(selection_path)?,
        &catalog_sources
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>(),
    )?;
    Ok(CompiledSetup {
        resolved,
        compiled,
        fragment,
    })
}

/// How writing `fragment` would change the file on disk (create, update, or
/// unchanged, with a line diff). Writes nothing.
pub fn inspect_fragment(
    fragment: &SetupFragment,
    mise_config_dir: &Path,
) -> Result<GeneratedFileChange, SetupsError> {
    Ok(inspect_generated_file(
        &fragment.path(mise_config_dir),
        fragment.contents().as_bytes(),
    )?)
}

/// Deletes setup `name`'s fragment, if present. Returns whether it existed.
/// Installed packages and tools stay; `mise prune --tools` removes tool
/// versions nothing uses anymore.
pub fn remove_fragment(mise_config_dir: &Path, name: &str) -> Result<bool, SetupsError> {
    validate_selection_name(name)?;
    let path = fragment_path(mise_config_dir, name);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SetupsError::Io { path, source }),
    }
}

#[derive(Debug, Error)]
pub enum SetupsError {
    #[error("could not load or validate the catalog: {0}")]
    Registry(#[from] powermise_registry::RegistryError),
    #[error("could not compile the setup: {0}")]
    Compiler(#[from] powermise_compiler::CompilerError),
    #[error("{0}")]
    State(#[from] StateError),
    #[error("could not read or write `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not save `{path}`: {source}")]
    TomlSerialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("could not determine your home directory")]
    NoConfigDir,
    #[error("could not write the imported config: {0}")]
    Render(String),
    #[error("setup `{name}` does not exist")]
    NotFound { name: String },
    #[error("setup `{name}` already exists at `{path}`")]
    AlreadyExists { name: String, path: PathBuf },
    #[error("{message}")]
    Other { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_catalog() -> CatalogPaths {
        CatalogPaths::in_dir(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog/profiles"),
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "powermise-setups-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn the_engineering_agents_fixture_compiles_to_a_conf_d_fragment() {
        let selection = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../selections/engineering-agents.toml");
        let setup = compile_setup(&repo_catalog(), &selection, Platform::Darwin, None).unwrap();
        let mise = scratch("mise");
        assert!(
            setup
                .fragment
                .path(&mise)
                .ends_with("conf.d/powermise-engineering-agents.toml")
        );
        assert!(setup.fragment.contents().contains("[tools]"));
        // Homebrew never pins: everything Homebrew-backed is declared `latest`.
        for line in setup
            .fragment
            .contents()
            .lines()
            .filter(|line| line.starts_with("\"brew"))
        {
            assert!(line.ends_with("= \"latest\""), "{line}");
        }
        assert!(!setup.compiled.installs.is_empty());
        let _ = fs::remove_dir_all(mise);
    }

    #[test]
    fn windows_tools_compile_to_winget_or_scoop_packages() {
        let selection =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../selections/windows-fixture.toml");
        let setup = compile_setup(&repo_catalog(), &selection, Platform::Win32, None).unwrap();
        assert!(
            setup.fragment.contents().contains("\"winget:"),
            "{}",
            setup.fragment.contents()
        );
    }

    #[test]
    fn writing_inspecting_and_removing_a_fragment() {
        let selection = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../selections/engineering-agents.toml");
        let setup = compile_setup(&repo_catalog(), &selection, Platform::Darwin, None).unwrap();
        let mise = scratch("roundtrip");
        // A user's own config is never touched.
        fs::write(mise.join("config.toml"), "[tools]\nnode = \"24\"\n").unwrap();

        let change = inspect_fragment(&setup.fragment, &mise).unwrap();
        assert_eq!(
            change.kind,
            powermise_state::GeneratedFileChangeKind::Create
        );
        setup.fragment.write(&mise).unwrap();
        assert_eq!(
            inspect_fragment(&setup.fragment, &mise).unwrap().kind,
            powermise_state::GeneratedFileChangeKind::Unchanged
        );
        assert!(remove_fragment(&mise, "engineering-agents").unwrap());
        assert!(!remove_fragment(&mise, "engineering-agents").unwrap());
        assert_eq!(
            fs::read_to_string(mise.join("config.toml")).unwrap(),
            "[tools]\nnode = \"24\"\n"
        );
        assert!(remove_fragment(&mise, "../escape").is_err());
        let _ = fs::remove_dir_all(mise);
    }

    #[test]
    fn the_embedded_catalog_is_written_once_and_loads() {
        let root = scratch("layout");
        let layout = PowermiseLayout::resolve(Some(root.clone())).unwrap();
        let paths = CatalogPaths::embedded(&layout).unwrap();
        assert!(load_catalog(&paths.harnesses).is_ok());
        let _ = fs::remove_dir_all(root);
    }
}
