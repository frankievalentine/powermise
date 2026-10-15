//! More things to bring into mise: what Homebrew installed, Brewfiles, Mac
//! App Store apps, and asdf-style default-package files.
//!
//! All of it is read from files (Homebrew's install receipts, a Brewfile,
//! `mas list` output); nothing is installed or changed here.
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::imports::HOMEBREW_PREFIXES;

/// A Homebrew formula or cask.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrewItem {
    /// As mise names it: `ripgrep`, or `mongodb/brew/mongodb-community` for a
    /// formula from another tap.
    pub name: String,
    pub version: Option<String>,
}

/// What Homebrew installed on this computer.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomebrewInventory {
    /// Formulae you installed yourself (dependencies are left out).
    pub formulas: Vec<BrewItem>,
    pub casks: Vec<BrewItem>,
}

/// The tap-qualified name, for taps other than Homebrew's own.
fn qualified(name: &str, tap: Option<&str>) -> String {
    match tap {
        Some(tap) if tap != "homebrew/core" && tap != "homebrew/cask" => format!("{tap}/{name}"),
        _ => name.to_owned(),
    }
}

fn receipt(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn subdirs(directory: &Path) -> Vec<(String, PathBuf)> {
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

/// Formulae installed on request and casks, from Homebrew's install
/// receipts under each prefix.
pub fn homebrew_inventory(prefixes: &[&str]) -> HomebrewInventory {
    let mut inventory = HomebrewInventory::default();
    for prefix in prefixes {
        let prefix = Path::new(prefix);
        for (name, path) in subdirs(&prefix.join("Cellar")) {
            // The newest version's receipt says whether you asked for it.
            let Some((version, version_path)) = subdirs(&path).into_iter().next_back() else {
                continue;
            };
            let Some(receipt) = receipt(&version_path.join("INSTALL_RECEIPT.json")) else {
                continue;
            };
            if receipt.get("installed_on_request") != Some(&serde_json::Value::Bool(true)) {
                continue;
            }
            let tap = receipt
                .pointer("/source/tap")
                .and_then(serde_json::Value::as_str);
            let item = BrewItem {
                name: qualified(&name, tap),
                version: Some(version.split('_').next().unwrap_or(&version).to_owned()),
            };
            if !inventory.formulas.contains(&item) {
                inventory.formulas.push(item);
            }
        }
        for (name, path) in subdirs(&prefix.join("Caskroom")) {
            let receipt = receipt(&path.join(".metadata/INSTALL_RECEIPT.json"));
            let tap = receipt
                .as_ref()
                .and_then(|receipt| receipt.pointer("/source/tap"))
                .and_then(serde_json::Value::as_str);
            let version = subdirs(&path)
                .into_iter()
                .next_back()
                .map(|(version, _)| version);
            let item = BrewItem {
                name: qualified(&name, tap),
                version,
            };
            if !inventory.casks.iter().any(|cask| cask.name == item.name) {
                inventory.casks.push(item);
            }
        }
    }
    inventory
}

/// What a Brewfile asks for.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brewfile {
    pub path: PathBuf,
    pub taps: Vec<String>,
    /// Formula names, tap-qualified when the Brewfile qualifies them.
    pub formulas: Vec<String>,
    pub casks: Vec<String>,
    /// `(name, App Store id)`.
    pub mas: Vec<(String, String)>,
    /// Lines powermise doesn't import (`vscode`, `whalebrew`, …), for the
    /// review.
    pub skipped: Vec<String>,
}

/// The first string argument of a Brewfile line: `brew "ripgrep", args: […]`
/// → `ripgrep`.
fn first_string(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = &rest[1..];
    rest.find(quote).map(|end| rest[..end].to_owned())
}

/// Reads a Brewfile's `tap`, `brew`, `cask`, and `mas` lines. Comments and
/// options (`args:`, `restart_service:`) are ignored.
pub fn parse_brewfile(path: &Path, text: &str) -> Brewfile {
    let mut brewfile = Brewfile {
        path: path.to_path_buf(),
        ..Brewfile::default()
    };
    for line in text.lines() {
        let line = line.split(" #").next().unwrap_or(line).trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let Some(name) = first_string(rest) else {
            continue;
        };
        match word {
            "tap" => brewfile.taps.push(name),
            "brew" => brewfile.formulas.push(name),
            "cask" => brewfile.casks.push(name),
            "mas" => {
                // `mas "Magnet", id: 441258766`
                let id = rest
                    .split("id:")
                    .nth(1)
                    .map(|id| id.trim().trim_end_matches(',').to_owned())
                    .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
                match id {
                    Some(id) => brewfile.mas.push((name, id)),
                    None => brewfile.skipped.push(line.to_owned()),
                }
            }
            _ => brewfile.skipped.push(line.to_owned()),
        }
    }
    brewfile
}

/// Where a Brewfile usually lives: `$HOMEBREW_BUNDLE_FILE`, then the home
/// folder's and `~/.config/homebrew`'s.
pub fn brewfile_paths(home: &Path) -> Vec<PathBuf> {
    std::env::var_os("HOMEBREW_BUNDLE_FILE")
        .map(PathBuf::from)
        .into_iter()
        .chain([
            home.join("Brewfile"),
            home.join(".Brewfile"),
            home.join(".config/homebrew/Brewfile"),
            home.join(".homebrew/Brewfile"),
        ])
        .filter(|path| path.is_file())
        .collect()
}

/// A Mac App Store app from `mas list`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasApp {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
}

