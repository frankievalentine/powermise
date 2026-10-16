//! Coming from another version manager: the versions it installed, which one
//! it had as your default, startup-file lines that still turn it on, and the
//! command-line packages installed into those versions.
//!
//! Everything here reads files and folder names in their documented default
//! locations (or where their environment variable points); nothing runs.
//! mise imports nvm, nodenv, pyenv, uv, and Homebrew installs itself
//! (`mise sync`); the rest are linked one version at a time (`mise link`).
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::MiseCommand;

/// A version manager (or Homebrew) with installs mise can use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportSource {
    Nvm,
    Nodenv,
    NodeBrew,
    Pyenv,
    Uv,
    RubyBrew,
    Fnm,
    Volta,
    N,
    NvmWindows,
    Rbenv,
    Rvm,
    Chruby,
    Goenv,
    Asdf,
}

impl ImportSource {
    pub const ALL: [Self; 15] = [
        Self::Nvm,
        Self::Nodenv,
        Self::NodeBrew,
        Self::Pyenv,
        Self::Uv,
        Self::RubyBrew,
        Self::Fnm,
        Self::Volta,
        Self::N,
        Self::NvmWindows,
        Self::Rbenv,
        Self::Rvm,
        Self::Chruby,
        Self::Goenv,
        Self::Asdf,
    ];

    /// The `mise sync <tool> --<flag>` that imports it, for sources mise
    /// imports itself; `None` means each version is linked with `mise link`.
    pub fn sync_flag(self) -> Option<&'static str> {
        match self {
            Self::Nvm => Some("nvm"),
            Self::Nodenv => Some("nodenv"),
            Self::NodeBrew | Self::RubyBrew => Some("brew"),
            Self::Pyenv => Some("pyenv"),
            Self::Uv => Some("uv"),
            _ => None,
        }
    }
}

/// One installed version, and its folder for linking.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportVersion {
    pub version: String,
    pub path: PathBuf,
    /// mise already has this version (installed, linked, or synced).
    pub in_mise: bool,
}

/// One tool's installs from one manager.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedImport {
    pub source: ImportSource,
    /// The mise tool (`node`, `python`, …).
    pub tool: String,
    pub versions: Vec<ImportVersion>,
    /// The version the manager had as your default, when it's installed.
    pub default_version: Option<String>,
}

impl DetectedImport {
    /// What brings the versions mise doesn't have yet into mise.
    pub fn commands(&self) -> Vec<MiseCommand> {
        if !self.versions.iter().any(|version| !version.in_mise) {
            return Vec::new();
        }
        match self.source.sync_flag() {
            Some(flag) => vec![MiseCommand::sync_with(&self.tool, flag)],
            None => self
                .versions
                .iter()
                .filter(|version| !version.in_mise)
                .filter_map(|version| {
                    MiseCommand::link(&self.tool, &version.version, &version.path).ok()
                })
                .collect(),
        }
    }
}

/// A directory from an environment variable (`NVM_DIR`, `PIPX_HOME`, …).
/// Tests see none of the machine's variables, so a CI runner's own nvm or
/// pipx never leaks into them.
fn env_dir(name: &str) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Visible subdirectories: `(name, path)`.
fn dirs(directory: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .filter(|(name, _)| !name.starts_with('.'))
        .collect();
    found.sort();
    found
}

/// A version manager's one-line version file (`~/.pyenv/version`).
fn read_line(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
}

/// Versions named like a version: `v20.11.1` → `20.11.1`.
fn versions_in(directory: &Path, strip: &[&str]) -> Vec<(String, PathBuf)> {
    dirs(directory)
        .into_iter()
        .map(|(name, path)| {
            let version = strip
                .iter()
                .fold(name.as_str(), |name, prefix| {
                    name.strip_prefix(prefix).unwrap_or(name)
                })
                .to_owned();
            (version, path)
        })
        .filter(|(version, _)| version.starts_with(|c: char| c.is_ascii_digit()))
        .collect()
}

/// The installed version a default like `20`, `v20.11.1`, or `ruby-3.2.2`
/// means: the newest installed version it's a prefix of.
fn resolve_default(wanted: &str, versions: &[(String, PathBuf)]) -> Option<String> {
    let wanted = wanted
        .trim()
        .trim_start_matches('v')
        .trim_start_matches("ruby-")
        .trim_start_matches("node-");
    if !wanted.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    versions
        .iter()
        .map(|(version, _)| version)
        .filter(|version| *version == wanted || version.starts_with(&format!("{wanted}.")))
        .max_by(|a, b| compare_versions(a, b))
        .cloned()
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |text: &str| -> Vec<u64> {
        text.split(|c: char| !c.is_ascii_digit())
            .filter_map(|part| part.parse().ok())
            .collect()
    };
    parts(a).cmp(&parts(b))
}

