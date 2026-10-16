//! Newer versions of the tools macOS ships: git, bash, make, and curl through
//! mise's Homebrew packages (in `conf.d/powermise-system.toml`, which also puts
//! each one's own `opt/<formula>/bin` folder first on PATH), and Ruby as a
//! mise tool.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::{CommandResult, find_mise, message, mise_config_dir};

const PREFIX: &str = "/opt/homebrew";

struct Spec {
    id: &'static str,
    title: &'static str,
    note: &'static str,
    /// macOS's own copy.
    system: &'static str,
    /// The Homebrew formula, or none for a mise tool (Ruby).
    formula: Option<&'static str>,
    /// Homebrew doesn't link it into `/opt/homebrew/bin`.
    keg_only: bool,
    /// macOS's copy is a stub until Apple's Command Line Tools are installed
    /// (running it opens their installer).
    needs_command_line_tools: bool,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "git",
        title: "git",
        note: "Apple’s build, from the Command Line Tools",
        system: "/usr/bin/git",
        formula: Some("git"),
        keg_only: false,
        needs_command_line_tools: true,
    },
    Spec {
        id: "bash",
        title: "bash",
        note: "From 2007; your login shell doesn’t change",
        system: "/bin/bash",
        formula: Some("bash"),
        keg_only: false,
        needs_command_line_tools: false,
    },
    Spec {
        id: "make",
        title: "make",
        note: "Some build scripts need a newer one",
        system: "/usr/bin/make",
        formula: Some("make"),
        keg_only: false,
        needs_command_line_tools: true,
    },
    Spec {
        id: "curl",
        title: "curl",
        note: "Homebrew keeps it off your PATH; mise puts it there",
        system: "/usr/bin/curl",
        formula: Some("curl"),
        keg_only: true,
        needs_command_line_tools: false,
    },
    Spec {
        id: "ruby",
        title: "ruby",
        note: "A mise tool, like Node and Python",
        system: "/usr/bin/ruby",
        formula: None,
        keg_only: false,
        needs_command_line_tools: false,
    },
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewerTool {
    id: &'static str,
    title: &'static str,
    note: &'static str,
    /// `homebrew` (installed by mise, folder on PATH) or `mise` (a mise tool).
    kind: &'static str,
    system_version: Option<String>,
    /// Homebrew's version (installed, or what mise would install).
    newer_version: Option<String>,
    /// Installed, and first on PATH through powermise's file or Homebrew's
    /// own setup. Always false for mise tools; the window checks those.
    using_newer: bool,
}

/// The first version number in a program's `--version` output.
fn version_of(program: &Path) -> Option<String> {
    let output = Command::new(program).arg("--version").output().ok()?;
    parse_version(String::from_utf8_lossy(&output.stdout).lines().next()?)
}

/// `3.2.57` from `GNU bash, version 3.2.57(1)-release`.
fn parse_version(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|word| {
        let number: String = word
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        (number.contains('.') && number.starts_with(|c: char| c.is_ascii_digit()))
            .then(|| number.trim_end_matches('.').to_owned())
    })
}

/// The newest version in `/opt/homebrew/Cellar/<formula>`, without
/// Homebrew's `_1` revision suffix.
fn cellar_version(formula: &str) -> Option<String> {
    let mut versions: Vec<Vec<u64>> = Vec::new();
    let mut names = Vec::new();
    for entry in std::fs::read_dir(Path::new(PREFIX).join("Cellar").join(formula)).ok()? {
        let name = entry.ok()?.file_name().to_string_lossy().into_owned();
        let plain = name.split('_').next().unwrap_or(&name).to_owned();
        versions.push(
            plain
                .split('.')
                .filter_map(|part| part.parse().ok())
                .collect(),
        );
        names.push(plain);
    }
    let newest = (0..names.len()).max_by(|&a, &b| versions[a].cmp(&versions[b]))?;
    Some(names.swap_remove(newest))
}

/// What `mise bootstrap packages apply --dry-run` would pour for formulas
/// that aren't installed: `pour curl/8.23.0 (requested)`.
fn dry_run_versions(formulas: &[&str]) -> Vec<(String, String)> {
    let Some(mise) = find_mise() else {
        return Vec::new();
    };
    let Ok(output) = Command::new(mise)
        .args(["bootstrap", "packages", "apply", "--dry-run"])
        .args(formulas.iter().map(|formula| format!("brew:{formula}")))
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    text.lines()
        .filter(|line| line.ends_with("(requested)"))
        .filter_map(|line| {
            let spec = line.strip_prefix("pour ")?.split_whitespace().next()?;
            let (name, version) = spec.rsplit_once('/')?;
            let formula = name.rsplit('/').next()?;
            Some((formula.to_owned(), version.split('_').next()?.to_owned()))
        })
        .collect()
}

fn keg_path(formula: &str) -> String {
    format!("{PREFIX}/opt/{formula}/bin")
}

/// mise's Homebrew packages need an Apple Silicon Mac.
fn homebrew_packages_work() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

