//! Saved AI setups (selection files): create, list, export, import, and remove.
use std::{fs, io, path::Path, path::PathBuf};

use powermise_domain::Selection;
use powermise_registry::load_selection;

use crate::{PowermiseLayout, SetupsError};

/// Manages named selection files under a powermise layout.
pub struct SelectionStore<'a> {
    layout: &'a PowermiseLayout,
}

impl<'a> SelectionStore<'a> {
    pub fn new(layout: &'a PowermiseLayout) -> Self {
        Self { layout }
    }

    /// Writes a new managed selection, failing if it already exists.
    pub fn create(&self, selection: &Selection) -> Result<PathBuf, SetupsError> {
        crate::validate_selection_name(&selection.name)?;
        fs::create_dir_all(&self.layout.selections).map_err(|source| SetupsError::Io {
            path: self.layout.selections.clone(),
            source,
        })?;
        let path = self.layout.selection_path(&selection.name);
        if path.exists() {
            return Err(SetupsError::AlreadyExists {
                name: selection.name.clone(),
                path,
            });
        }
        let contents = toml::to_string(selection).map_err(|source| SetupsError::TomlSerialize {
            path: path.clone(),
            source,
        })?;
        fs::write(&path, contents).map_err(|source| SetupsError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Lists managed selection names in deterministic order.
    pub fn list(&self) -> Result<Vec<String>, SetupsError> {
        let mut names = Vec::new();
        let entries = match fs::read_dir(&self.layout.selections) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(names),
            Err(source) => {
                return Err(SetupsError::Io {
                    path: self.layout.selections.clone(),
                    source,
                });
            }
        };
        for entry in entries {
            let entry = entry.map_err(|source| SetupsError::Io {
                path: self.layout.selections.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Reads a managed selection by name.
    pub fn read(&self, name: &str) -> Result<Selection, SetupsError> {
        crate::validate_selection_name(name)?;
        let path = self.layout.selection_path(name);
        if !path.exists() {
            return Err(SetupsError::NotFound { name: name.into() });
        }
        let selection = load_selection(&path)?;
        if selection.name != name {
            return Err(SetupsError::Other {
                message: format!(
                    "setup file `{}` declares name `{}`; the file name and `name` must match",
                    path.display(),
                    selection.name
                ),
            });
        }
        Ok(selection)
    }

    /// Renders a managed selection as TOML for export.
    pub fn export(&self, name: &str) -> Result<String, SetupsError> {
        let selection = self.read(name)?;
        toml::to_string(&selection).map_err(|source| SetupsError::TomlSerialize {
            path: self.layout.selection_path(name),
            source,
        })
    }

    /// Validates and copies a selection file into the managed store.
    ///
    /// The managed name is `name_override` if given, otherwise the selection's
    /// own `name` field. Fails if the destination already exists.
    pub fn import(
        &self,
        source_path: &Path,
        name_override: Option<&str>,
    ) -> Result<PathBuf, SetupsError> {
        let selection = load_selection(source_path)?;
        let mut imported = selection;
        if let Some(name) = name_override {
            imported.name = name.into();
        }
        self.create(&imported)
    }

    /// Removes a managed selection file, returning the removed path.
    pub fn remove(&self, name: &str) -> Result<PathBuf, SetupsError> {
        crate::validate_selection_name(name)?;
        let path = self.layout.selection_path(name);
        if !path.exists() {
            return Err(SetupsError::NotFound { name: name.into() });
        }
        fs::remove_file(&path).map_err(|source| SetupsError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }
}