/// `mas list` output: `441258766  Magnet  (3.0.7)` per line.
pub fn parse_mas_list(text: &str) -> Vec<MasApp> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (id, rest) = line.split_once(char::is_whitespace)?;
            if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let rest = rest.trim();
            let (name, version) = match rest.rfind(" (") {
                Some(index) if rest.ends_with(')') => (
                    rest[..index].trim().to_owned(),
                    Some(rest[index + 2..rest.len() - 1].to_owned()),
                ),
                _ => (rest.to_owned(), None),
            };
            Some(MasApp {
                id: id.to_owned(),
                name,
                version,
            })
        })
        .collect()
}

/// A package from an asdf-style default-packages file, which mise now
/// recommends installing as a tool (`npm:`, `pipx:`, `gem:`) instead.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultPackage {
    /// The mise backend: `npm`, `pipx`, or `gem`.
    pub backend: &'static str,
    pub name: String,
    pub version: Option<String>,
    pub file: PathBuf,
}

/// Packages listed in `~/.default-npm-packages` (and `-nodejs-`/`-node-`),
/// `~/.default-python-packages`, and `~/.default-gems`.
pub fn default_packages(home: &Path) -> Vec<DefaultPackage> {
    let mut found = Vec::new();
    let mut read =
        |backend: &'static str, file: PathBuf, split: fn(&str) -> (String, Option<String>)| {
            let Ok(text) = fs::read_to_string(&file) else {
                return;
            };
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or_default().trim();
                // Skip pip options and blank lines.
                if line.is_empty() || line.starts_with('-') {
                    continue;
                }
                let (name, version) = split(line);
                if !name.is_empty()
                    && !found
                        .iter()
                        .any(|p: &DefaultPackage| p.backend == backend && p.name == name)
                {
                    found.push(DefaultPackage {
                        backend,
                        name,
                        version,
                        file: file.clone(),
                    });
                }
            }
        };
    for name in [
        ".default-npm-packages",
        ".default-nodejs-packages",
        ".default-node-packages",
    ] {
        // `typescript@5` or `@types/node@^24`: the version is after the last `@`
        // that isn't the scope's.
        read("npm", home.join(name), |line| {
            let word = line.split_whitespace().next().unwrap_or(line);
            match word[1..].rfind('@') {
                Some(index) => (
                    word[..index + 1].to_owned(),
                    Some(word[index + 2..].to_owned())
                        .filter(|v| v.starts_with(|c: char| c.is_ascii_digit())),
                ),
                None => (word.to_owned(), None),
            }
        });
    }
    // `black==24.8.0` or `httpie`.
    read(
        "pipx",
        home.join(".default-python-packages"),
        |line| match line.split_once("==") {
            Some((name, version)) => (name.trim().to_owned(), Some(version.trim().to_owned())),
            None => (
                line.split(|c: char| "<>=!~[ ;".contains(c))
                    .next()
                    .unwrap_or(line)
                    .to_owned(),
                None,
            ),
        },
    );
    // `rubocop 1.65.0` or `rubocop -v 1.65.0`.
    read("gem", home.join(".default-gems"), |line| {
        let mut words = line
            .split_whitespace()
            .filter(|word| *word != "-v" && *word != "--version");
        let name = words.next().unwrap_or_default().to_owned();
        let version = words
            .next()
            .map(str::to_owned)
            .filter(|version| version.starts_with(|c: char| c.is_ascii_digit()));
        (name, version)
    });
    found
}

