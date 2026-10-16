//! Newer versions of the tools macOS ships (git, bash, make, curl):
//! `conf.d/powermise-system.toml`, one file powermise owns. It declares the
//! Homebrew packages, which mise installs itself, and puts their folders
//! first on your PATH with `[env] _.path`. mise doesn't add Homebrew's folder
//! to PATH on its own, and curl is keg-only (never linked into it). Deleting
//! the file puts macOS's versions back.
use std::{collections::BTreeMap, fs, path::Path, path::PathBuf};

use powermise_state::{GeneratedFileChange, inspect_generated_file, write_atomically};

use crate::SetupsError;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemToolsConfig {
    /// `brew:<formula>` → version.
    pub packages: BTreeMap<String, String>,
    /// Folders mise puts first on PATH, in order.
    pub paths: Vec<String>,
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
    let paths = table
        .get("env")
        .and_then(|env| env.get("_"))
        .and_then(|underscore| underscore.get("path"))
        .and_then(toml::Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| path.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    SystemToolsConfig { packages, paths }
}

impl SystemToolsConfig {
    /// The file plus these packages and folders (each added once).
    pub fn merged(mut self, packages: &[String], paths: &[String]) -> Self {
        for package in packages {
            self.packages
                .entry(package.clone())
                .or_insert_with(|| "latest".into());
        }
        for path in paths {
            if !self.paths.contains(path) {
                self.paths.push(path.clone());
            }
        }
        self
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
        if !self.paths.is_empty() {
            let paths: Vec<String> = self.paths.iter().map(|path| quote(path)).collect();
            out.push_str(&format!("\n[env]\n_.path = [{}]\n", paths.join(", ")));
        }
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

    #[test]
    fn adds_packages_and_paths_once() {
        let dir = std::env::temp_dir().join(format!("powermise-system-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let first = SystemToolsConfig::default()
            .merged(&["brew:git".into()], &["/opt/homebrew/bin".into()]);
        write_system_tools(&dir, &first).unwrap();
        let second = read_system_tools(&dir).merged(
            &["brew:curl".into(), "brew:git".into()],
            &[
                "/opt/homebrew/opt/curl/bin".into(),
                "/opt/homebrew/bin".into(),
            ],
        );
        let contents = second.contents();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(second.packages.len(), 2);
        assert_eq!(
            second.paths,
            ["/opt/homebrew/bin", "/opt/homebrew/opt/curl/bin"]
        );
        // What mise reads: `[env] _.path` and `[bootstrap.packages]`.
        let table: toml::Table = contents.parse().unwrap();
        assert_eq!(
            table["env"]["_"]["path"].as_array().unwrap().len(),
            2,
            "{contents}"
        );
        assert_eq!(
            table["bootstrap"]["packages"]["brew:curl"].as_str(),
            Some("latest")
        );
    }
}