#[tauri::command]
pub async fn newer_tools() -> CommandResult<Vec<NewerTool>> {
    if !cfg!(target_os = "macos") {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(read_newer_tools)
        .await
        .map_err(message)?
}

fn read_newer_tools() -> CommandResult<Vec<NewerTool>> {
    {
        let config = powermise_setups::read_system_tools(&mise_config_dir()?);
        let tools_installed = crate::system::command_line_tools_installed();
        let homebrew = homebrew_packages_work();
        let specs: Vec<&Spec> = SPECS
            .iter()
            .filter(|spec| homebrew || spec.formula.is_none())
            .collect();
        let missing: Vec<&str> = specs
            .iter()
            .filter_map(|spec| spec.formula)
            .filter(|formula| cellar_version(formula).is_none())
            .collect();
        let poured = if missing.is_empty() {
            Vec::new()
        } else {
            dry_run_versions(&missing)
        };
        let ruby_latest = find_mise().and_then(|mise| {
            let output = Command::new(mise).args(["latest", "ruby"]).output().ok()?;
            let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            (output.status.success() && !version.is_empty()).then_some(version)
        });
        Ok(specs
            .into_iter()
            .map(|spec| {
                let system_version = (tools_installed || !spec.needs_command_line_tools)
                    .then(|| version_of(Path::new(spec.system)))
                    .flatten();
                let Some(formula) = spec.formula else {
                    return NewerTool {
                        id: spec.id,
                        title: spec.title,
                        note: spec.note,
                        kind: "mise",
                        system_version,
                        newer_version: ruby_latest.clone(),
                        using_newer: false,
                    };
                };
                let installed = cellar_version(formula);
                let on_path = config.paths().contains(&keg_path(formula))
                    // Homebrew's own setup line puts its folder on PATH, but
                    // not keg-only formulas'.
                    || (!spec.keg_only && Path::new(PREFIX).join("bin/brew").exists());
                NewerTool {
                    id: spec.id,
                    title: spec.title,
                    note: spec.note,
                    kind: "homebrew",
                    system_version,
                    using_newer: installed.is_some() && on_path,
                    newer_version: installed.or_else(|| {
                        poured
                            .iter()
                            .find(|(name, _)| name == formula)
                            .map(|(_, version)| version.clone())
                    }),
                }
            })
            .collect())
    }
}

/// The Homebrew formulas for these tool ids, refusing anything else.
pub fn formulas_for(ids: &[String]) -> CommandResult<Vec<&'static str>> {
    ids.iter()
        .map(|id| {
            SPECS
                .iter()
                .find(|spec| spec.id == id)
                .and_then(|spec| spec.formula)
                .ok_or_else(|| format!("`{id}` isn't a tool powermise updates"))
        })
        .collect()
}

fn config_with(ids: &[String]) -> CommandResult<(PathBuf, powermise_setups::SystemToolsConfig)> {
    let formulas = formulas_for(ids)?;
    let packages: Vec<String> = formulas
        .iter()
        .map(|formula| format!("brew:{formula}"))
        .collect();
    let mise = mise_config_dir()?;
    // Each formula's own folder goes on PATH, not `/opt/homebrew/bin`, which
    // also holds their dependencies (Homebrew's python3) ahead of mise's tools.
    let config = powermise_setups::read_system_tools(&mise).merged(&packages);
    Ok((mise, config))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewerToolsFile {
    path: PathBuf,
    contents: String,
}

/// The file powermise would write for these tools. Writes nothing.
#[tauri::command]
pub fn newer_tools_preview(ids: Vec<String>) -> CommandResult<NewerToolsFile> {
    let (mise, config) = config_with(&ids)?;
    powermise_setups::inspect_system_tools(&mise, &config).map_err(message)?;
    Ok(NewerToolsFile {
        path: powermise_setups::system_tools_path(&mise),
        contents: config.contents(),
    })
}

/// Adds these tools to `conf.d/powermise-system.toml`. Installing them is
/// the separate `useNewerTools` run.
#[tauri::command]
pub fn write_newer_tools(ids: Vec<String>) -> CommandResult<PathBuf> {
    let (mise, config) = config_with(&ids)?;
    powermise_setups::write_system_tools(&mise, &config).map_err(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_version_lines() {
        let read = |line| parse_version(line);
        assert_eq!(
            read("git version 2.39.5 (Apple Git-154)").as_deref(),
            Some("2.39.5")
        );
        assert_eq!(
            read("GNU bash, version 3.2.57(1)-release (arm64-apple-darwin25)").as_deref(),
            Some("3.2.57")
        );
        assert_eq!(
            read("ruby 2.6.10p210 (2022-04-12 revision 67958)").as_deref(),
            Some("2.6.10")
        );
        assert_eq!(read("GNU Make 3.81").as_deref(), Some("3.81"));
        assert_eq!(
            read("curl 8.7.1 (x86_64-apple-darwin25.0) libcurl/8.7.1").as_deref(),
            Some("8.7.1")
        );
    }

    #[test]
    fn only_known_tools_are_updated() {
        assert_eq!(formulas_for(&["curl".into()]).unwrap(), ["curl"]);
        assert!(formulas_for(&["ruby".into()]).is_err());
        assert!(formulas_for(&["rm -rf".into()]).is_err());
    }
}