/// Homebrew's prefixes, for [`homebrew_inventory`].
pub fn homebrew_prefixes() -> &'static [&'static str] {
    &HOMEBREW_PREFIXES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("powermise-sources-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_homebrew_receipts() {
        let prefix = temp("brew");
        let write = |path: &str, text: &str| {
            let path = prefix.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "Cellar/ripgrep/15.2.0/INSTALL_RECEIPT.json",
            r#"{"installed_on_request":true,"source":{"tap":"homebrew/core"}}"#,
        );
        write(
            "Cellar/pcre2/10.44/INSTALL_RECEIPT.json",
            r#"{"installed_on_request":false,"source":{"tap":"homebrew/core"}}"#,
        );
        write(
            "Cellar/mongodb-community/8.0.4_1/INSTALL_RECEIPT.json",
            r#"{"installed_on_request":true,"source":{"tap":"mongodb/brew"}}"#,
        );
        write(
            "Caskroom/ghostty/.metadata/INSTALL_RECEIPT.json",
            r#"{"source":{"tap":"homebrew/cask"}}"#,
        );
        fs::create_dir_all(prefix.join("Caskroom/ghostty/1.3.1")).unwrap();
        let inventory = homebrew_inventory(&[prefix.to_str().unwrap()]);
        fs::remove_dir_all(&prefix).unwrap();
        let formulas: Vec<_> = inventory
            .formulas
            .iter()
            .map(|item| (item.name.as_str(), item.version.as_deref()))
            .collect();
        assert_eq!(
            formulas,
            [
                ("mongodb/brew/mongodb-community", Some("8.0.4")),
                ("ripgrep", Some("15.2.0"))
            ]
        );
        assert_eq!(inventory.casks[0].name, "ghostty");
        assert_eq!(inventory.casks[0].version.as_deref(), Some("1.3.1"));
    }

    #[test]
    fn parses_a_brewfile() {
        let brewfile = parse_brewfile(
            Path::new("/Brewfile"),
            r#"
# Taps
tap "mongodb/brew"
brew "ripgrep"
brew "uv" # python package manager
brew "postgresql@17", restart_service: true
#brew "railway"
cask "ghostty"
mas "Magnet", id: 441258766 # (Paid)
vscode "rust-lang.rust-analyzer"
"#,
        );
        assert_eq!(brewfile.taps, ["mongodb/brew"]);
        assert_eq!(brewfile.formulas, ["ripgrep", "uv", "postgresql@17"]);
        assert_eq!(brewfile.casks, ["ghostty"]);
        assert_eq!(
            brewfile.mas,
            [("Magnet".to_owned(), "441258766".to_owned())]
        );
        assert_eq!(brewfile.skipped, [r#"vscode "rust-lang.rust-analyzer""#]);
    }

    #[test]
    fn parses_mas_list() {
        let apps = parse_mas_list(
            "1447043133  Cursor Pro     (2.16)\n 441258766  Magnet         (3.0.7)\nNo installed apps\n",
        );
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "Cursor Pro");
        assert_eq!(apps[1].id, "441258766");
        assert_eq!(apps[1].version.as_deref(), Some("3.0.7"));
    }

    #[test]
    fn reads_default_package_files() {
        let home = temp("defaults");
        fs::write(
            home.join(".default-npm-packages"),
            "typescript@5.6.2\n@types/node@^24\n# comment\nprettier\n",
        )
        .unwrap();
        fs::write(
            home.join(".default-python-packages"),
            "black==24.8.0\n-r other.txt\nhttpie>=3\n",
        )
        .unwrap();
        fs::write(home.join(".default-gems"), "rubocop -v 1.65.0\nbundler\n").unwrap();
        let packages = default_packages(&home);
        fs::remove_dir_all(&home).unwrap();
        let found: Vec<_> = packages
            .iter()
            .map(|p| (p.backend, p.name.as_str(), p.version.as_deref()))
            .collect();
        assert_eq!(
            found,
            [
                ("npm", "typescript", Some("5.6.2")),
                ("npm", "@types/node", None),
                ("npm", "prettier", None),
                ("pipx", "black", Some("24.8.0")),
                ("pipx", "httpie", None),
                ("gem", "rubocop", Some("1.65.0")),
                ("gem", "bundler", None),
            ]
        );
    }
}