/// Homebrew's versions of a formula and its versioned variants
/// (`node`, `node@22`), as `(version, path)`.
fn homebrew(formula: &str) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for prefix in ["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"] {
        let cellar = Path::new(prefix).join("Cellar");
        for (name, path) in dirs(&cellar) {
            if name == formula || name.starts_with(&format!("{formula}@")) {
                for (version, path) in dirs(&path) {
                    // Revision suffixes (`22.9.0_1`) aren't part of the version.
                    let version = version.split('_').next().unwrap_or_default().to_owned();
                    found.push((version, path));
                }
            }
        }
    }
    found
}

/// asdf plugin names that differ from mise's.
fn asdf_tool(plugin: &str) -> &str {
    match plugin {
        "nodejs" => "node",
        "golang" => "go",
        other => other,
    }
}

/// Every manager's installs this computer has, with the versions mise
/// already has (from `installs`, mise's installs folder) marked.
pub fn detect_imports(home: &Path, installs: Option<&Path>) -> Vec<DetectedImport> {
    let mut found = Vec::new();
    let mut push = |source: ImportSource,
                    tool: &str,
                    versions: Vec<(String, PathBuf)>,
                    default: Option<String>| {
        let mut versions = versions;
        versions.sort_by(|a, b| compare_versions(&a.0, &b.0));
        versions.dedup_by(|a, b| a.0 == b.0);
        if versions.is_empty() {
            return;
        }
        let default_version = default.and_then(|wanted| resolve_default(&wanted, &versions));
        found.push(DetectedImport {
            source,
            tool: tool.to_owned(),
            versions: versions
                .into_iter()
                .map(|(version, path)| ImportVersion {
                    in_mise: installs
                        .is_some_and(|installs| installs.join(tool).join(&version).exists()),
                    version,
                    path,
                })
                .collect(),
            default_version,
        });
    };

    // nvm (macOS and Linux).
    let nvm = env_dir("NVM_DIR").unwrap_or_else(|| home.join(".nvm"));
    push(
        ImportSource::Nvm,
        "node",
        versions_in(&nvm.join("versions/node"), &["v"]),
        read_line(&nvm.join("alias/default")),
    );
    // nodenv.
    let nodenv = env_dir("NODENV_ROOT").unwrap_or_else(|| home.join(".nodenv"));
    push(
        ImportSource::Nodenv,
        "node",
        versions_in(&nodenv.join("versions"), &[]),
        read_line(&nodenv.join("version")),
    );
    push(ImportSource::NodeBrew, "node", homebrew("node"), None);
    // fnm keeps each version's files in `<version>/installation`.
    let fnm = env_dir("FNM_DIR")
        .into_iter()
        .chain([
            home.join(".local/share/fnm"),
            home.join("Library/Application Support/fnm"),
            home.join("AppData/Roaming/fnm"),
        ])
        .find(|dir| dir.join("node-versions").is_dir());
    if let Some(fnm) = fnm {
        let versions = versions_in(&fnm.join("node-versions"), &["v"])
            .into_iter()
            .map(|(version, path)| (version, path.join("installation")))
            .filter(|(_, path)| path.is_dir())
            .collect();
        // `aliases/default` links to the default's `installation` folder.
        let default = fs::read_link(fnm.join("aliases/default"))
            .ok()
            .and_then(|target| target.parent()?.file_name()?.to_str().map(str::to_owned));
        push(ImportSource::Fnm, "node", versions, default);
    }
    // Volta.
    let volta = env_dir("VOLTA_HOME").unwrap_or_else(|| {
        if cfg!(windows) {
            env_dir("LOCALAPPDATA")
                .unwrap_or_else(|| home.join("AppData/Local"))
                .join("Volta")
        } else {
            home.join(".volta")
        }
    });
    let volta_default = fs::read_to_string(volta.join("tools/user/platform.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|platform| {
            platform
                .pointer("/node/runtime")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    push(
        ImportSource::Volta,
        "node",
        versions_in(&volta.join("tools/image/node"), &[]),
        volta_default,
    );
    // n.
    let n = env_dir("N_PREFIX").unwrap_or_else(|| PathBuf::from("/usr/local"));
    push(
        ImportSource::N,
        "node",
        versions_in(&n.join("n/versions/node"), &[]),
        None,
    );
    // nvm-windows keeps versions as `v20.11.1` in NVM_HOME.
    if cfg!(windows) {
        let nvm_windows = env_dir("NVM_HOME").unwrap_or_else(|| {
            env_dir("APPDATA")
                .unwrap_or_else(|| home.join("AppData/Roaming"))
                .join("nvm")
        });
        push(
            ImportSource::NvmWindows,
            "node",
            versions_in(&nvm_windows, &["v"]),
            None,
        );
    }

    // pyenv.
    let pyenv = env_dir("PYENV_ROOT").unwrap_or_else(|| home.join(".pyenv"));
    push(
        ImportSource::Pyenv,
        "python",
        versions_in(&pyenv.join("versions"), &[]),
        read_line(&pyenv.join("version")),
    );
    // uv: `cpython-3.12.4-macos-aarch64-none` → `3.12.4`.
    let uv = env_dir("UV_PYTHON_INSTALL_DIR").unwrap_or_else(|| {
        if cfg!(windows) {
            env_dir("APPDATA")
                .unwrap_or_else(|| home.join("AppData/Roaming"))
                .join("uv/data/python")
        } else {
            env_dir("XDG_DATA_HOME")
                .unwrap_or_else(|| home.join(".local/share"))
                .join("uv/python")
        }
    });
    let uv_versions = dirs(&uv)
        .into_iter()
        .filter_map(|(name, path)| {
            let version = name.strip_prefix("cpython-")?.split('-').next()?.to_owned();
            // `cpython-3.12-…` is uv's link to the newest 3.12, not a version.
            (version.matches('.').count() >= 2).then_some((version, path))
        })
        .collect();
    push(ImportSource::Uv, "python", uv_versions, None);

    // Ruby: Homebrew, rbenv, rvm (`ruby-3.2.2`), chruby / ruby-install.
    push(ImportSource::RubyBrew, "ruby", homebrew("ruby"), None);
    let rbenv = env_dir("RBENV_ROOT").unwrap_or_else(|| home.join(".rbenv"));
    push(
        ImportSource::Rbenv,
        "ruby",
        versions_in(&rbenv.join("versions"), &[]),
        read_line(&rbenv.join("version")),
    );
    let rvm = env_dir("rvm_path").unwrap_or_else(|| home.join(".rvm"));
    let rvm_default = fs::read_to_string(rvm.join("config/alias"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("default="))
                .map(str::to_owned)
        });
    push(
        ImportSource::Rvm,
        "ruby",
        versions_in(&rvm.join("rubies"), &["ruby-"]),
        rvm_default,
    );
    push(
        ImportSource::Chruby,
        "ruby",
        versions_in(&home.join(".rubies"), &["ruby-"]),
        None,
    );

    // goenv.
    let goenv = env_dir("GOENV_ROOT").unwrap_or_else(|| home.join(".goenv"));
    push(
        ImportSource::Goenv,
        "go",
        versions_in(&goenv.join("versions"), &[]),
        read_line(&goenv.join("version")),
    );

    // asdf: every plugin's installs, and its global `~/.tool-versions`.
    let asdf = env_dir("ASDF_DATA_DIR").unwrap_or_else(|| home.join(".asdf"));
    let tool_versions = fs::read_to_string(
        env_dir("ASDF_DEFAULT_TOOL_VERSIONS_FILENAME")
            .map(|name| home.join(name))
            .unwrap_or_else(|| home.join(".tool-versions")),
    )
    .unwrap_or_default();
    for (plugin, path) in dirs(&asdf.join("installs")) {
        let default = tool_versions.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some(plugin.as_str())).then(|| words.next().map(str::to_owned))?
        });
        push(
            ImportSource::Asdf,
            asdf_tool(&plugin),
            versions_in(&path, &[]),
            default,
        );
    }
    found
}

