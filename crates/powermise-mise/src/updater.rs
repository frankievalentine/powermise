//! How to update mise itself when `mise self-update` can't: Homebrew, winget,
//! and Scoop builds are updated by their package manager, and packagers may
//! ship instructions next to the binary
//! (<https://mise.jdx.dev/packaging.html>).
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

/// The package manager that owns this mise, or the packager's instructions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MiseUpdater {
    /// `brew upgrade mise`.
    Homebrew,
    /// `winget upgrade --id jdx.mise --exact`.
    Winget,
    /// `scoop update mise`.
    Scoop,
    /// What the package says to run, such as an apt or dnf command.
    Instructions { message: String },
    /// Unknown; update mise the way it was installed.
    Unknown,
}

impl MiseUpdater {
    /// The program and arguments, for updaters the app can run itself (no
    /// administrator password needed).
    pub fn command(&self) -> Option<(&'static str, Vec<String>)> {
        match self {
            Self::Homebrew => Some(("brew", vec!["upgrade".into(), "mise".into()])),
            Self::Winget => Some((
                "winget",
                vec![
                    "upgrade".into(),
                    "--id".into(),
                    "jdx.mise".into(),
                    "--exact".into(),
                    "--accept-source-agreements".into(),
                ],
            )),
            Self::Scoop => Some(("scoop", vec!["update".into(), "mise".into()])),
            Self::Instructions { .. } | Self::Unknown => None,
        }
    }
}

/// Works out who owns the mise at `executable`. mise finds its install prefix
/// the same way: resolve symlinks, then go two folders up (`/usr/bin/mise` →
/// `/usr`).
pub fn mise_updater(executable: &Path) -> MiseUpdater {
    let resolved = fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let prefix = resolved
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    if let Some(message) = prefix.as_deref().and_then(instructions) {
        return MiseUpdater::Instructions { message };
    }
    let lower = resolved.to_string_lossy().to_lowercase().replace('\\', "/");
    if lower.contains("/cellar/mise/")
        || prefix.as_deref().is_some_and(|prefix| {
            prefix.join("lib/.disable-self-update").is_file() && lower.contains("homebrew")
        })
    {
        MiseUpdater::Homebrew
    } else if lower.contains("/microsoft/winget/") {
        MiseUpdater::Winget
    } else if lower.contains("/scoop/") {
        MiseUpdater::Scoop
    } else if lower.contains("/chocolatey/") {
        MiseUpdater::Instructions {
            message: "In PowerShell, run as administrator:\n\n  choco upgrade mise\n".into(),
        }
    } else {
        MiseUpdater::Unknown
    }
}

/// The packager's instructions file under the install prefix: its `message`,
/// or else the first other key's value (alphabetically), as mise prints it.
fn instructions(prefix: &Path) -> Option<String> {
    let candidates: Vec<PathBuf> = std::env::var_os("MISE_SELF_UPDATE_INSTRUCTIONS")
        .map(|path| vec![PathBuf::from(path)])
        .unwrap_or_else(|| {
            [
                "lib/mise-self-update-instructions.toml",
                "lib/mise/mise-self-update-instructions.toml",
                "lib64/mise/mise-self-update-instructions.toml",
            ]
            .iter()
            .map(|path| prefix.join(path))
            .collect()
        });
    candidates.iter().find_map(|path| {
        let table: toml::Table = fs::read_to_string(path).ok()?.parse().ok()?;
        if let Some(message) = table.get("message").and_then(toml::Value::as_str) {
            return Some(message.trim().to_owned());
        }
        let mut keys: Vec<_> = table.keys().collect();
        keys.sort();
        keys.into_iter()
            .find_map(|key| table[key].as_str())
            .map(|text| text.trim().to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_packager_instructions() {
        let root = std::env::temp_dir().join(format!("powermise-updater-{}", std::process::id()));
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(bin.join("mise"), "").unwrap();
        fs::write(
            root.join("lib/mise-self-update-instructions.toml"),
            "message = \"To update mise, run:\\n\\n  sudo apt install --only-upgrade mise\\n\"\n",
        )
        .unwrap();
        let updater = mise_updater(&bin.join("mise"));
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            updater,
            MiseUpdater::Instructions {
                message: "To update mise, run:\n\n  sudo apt install --only-upgrade mise".into()
            }
        );
    }

    #[test]
    fn recognizes_package_managers_by_path() {
        let cases = [
            (
                "/opt/homebrew/Cellar/mise/2026.10.10/bin/mise",
                MiseUpdater::Homebrew,
            ),
            (
                r"C:\Users\me\AppData\Local\Microsoft\WinGet\Packages\jdx.mise_x\mise\bin\mise.exe",
                MiseUpdater::Winget,
            ),
            (
                r"C:\Users\me\scoop\apps\mise\current\bin\mise.exe",
                MiseUpdater::Scoop,
            ),
            ("/home/me/.local/bin/mise", MiseUpdater::Unknown),
        ];
        for (path, expected) in cases {
            assert_eq!(mise_updater(Path::new(path)), expected, "{path}");
        }
    }
}
