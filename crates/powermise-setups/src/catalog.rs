//! Catalog location: an explicit directory, `POWERMISE_CATALOG_DIR`, a
//! repository checkout's `catalog/profiles`, or the catalog embedded in this
//! build. Embedding means an installed binary (or the desktop app) works from
//! any working directory.
use std::{
    env,
    path::{Path, PathBuf},
};

use crate::{CatalogPaths, PowermiseLayout, SetupsError};

/// The catalog files compiled into this build, by file name.
const EMBEDDED_CATALOG: [(&str, &str); 4] = [
    (
        "harnesses.toml",
        include_str!("../../../catalog/profiles/harnesses.toml"),
    ),
    (
        "tools.toml",
        include_str!("../../../catalog/profiles/tools.toml"),
    ),
    (
        "integrations.toml",
        include_str!("../../../catalog/profiles/integrations.toml"),
    ),
    (
        "mcp.toml",
        include_str!("../../../catalog/profiles/mcp.toml"),
    ),
];

/// The repository-relative catalog directory used during development.
const CHECKOUT_CATALOG_DIR: &str = "catalog/profiles";

impl CatalogPaths {
    /// The standard catalog file names inside `directory`.
    pub fn in_dir(directory: &Path) -> Self {
        Self {
            harnesses: directory.join("harnesses.toml"),
            tools: directory.join("tools.toml"),
            integrations: directory.join("integrations.toml"),
            mcp: directory.join("mcp.toml"),
        }
    }

    /// Resolves the catalog directory from `explicit`, then
    /// `POWERMISE_CATALOG_DIR`, then `./catalog/profiles` when the working
    /// directory is a repository checkout, and finally the embedded catalog,
    /// materialized under `<powermise root>/state/catalog`.
    pub fn resolve(explicit: Option<PathBuf>) -> Result<Self, SetupsError> {
        if let Some(directory) =
            explicit.or_else(|| env::var_os("POWERMISE_CATALOG_DIR").map(PathBuf::from))
        {
            return Ok(Self::in_dir(&directory));
        }
        let checkout = PathBuf::from(CHECKOUT_CATALOG_DIR);
        if checkout.join("harnesses.toml").is_file() {
            return Ok(Self::in_dir(&checkout));
        }
        let layout = PowermiseLayout::resolve(None)?;
        Self::embedded(&layout)
    }

    /// Writes the embedded catalog under the layout's state directory (only
    /// when its contents changed) and returns its paths.
    pub fn embedded(layout: &PowermiseLayout) -> Result<Self, SetupsError> {
        let directory = layout.state.join("catalog");
        for (file_name, contents) in EMBEDDED_CATALOG {
            powermise_state::write_atomically(&directory.join(file_name), contents.as_bytes())?;
        }
        Ok(Self::in_dir(&directory))
    }
}