// ---------------------------------------------------------------------------
// Startup files that still turn another manager on
// ---------------------------------------------------------------------------

/// A line in a shell startup file that turns on another version manager,
/// which fights mise over `PATH`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellConflict {
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
    pub text: String,
    /// `nvm`, `pyenv`, …
    pub manager: &'static str,
}

/// What each manager's setup line contains.
const CONFLICT_PATTERNS: &[(&str, &[&str])] = &[
    ("nvm", &["nvm.sh", "NVM_DIR/nvm", "nvm use"]),
    ("nodenv", &["nodenv init"]),
    ("fnm", &["fnm env"]),
    ("volta", &["VOLTA_HOME", ".volta/bin"]),
    ("pyenv", &["pyenv init", "pyenv virtualenv-init"]),
    ("rbenv", &["rbenv init"]),
    ("rvm", &["rvm/scripts/rvm", ".rvm/bin"]),
    ("chruby", &["chruby.sh", "chruby.fish"]),
    ("goenv", &["goenv init"]),
    (
        "asdf",
        &["asdf.sh", "asdf.fish", "asdf/shims", "/asdf/libexec"],
    ),
];

/// Lines in your shell startup files that turn on another version manager.
/// Commented-out lines don't count.
pub fn shell_conflicts(home: &Path) -> Vec<ShellConflict> {
    let zdotdir = env_dir("ZDOTDIR").unwrap_or_else(|| home.to_path_buf());
    let mut files = vec![
        zdotdir.join(".zshrc"),
        zdotdir.join(".zprofile"),
        zdotdir.join(".zshenv"),
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".profile"),
        home.join(".config/fish/config.fish"),
    ];
    // PowerShell profiles: PowerShell 7 and Windows PowerShell 5.1, for every
    // host, in Documents (which OneDrive may move) or, on macOS and Linux,
    // ~/.config/powershell.
    let config = env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
    for folder in [
        home.join("Documents/PowerShell"),
        home.join("Documents/WindowsPowerShell"),
        home.join("OneDrive/Documents/PowerShell"),
        home.join("OneDrive/Documents/WindowsPowerShell"),
        config.join("powershell"),
    ] {
        files.push(folder.join("Microsoft.PowerShell_profile.ps1"));
        files.push(folder.join("profile.ps1"));
    }
    // Nushell's env.nu and config.nu.
    for folder in [
        config.join("nushell"),
        home.join("Library/Application Support/nushell"),
        home.join("AppData/Roaming/nushell"),
    ] {
        files.push(folder.join("env.nu"));
        files.push(folder.join("config.nu"));
    }
    files.dedup();
    let mut conflicts = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((manager, _)) = CONFLICT_PATTERNS
                .iter()
                .find(|(_, needles)| needles.iter().any(|needle| trimmed.contains(needle)))
            {
                conflicts.push(ShellConflict {
                    file: file.clone(),
                    line: index + 1,
                    text: trimmed.chars().take(200).collect(),
                    manager,
                });
            }
        }
    }
    conflicts
}

