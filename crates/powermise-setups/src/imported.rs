//! What Import brought into mise: `conf.d/powermise-imported.toml`, one
//! file powermise owns (like a setup's), so your own config is never edited
//! and deleting the file takes the import back out of mise's config.
//! Homebrew formulas also get `[env] _.path`, since mise installs them
//! without putting them on PATH.
use std::{collections::BTreeMap, fs, path::Path, path::PathBuf};

use powermise_compiler::{env_path_toml, homebrew_formula_paths};
use powermise_domain::Platform;
use powermise_state::{GeneratedFileChange, inspect_generated_file, write_atomically};
use serde::Serialize;

use crate::SetupsError;

/// `[tools]` and `[bootstrap.packages]` the import declares. `[env] _.path`
/// follows from the packages, so it isn't stored.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ImportedConfig {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Bootstrap::is_empty")]
    pub bootstrap: Bootstrap,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Bootstrap {
    pub packages: BTreeMap<String, String>,
}

impl Bootstrap {
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// `<mise config dir>/conf.d/powermise-imported.toml`.
pub fn imported_path(mise_config_dir: &Path) -> PathBuf {
    mise_config_dir
        .join("conf.d")
        .join("powermise-imported.toml")
}

/// The imported file's entries, or none when there isn't one yet.
pub fn read_imported(mise_config_dir: &Path) -> ImportedConfig {
    let Ok(text) = fs::read_to_string(imported_path(mise_config_dir)) else {
        return ImportedConfig::default();
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return ImportedConfig::default();
    };
    let strings = |value: Option<&toml::Value>| -> BTreeMap<String, String> {
        value
            .and_then(toml::Value::as_table)
            .map(|table| {
                table
                    .iter()
                    .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    };
    ImportedConfig {
        tools: strings(table.get("tools")),
        bootstrap: Bootstrap {
            packages: strings(
                table
                    .get("bootstrap")
                    .and_then(|bootstrap| bootstrap.get("packages")),
            ),
        },
    }
}

impl ImportedConfig {
    /// The existing file plus these new entries (a new pick replaces an old
    /// version of the same tool).
    pub fn merged(mut self, tools: &[(String, String)], packages: &[String]) -> Self {
        for (tool, version) in tools {
            self.tools.insert(tool.clone(), version.clone());
        }
        for package in packages {
            self.bootstrap
                .packages
                .entry(package.clone())
                .or_insert_with(|| "latest".into());
        }
        self
    }

    /// The file for this machine. On an Intel Mac mise skips `brew:`
    /// formulas, so there's nothing to put on PATH.
    pub fn contents(&self) -> Result<String, SetupsError> {
        let target = crate::host_platform();
        let target = crate::compile_options(target)
            .homebrew_formulas
            .then_some(target);
        self.contents_for(target)
    }

    /// The file for `target` (none: no Homebrew formulas on PATH).
    pub fn contents_for(&self, target: Option<Platform>) -> Result<String, SetupsError> {
        let paths = target.map_or_else(Vec::new, |target| {
            homebrew_formula_paths(self.bootstrap.packages.keys().map(String::as_str), target)
        });
        let body = toml::to_string(self).map_err(|error| SetupsError::Render(error.to_string()))?;
        Ok(format!(
            "# Written by powermise's Import. Tools and packages you brought into mise\n\
# from other version managers, Homebrew, a Brewfile, and so on. powermise adds\n\
# to this file when you import more; delete it to take them out of mise's\n\
# config (anything installed stays installed).\n\n{}{}",
            body.trim_start(),
            env_path_toml(&paths)
        ))
    }
}

/// How writing `config` would change the file (with a diff). Writes nothing.
pub fn inspect_imported(
    mise_config_dir: &Path,
    config: &ImportedConfig,
) -> Result<GeneratedFileChange, SetupsError> {
    Ok(inspect_generated_file(
        &imported_path(mise_config_dir),
        config.contents()?.as_bytes(),
    )?)
}

pub fn write_imported(
    mise_config_dir: &Path,
    config: &ImportedConfig,
) -> Result<PathBuf, SetupsError> {
    let path = imported_path(mise_config_dir);
    write_atomically(&path, config.contents()?.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_to_what_was_imported_before() {
        let dir = std::env::temp_dir().join(format!("powermise-imported-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let first = ImportedConfig::default().merged(
            &[("npm:prettier".into(), "3.3.3".into())],
            &["brew-cask:ghostty".into()],
        );
        write_imported(&dir, &first).unwrap();
        let second = read_imported(&dir).merged(
            &[("flyctl".into(), "latest".into())],
            &["mas:441258766".into(), "brew-cask:ghostty".into()],
        );
        let contents = second.contents_for(Some(Platform::Darwin)).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(
            contents.contains("\"npm:prettier\" = \"3.3.3\""),
            "{contents}"
        );
        assert!(contents.contains("flyctl = \"latest\""), "{contents}");
        assert!(contents.contains("[bootstrap.packages]"), "{contents}");
        assert_eq!(second.bootstrap.packages.len(), 2);
        // What mise reads: the file is valid TOML with those tables.
        let table: toml::Table = contents.parse().unwrap();
        assert!(
            table["bootstrap"]["packages"]
                .get("mas:441258766")
                .is_some()
        );
        // No `brew:` formulas, so nothing to put on PATH.
        assert!(!contents.contains("[env]"), "{contents}");
    }

    #[test]
    fn homebrew_formulas_go_on_path() {
        let config =
            ImportedConfig::default().merged(&[], &["brew:jq".into(), "brew-cask:ghostty".into()]);
        let table: toml::Table = config
            .contents_for(Some(Platform::Linux))
            .unwrap()
            .parse()
            .unwrap();
        let paths = table["env"]["_"]["path"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].as_str(),
            Some("/home/linuxbrew/.linuxbrew/opt/jq/bin")
        );
        // An Intel Mac skips formulas: nothing on PATH.
        assert!(!config.contents_for(None).unwrap().contains("[env]"));
    }
}
