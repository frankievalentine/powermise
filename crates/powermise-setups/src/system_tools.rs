//! Newer versions of the tools macOS ships (git, bash, make, curl):
//! `conf.d/powermise-system.toml`, one file powermise owns. It declares the
//! Homebrew packages, which mise installs itself, and puts each one's own
//! `/opt/homebrew/opt/<formula>/bin` first on your PATH with `[env] _.path`.
//! mise doesn't put Homebrew formulas on PATH on its own, and curl is
//! keg-only (never linked into `/opt/homebrew/bin`). Deleting the file puts
//! macOS's versions back.
use std::{collections::BTreeMap, fs, path::Path, path::PathBuf};

use powermise_compiler::{env_path_toml, homebrew_formula_paths};
use powermise_domain::Platform;
use powermise_state::{GeneratedFileChange, inspect_generated_file, write_atomically};

use crate::SetupsError;

/// `[bootstrap.packages]` the file declares. `[env] _.path` follows from the
/// packages, so it isn't stored: rewriting the file drops anything else an
/// older powermise put there (`/opt/homebrew/bin`, which also holds every
/// formula's dependencies and would shadow mise's own tools).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemToolsConfig {
    /// `brew:<formula>` → version.
    pub packages: BTreeMap<String, String>,
}

/// `<mise config dir>/conf.d/powermise-system.toml`.
pub fn system_tools_path(mise_config_dir: &Path) -> PathBuf {
    mise_config_dir.join("conf.d").join("powermise-system.toml")
}

/// The file's entries, or none when there isn't one yet.
pub fn read_system_tools(mise_config_dir: &Path) -> SystemToolsConfig {
    let Some(table) = fs::read_to_string(system_tools_path(mise_config_dir))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
    else {
        return SystemToolsConfig::default();
    };
    let packages = table
        .get("bootstrap")
        .and_then(|bootstrap| bootstrap.get("packages"))
        .and_then(toml::Value::as_table)
        .map(|packages| {
            packages
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    SystemToolsConfig { packages }
}

impl SystemToolsConfig {
    /// The file plus these packages (each added once).
    pub fn merged(mut self, packages: &[String]) -> Self {
        for package in packages {
            self.packages
                .entry(package.clone())
                .or_insert_with(|| "latest".into());
        }
        self
    }

    /// The folders mise puts first on PATH: each formula's own
    /// `/opt/homebrew/opt/<formula>/bin`, which keg-only curl has too.
    pub fn paths(&self) -> Vec<String> {
        homebrew_formula_paths(self.packages.keys().map(String::as_str), Platform::Darwin)
    }

    pub fn contents(&self) -> String {
        let quote = |text: &str| toml::Value::String(text.into()).to_string();
        let mut out = String::from(
            "# Written by powermise: newer versions of tools macOS ships (git, bash,\n\
# make, curl). mise installs the Homebrew packages below, and puts their\n\
# folders ahead of macOS's on your PATH in every shell where mise is turned\n\
# on. Delete this file to go back to macOS's versions (the packages stay\n\
# installed).\n",
        );
        if !self.packages.is_empty() {
            out.push_str("\n[bootstrap.packages]\n");
            for (package, version) in &self.packages {
                out.push_str(&format!("{} = {}\n", quote(package), quote(version)));
            }
        }
        out.push_str(&env_path_toml(&self.paths()));
        out
    }
}

/// How writing `config` would change the file. Writes nothing.
pub fn inspect_system_tools(
    mise_config_dir: &Path,
    config: &SystemToolsConfig,
) -> Result<GeneratedFileChange, SetupsError> {
    Ok(inspect_generated_file(
        &system_tools_path(mise_config_dir),
        config.contents().as_bytes(),
    )?)
}

pub fn write_system_tools(
    mise_config_dir: &Path,
    config: &SystemToolsConfig,
) -> Result<PathBuf, SetupsError> {
    let path = system_tools_path(mise_config_dir);
    write_atomically(&path, config.contents().as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("powermise-system-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn adds_packages_once_with_their_own_folders() {
        let dir = temp_dir("add");
        let first = SystemToolsConfig::default().merged(&["brew:git".into()]);
        write_system_tools(&dir, &first).unwrap();
        let second = read_system_tools(&dir).merged(&["brew:curl".into(), "brew:git".into()]);
        let contents = second.contents();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(second.packages.len(), 2);
        // What mise reads: `[env] _.path` and `[bootstrap.packages]`.
        let table: toml::Table = contents.parse().unwrap();
        assert_eq!(
            table["env"]["_"]["path"].as_array().unwrap(),
            &[
                toml::Value::from("/opt/homebrew/opt/curl/bin"),
                toml::Value::from("/opt/homebrew/opt/git/bin"),
            ],
            "{contents}"
        );
        assert_eq!(
            table["bootstrap"]["packages"]["brew:curl"].as_str(),
            Some("latest")
        );
    }

    #[test]
    fn rewriting_drops_homebrews_shared_folder() {
        let dir = temp_dir("migrate");
        let path = system_tools_path(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "[bootstrap.packages]\n\"brew:curl\" = \"latest\"\n\"brew:git\" = \"latest\"\n\n\
             [env]\n_.path = [\"/opt/homebrew/opt/curl/bin\", \"/opt/homebrew/bin\"]\n",
        )
        .unwrap();
        let config = read_system_tools(&dir).merged(&["brew:make".into()]);
        write_system_tools(&dir, &config).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(!contents.contains("\"/opt/homebrew/bin\""), "{contents}");
        assert!(
            contents.contains(
                "_.path = [\"/opt/homebrew/opt/curl/bin\", \"/opt/homebrew/opt/git/bin\", \"/opt/homebrew/opt/make/bin\"]"
            ),
            "{contents}"
        );
    }
}