// ---------------------------------------------------------------------------
// Command-line packages installed outside mise
// ---------------------------------------------------------------------------

/// A command-line package installed globally with npm, pipx, uv, or cargo,
/// which mise can manage instead (`npm:…`, `pipx:…`, `cargo:…`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalPackage {
    /// The mise backend: `npm`, `pipx`, `cargo`, or `gem`.
    pub backend: &'static str,
    pub name: String,
    pub version: Option<String>,
    /// Where it was found, in words (`npm in nvm’s Node 20.11.1`).
    pub found_in: String,
}

/// npm packages that come with Node itself.
const BUNDLED_NPM: &[&str] = &["npm", "corepack"];

/// Where Homebrew may be installed (its global npm packages live in
/// `<prefix>/lib/node_modules`).
pub const HOMEBREW_PREFIXES: [&str; 3] =
    ["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"];

/// Global npm packages in the default Node of each manager (and in
/// Homebrew's, under `homebrew_prefixes`, and Volta's), pipx and uv tools,
/// and cargo installs.
pub fn global_packages(
    home: &Path,
    imports: &[DetectedImport],
    homebrew_prefixes: &[&str],
) -> Vec<GlobalPackage> {
    let mut found: Vec<GlobalPackage> = Vec::new();
    let mut add = |package: GlobalPackage| {
        if !found
            .iter()
            .any(|existing| existing.backend == package.backend && existing.name == package.name)
        {
            found.push(package);
        }
    };

    // npm: `<node>/lib/node_modules` (`<node>/node_modules` on Windows).
    let mut node_dirs: Vec<(PathBuf, String)> = imports
        .iter()
        .filter(|import| import.tool == "node")
        .filter_map(|import| {
            let default = import.default_version.as_ref()?;
            let version = import
                .versions
                .iter()
                .find(|version| &version.version == default)?;
            Some((
                version.path.clone(),
                format!("{} Node {default}", label(import.source)),
            ))
        })
        .collect();
    for prefix in homebrew_prefixes {
        node_dirs.push((PathBuf::from(prefix), "Homebrew’s Node".into()));
    }
    if cfg!(windows)
        && let Some(appdata) = env_dir("APPDATA")
    {
        node_dirs.push((appdata.join("npm"), "npm".into()));
    }
    for (dir, where_) in node_dirs {
        for modules in [dir.join("lib/node_modules"), dir.join("node_modules")] {
            for (name, path) in dirs(&modules) {
                // Scoped packages: `@scope/name`.
                let packages = if name.starts_with('@') {
                    dirs(&path)
                        .into_iter()
                        .map(|(inner, path)| (format!("{name}/{inner}"), path))
                        .collect()
                } else {
                    vec![(name, path)]
                };
                for (name, path) in packages {
                    if BUNDLED_NPM.contains(&name.as_str()) {
                        continue;
                    }
                    let version = fs::read_to_string(path.join("package.json"))
                        .ok()
                        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                        .and_then(|manifest| manifest.get("version")?.as_str().map(str::to_owned));
                    add(GlobalPackage {
                        backend: "npm",
                        name,
                        version,
                        found_in: format!("npm, in {where_}"),
                    });
                }
            }
        }
    }
    // Volta installs packages into its own folder.
    let volta = env_dir("VOLTA_HOME").unwrap_or_else(|| home.join(".volta"));
    for (name, _) in dirs(&volta.join("tools/image/packages")) {
        add(GlobalPackage {
            backend: "npm",
            name,
            version: None,
            found_in: "Volta".into(),
        });
    }

    // pipx: `<PIPX_HOME>/venvs/<name>`, version from pipx's metadata.
    let pipx = env_dir("PIPX_HOME").unwrap_or_else(|| home.join(".local/pipx"));
    for (name, path) in dirs(&pipx.join("venvs")) {
        let version = fs::read_to_string(path.join("pipx_metadata.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|meta| {
                meta.pointer("/main_package/package_version")?
                    .as_str()
                    .map(str::to_owned)
            });
        add(GlobalPackage {
            backend: "pipx",
            name,
            version,
            found_in: "pipx".into(),
        });
    }
    // uv tools (mise's pipx backend installs with uv).
    let uv_tools = env_dir("UV_TOOL_DIR").unwrap_or_else(|| {
        env_dir("XDG_DATA_HOME")
            .unwrap_or_else(|| home.join(".local/share"))
            .join("uv/tools")
    });
    for (name, _) in dirs(&uv_tools) {
        add(GlobalPackage {
            backend: "pipx",
            name,
            version: None,
            found_in: "uv tool".into(),
        });
    }

    // cargo: `.crates2.json` keys like `ripgrep 14.1.0 (registry+…)`.
    let cargo = env_dir("CARGO_HOME").unwrap_or_else(|| home.join(".cargo"));
    if let Some(installs) = fs::read_to_string(cargo.join(".crates2.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|crates| crates.get("installs")?.as_object().cloned())
    {
        for key in installs.keys() {
            // Only crates.io installs; git and path installs can't be re-made.
            if !key.contains("(registry+") {
                continue;
            }
            let mut words = key.split_whitespace();
            if let (Some(name), Some(version)) = (words.next(), words.next()) {
                add(GlobalPackage {
                    backend: "cargo",
                    name: name.to_owned(),
                    version: Some(version.to_owned()),
                    found_in: "cargo install".into(),
                });
            }
        }
    }
    // Ruby gems that install commands, in each manager's default Ruby,
    // Homebrew's Ruby, and your user gem folders. Gems that come with Ruby
    // (`specifications/default`) and libraries without commands are skipped.
    let mut gem_roots: Vec<(PathBuf, String)> = imports
        .iter()
        .filter(|import| import.tool == "ruby")
        .filter_map(|import| {
            let default = import.default_version.as_ref()?;
            let version = import
                .versions
                .iter()
                .find(|version| &version.version == default)?;
            Some((
                version.path.join("lib/ruby/gems"),
                format!("{} Ruby {default}", label(import.source)),
            ))
        })
        .collect();
    for prefix in homebrew_prefixes {
        gem_roots.push((
            Path::new(prefix).join("lib/ruby/gems"),
            "Homebrew’s Ruby".into(),
        ));
    }
    gem_roots.push((home.join(".gem/ruby"), "your gems".into()));
    gem_roots.push((home.join(".local/share/gem/ruby"), "your gems".into()));
    for (root, where_) in gem_roots {
        // `<root>/<abi version>/specifications/<name>-<version>.gemspec`.
        for (_, abi) in dirs(&root) {
            let Ok(entries) = fs::read_dir(abi.join("specifications")) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let file = entry.file_name().to_string_lossy().into_owned();
                let Some(stem) = file.strip_suffix(".gemspec") else {
                    continue;
                };
                // The version starts after the last `-` followed by a digit.
                let Some(split) = stem
                    .match_indices('-')
                    .filter(|(index, _)| {
                        stem[index + 1..].starts_with(|c: char| c.is_ascii_digit())
                    })
                    .map(|(index, _)| index)
                    .next_back()
                else {
                    continue;
                };
                let Ok(spec) = fs::read_to_string(entry.path()) else {
                    continue;
                };
                // `s.executables = ["rubocop".freeze]` — only gems with commands.
                let has_commands = spec.lines().any(|line| {
                    line.contains(".executables = [") && !line.contains(".executables = []")
                });
                if !has_commands {
                    continue;
                }
                add(GlobalPackage {
                    backend: "gem",
                    name: stem[..split].to_owned(),
                    version: Some(stem[split + 1..].to_owned()),
                    found_in: format!("gem, in {where_}"),
                });
            }
        }
    }
    found.sort_by(|a, b| (a.backend, &a.name).cmp(&(b.backend, &b.name)));
    found
}

// ---------------------------------------------------------------------------
// Programs installed some other way
// ---------------------------------------------------------------------------

/// A program found where installers put command-line tools, which mise may
/// be able to manage (matched against the registry's command names by the
/// app).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledProgram {
    /// The command name (`flyctl`).
    pub name: String,
    pub path: PathBuf,
    /// Where it came from, in words (`Homebrew`, `Fly’s installer`).
    pub found_in: String,
    /// The Homebrew formula or cask it belongs to, when it's Homebrew's;
    /// the app matches that to the registry rather than the command name.
    pub package: Option<String>,
    /// The command that removes the old copy, when there's a clear one
    /// (`brew uninstall flyctl`). Shown, never run.
    pub remove_command: Option<String>,
}

/// Folders installers put command-line tools in, with how to describe them.
fn program_dirs(home: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut dirs: Vec<(PathBuf, &'static str)> = HOMEBREW_PREFIXES
        .iter()
        .map(|prefix| (Path::new(prefix).join("bin"), "Homebrew"))
        .collect();
    dirs.extend([
        (home.join(".local/bin"), "an installer"),
        (home.join("bin"), "an installer"),
        (home.join(".fly/bin"), "Fly’s installer"),
        (home.join(".bun/bin"), "Bun"),
        (home.join(".deno/bin"), "Deno’s installer"),
        (
            env_dir("GOBIN").unwrap_or_else(|| {
                env_dir("GOPATH")
                    .unwrap_or_else(|| home.join("go"))
                    .join("bin")
            }),
            "go install",
        ),
    ]);
    if cfg!(target_os = "linux") {
        dirs.push((PathBuf::from("/snap/bin"), "Snap"));
    }
    if cfg!(windows) {
        if let Some(local) = env_dir("LOCALAPPDATA") {
            dirs.push((local.join("Microsoft/WinGet/Links"), "winget"));
        }
        dirs.push((home.join("scoop/shims"), "Scoop"));
    }
    dirs
}

/// Programs in the usual install folders (not mise's own, not the
/// system's), with where each came from. Node, Python, and Ruby packages
/// are left to [`global_packages`], which knows their package names.
pub fn installed_programs(home: &Path, mise_data: Option<&Path>) -> Vec<InstalledProgram> {
    // Compared with resolved program paths, so resolve it too (macOS's
    // `/var` and `/tmp` are links to `/private/…`).
    let mise_data =
        mise_data.map(|data| fs::canonicalize(data).unwrap_or_else(|_| data.to_path_buf()));
    let mise_data = mise_data.as_deref();
    let mut found: Vec<InstalledProgram> = Vec::new();
    for (dir, label) in program_dirs(home) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let name = name
                .strip_suffix(".exe")
                .or_else(|| name.strip_suffix(".cmd"))
                .unwrap_or(&name)
                .to_owned();
            if name.starts_with('.')
                || name == "mise"
                || found.iter().any(|program| program.name == name)
            {
                continue;
            }
            let resolved = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !resolved.is_file() || mise_data.is_some_and(|data| resolved.starts_with(data)) {
                continue;
            }
            let text = resolved.to_string_lossy().replace('\\', "/");
            // An app's command-line helper (VS Code's `code`, Docker
            // Desktop's `docker`) comes and goes with the app.
            if text.contains(".app/") && !text.contains("/Caskroom/") {
                continue;
            }
            // Packages from npm, pip, or gem are listed by package instead.
            if [
                "/node_modules/",
                "/site-packages/",
                "/gems/",
                "/.cargo/",
                "/pipx/",
                "/uv/tools/",
            ]
            .iter()
            .any(|part| text.contains(part))
            {
                continue;
            }
            let after = |marker: &str| {
                text.split_once(marker)
                    .and_then(|(_, rest)| rest.split('/').next())
                    .map(str::to_owned)
            };
            let (found_in, package, remove_command) = if let Some(formula) = after("/Cellar/") {
                let command = format!("brew uninstall {formula}");
                ("Homebrew".to_owned(), Some(formula), Some(command))
            } else if let Some(cask) = after("/Caskroom/") {
                let command = format!("brew uninstall --cask {cask}");
                ("Homebrew".to_owned(), Some(cask), Some(command))
            } else if label == "Homebrew" {
                // Not Homebrew's own: something installed into its bin folder.
                (dir.display().to_string(), None, None)
            } else {
                (label.to_owned(), None, None)
            };
            found.push(InstalledProgram {
                name,
                path,
                found_in,
                package,
                remove_command,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// A source's name, for messages.
fn label(source: ImportSource) -> &'static str {
    match source {
        ImportSource::Nvm | ImportSource::NvmWindows => "nvm’s",
        ImportSource::Nodenv => "nodenv’s",
        ImportSource::NodeBrew | ImportSource::RubyBrew => "Homebrew’s",
        ImportSource::Fnm => "fnm’s",
        ImportSource::Volta => "Volta’s",
        ImportSource::N => "n’s",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("powermise-imports-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_link_sources_defaults_and_whats_already_in_mise() {
        let home = temp("detect");
        let installs = home.join("mise-installs");
        for dir in [
            ".rbenv/versions/3.2.2/bin",
            ".rbenv/versions/3.3.0/bin",
            ".asdf/installs/nodejs/20.11.1/bin",
            ".asdf/installs/golang/1.22.0/bin",
            ".local/share/fnm/node-versions/v18.20.4/installation/bin",
            ".local/share/fnm/node-versions/v20.11.1/installation/bin",
            ".rvm/rubies/ruby-3.1.4/bin",
            ".nvm/versions/node/v20.11.1",
            ".nvm/versions/node/v22.3.0",
            "mise-installs/ruby/3.3.0",
        ] {
            fs::create_dir_all(home.join(dir)).unwrap();
        }
        fs::write(home.join(".rbenv/version"), "3.2\n").unwrap();
        fs::write(
            home.join(".tool-versions"),
            "nodejs 20.11.1\ngolang 1.22.0\n",
        )
        .unwrap();
        fs::create_dir_all(home.join(".rvm/config")).unwrap();
        fs::write(home.join(".rvm/config/alias"), "default=ruby-3.1.4\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            home.join(".local/share/fnm/node-versions/v20.11.1/installation"),
            {
                fs::create_dir_all(home.join(".local/share/fnm/aliases")).unwrap();
                home.join(".local/share/fnm/aliases/default")
            },
        )
        .unwrap();

        let found = detect_imports(&home, Some(&installs));
        let get = |source, tool: &str| {
            found
                .iter()
                .find(|import| import.source == source && import.tool == tool)
                .unwrap_or_else(|| panic!("{source:?} {tool}"))
        };
        let rbenv = get(ImportSource::Rbenv, "ruby");
        assert_eq!(rbenv.default_version.as_deref(), Some("3.2.2"));
        assert!(rbenv.versions[1].in_mise && !rbenv.versions[0].in_mise);
        // Only the version mise doesn't have is linked.
        let commands: Vec<_> = rbenv.commands().iter().map(MiseCommand::display).collect();
        assert_eq!(commands.len(), 1);
        assert!(
            commands[0].starts_with("mise link ruby@3.2.2 "),
            "{commands:?}"
        );

        // nvm imports with one `mise sync`, not a link per version.
        if std::env::var_os("NVM_DIR").is_none() {
            let nvm = get(ImportSource::Nvm, "node");
            let versions: Vec<_> = nvm.versions.iter().map(|v| v.version.as_str()).collect();
            assert_eq!(versions, ["20.11.1", "22.3.0"]);
            let commands: Vec<_> = nvm.commands().iter().map(MiseCommand::display).collect();
            assert_eq!(commands, ["mise sync node --nvm"]);
        }

        let asdf_node = get(ImportSource::Asdf, "node");
        assert_eq!(asdf_node.default_version.as_deref(), Some("20.11.1"));
        assert_eq!(get(ImportSource::Asdf, "go").versions[0].version, "1.22.0");
        assert_eq!(
            get(ImportSource::Rvm, "ruby").default_version.as_deref(),
            Some("3.1.4")
        );
        let fnm = get(ImportSource::Fnm, "node");
        assert!(fnm.versions[0].path.ends_with("installation"));
        #[cfg(unix)]
        assert_eq!(fnm.default_version.as_deref(), Some("20.11.1"));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn spots_other_managers_in_startup_files() {
        let home = temp("shell");
        fs::write(
            home.join(".zshrc"),
            "export PATH=x\n# eval \"$(pyenv init -)\"\neval \"$(pyenv init -)\"\n[ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"\neval \"$(mise activate zsh)\"\n",
        )
        .unwrap();
        let conflicts = shell_conflicts(&home);
        fs::remove_dir_all(&home).unwrap();
        let found: Vec<_> = conflicts.iter().map(|c| (c.manager, c.line)).collect();
        assert_eq!(found, [("pyenv", 3), ("nvm", 4)]);
    }

    #[test]
    fn spots_other_managers_in_powershell_and_nushell() {
        let home = temp("shell-more");
        let profile = home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1");
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            "# fnm env --use-on-cd | Out-String | Invoke-Expression\nfnm env --use-on-cd | Out-String | Invoke-Expression\n",
        )
        .unwrap();
        let nushell = home.join("AppData/Roaming/nushell/env.nu");
        fs::create_dir_all(nushell.parent().unwrap()).unwrap();
        fs::write(&nushell, "fnm env --json | from json | load-env\n").unwrap();
        let conflicts = shell_conflicts(&home);
        fs::remove_dir_all(&home).unwrap();
        let found: Vec<_> = conflicts
            .iter()
            .map(|c| (c.file.file_name().unwrap().to_owned(), c.line))
            .collect();
        assert_eq!(
            found,
            [
                ("Microsoft.PowerShell_profile.ps1".into(), 2),
                ("env.nu".into(), 1)
            ]
        );
    }

    #[test]
    fn lists_global_packages() {
        let home = temp("packages");
        let node = home.join(".nvm/versions/node/v20.11.1");
        for (dir, manifest) in [
            ("lib/node_modules/prettier", r#"{"version":"3.3.3"}"#),
            ("lib/node_modules/npm", r#"{"version":"10.0.0"}"#),
            ("lib/node_modules/@biomejs/biome", r#"{"version":"1.9.0"}"#),
        ] {
            fs::create_dir_all(node.join(dir)).unwrap();
            fs::write(node.join(dir).join("package.json"), manifest).unwrap();
        }
        fs::create_dir_all(home.join(".nvm/alias")).unwrap();
        fs::write(home.join(".nvm/alias/default"), "20\n").unwrap();
        fs::create_dir_all(home.join(".local/pipx/venvs/black")).unwrap();
        fs::write(
            home.join(".local/pipx/venvs/black/pipx_metadata.json"),
            r#"{"main_package":{"package_version":"24.8.0"}}"#,
        )
        .unwrap();
        let gems = home.join(".gem/ruby/3.3.0/specifications");
        fs::create_dir_all(gems.join("default")).unwrap();
        fs::write(
            gems.join("rubocop-1.65.0.gemspec"),
            "  s.executables = [\"rubocop\".freeze]\n",
        )
        .unwrap();
        fs::write(
            gems.join("activesupport-7.2.1.gemspec"),
            "  s.executables = []\n",
        )
        .unwrap();
        fs::write(
            gems.join("default/bundler-2.5.11.gemspec"),
            "  s.executables = [\"bundle\".freeze]\n",
        )
        .unwrap();
        fs::create_dir_all(home.join(".cargo")).unwrap();
        fs::write(
            home.join(".cargo/.crates2.json"),
            r#"{"installs":{"ripgrep 14.1.0 (registry+https://github.com/rust-lang/crates.io-index)":{},"mine 0.1.0 (path+file:///x)":{}}}"#,
        )
        .unwrap();
        let imports = detect_imports(&home, None);
        let packages = global_packages(&home, &imports, &[]);
        fs::remove_dir_all(&home).unwrap();
        let names: Vec<_> = packages
            .iter()
            .filter(|p| p.backend != "cargo" || std::env::var_os("CARGO_HOME").is_none())
            .map(|p| (p.backend, p.name.as_str(), p.version.as_deref()))
            .collect();
        let mut expected = vec![
            ("gem", "rubocop", Some("1.65.0")),
            ("npm", "@biomejs/biome", Some("1.9.0")),
            ("npm", "prettier", Some("3.3.3")),
            ("pipx", "black", Some("24.8.0")),
        ];
        // `cargo test` sets CARGO_HOME, which points the scan at the real one.
        if std::env::var_os("CARGO_HOME").is_none() {
            expected.insert(0, ("cargo", "ripgrep", Some("14.1.0")));
        }
        assert_eq!(names, expected);
    }

    #[test]
    fn finds_programs_installers_put_on_path() {
        let home = temp("programs");
        let write = |path: &str| {
            let path = home.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
        };
        write(".fly/bin/flyctl");
        write(".local/bin/stripe");
        write(".local/bin/mise");
        write(".local/bin/.hidden");
        // A pipx-installed command, listed by package instead.
        write(".local/pipx/venvs/black/bin/black");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            home.join(".local/pipx/venvs/black/bin/black"),
            home.join(".local/bin/black"),
        )
        .unwrap();
        let mise_data = home.join(".local/share/mise");
        write(".local/share/mise/installs/jq/1.8.2/bin/jq");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            home.join(".local/share/mise/installs/jq/1.8.2/bin/jq"),
            home.join(".local/bin/jq"),
        )
        .unwrap();
        let found = installed_programs(&home, Some(&mise_data));
        fs::remove_dir_all(&home).unwrap();
        let ours: Vec<_> = found
            .iter()
            .filter(|program| program.path.starts_with(&home))
            .map(|program| (program.name.as_str(), program.found_in.as_str()))
            .collect();
        assert_eq!(
            ours,
            [("flyctl", "Fly’s installer"), ("stripe", "an installer")]
        );
    }
}
