//! Developer basics mise and the tools it installs rely on, per platform —
//! Apple's Command Line Tools, a Linux compiler with make, git, and curl,
//! Git for Windows, and a PowerShell that runs your profile — with a fix for
//! each that's missing.
//!
//! mise's own Node, Python, Ruby, and Go are ready-built, so most people never
//! notice these. git (agent plugins, skills, Git-based tools), gems and npm
//! packages with native code, and `cargo install` do.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::{CommandResult, find_mise, find_program, home_dir, message};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCheck {
    pub id: &'static str,
    pub title: String,
    pub ok: bool,
    /// What's missing and why it matters, or what's there.
    pub detail: String,
    /// The fix, when powermise has one.
    pub fix: Option<SystemFix>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFix {
    pub label: String,
    /// The exact command, as shown before it runs.
    pub command: String,
    /// Runs in your terminal app, where `sudo` can ask for your password.
    pub terminal: bool,
}

/// One program a fix runs.
pub struct FixStep {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub display: String,
}

pub struct FixPlan {
    pub steps: Vec<FixStep>,
    pub terminal: bool,
}

struct Check {
    view: SystemCheck,
    plan: Option<FixPlan>,
}

fn step(program: PathBuf, name: &str, arguments: &[&str]) -> FixStep {
    FixStep {
        display: std::iter::once(name)
            .chain(arguments.iter().copied())
            .map(|word| {
                if word.contains(' ') {
                    format!("\"{word}\"")
                } else {
                    word.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        program,
        arguments: arguments.iter().map(|&word| word.to_owned()).collect(),
    }
}

fn check(
    id: &'static str,
    title: &str,
    ok: bool,
    detail: String,
    label: &str,
    plan: Option<FixPlan>,
) -> Check {
    let plan = plan.filter(|_| !ok);
    Check {
        view: SystemCheck {
            id,
            title: title.into(),
            ok,
            detail,
            fix: plan.as_ref().map(|plan| SystemFix {
                label: label.into(),
                command: plan
                    .steps
                    .iter()
                    .map(|step| step.display.as_str())
                    .collect::<Vec<_>>()
                    .join(" && "),
                terminal: plan.terminal,
            }),
        },
        plan,
    }
}

/// `mise bootstrap packages apply` for packages no config declares (mise
/// installs them without recording them).
fn mise_packages(packages: &[String], update: bool) -> Option<FixStep> {
    let mise = find_mise()?;
    let mut arguments = vec!["bootstrap", "packages", "apply", "--yes"];
    if update {
        arguments.push("--update");
    }
    arguments.extend(packages.iter().map(String::as_str));
    Some(step(mise, "mise", &arguments))
}

fn checks() -> Vec<Check> {
    if cfg!(target_os = "macos") {
        vec![command_line_tools()]
    } else if cfg!(windows) {
        let mut checks = vec![git_for_windows()];
        checks.extend(powershell_policy());
        checks
    } else {
        vec![linux_build_tools()]
    }
}

/// Whether Apple's Command Line Tools (or Xcode) are installed. Until they
/// are, `/usr/bin/git` and `/usr/bin/make` only open Apple's installer.
pub fn command_line_tools_installed() -> bool {
    Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
        .is_some_and(|path| path.join("usr/bin/git").exists())
}

/// Apple's Command Line Tools: git, clang, and make.
fn command_line_tools() -> Check {
    let ok = command_line_tools_installed();
    check(
        "command-line-tools",
        "Apple’s Command Line Tools",
        ok,
        if ok {
            "git, clang, and make are installed.".into()
        } else {
            "git, clang, and make come with them. Without them, git opens Apple’s installer, and gems, npm packages with native code, and cargo install can’t build.".into()
        },
        "Install…",
        Some(FixPlan {
            // Opens Apple's own installer window, which asks for anything it needs.
            steps: vec![step(
                PathBuf::from("/usr/bin/xcode-select"),
                "xcode-select",
                &["--install"],
            )],
            terminal: false,
        }),
    )
}

/// A C compiler, make, git, and curl, from the distribution's package manager
/// through mise (`apt:build-essential` and so on). Installing needs `sudo`, so
/// it runs in the terminal.
fn linux_build_tools() -> Check {
    let compiler = find_program("cc").or_else(|| find_program("gcc")).is_some();
    let make = find_program("make").is_some();
    let git = find_program("git").is_some();
    let curl = find_program("curl").is_some();
    let mut missing = Vec::new();
    if !compiler {
        missing.push("a C compiler");
    }
    if !make {
        missing.push("make");
    }
    if !git {
        missing.push("git");
    }
    if !curl {
        missing.push("curl");
    }
    let manager = [
        ("apt-get", "apt"),
        ("dnf", "dnf"),
        ("pacman", "pacman"),
        ("zypper", "zypper"),
        ("apk", "apk"),
    ]
    .into_iter()
    .find(|(program, _)| find_program(program).is_some())
    .map(|(_, manager)| manager);
    let plan = manager.and_then(|manager| {
        let mut packages: Vec<&str> = Vec::new();
        if !compiler || !make {
            packages.extend(match manager {
                "apt" => &["build-essential"][..],
                "pacman" => &["base-devel"][..],
                "apk" => &["build-base"][..],
                _ => &["gcc", "gcc-c++", "make"][..],
            });
        }
        if !git {
            packages.push("git");
        }
        if !curl {
            packages.push("curl");
        }
        let specs: Vec<String> = packages
            .iter()
            .map(|package| format!("{manager}:{package}"))
            .collect();
        Some(FixPlan {
            steps: vec![mise_packages(&specs, true)?],
            terminal: true,
        })
    });
    let ok = missing.is_empty();
    check(
        "build-tools",
        "Build tools",
        ok,
        if ok {
            "A C compiler, make, git, and curl are installed.".into()
        } else {
            format!(
                "Missing {}. Gems and npm packages with native code, cargo install, and tools mise builds from source need them{}.",
                join_words(&missing),
                if manager.is_none() {
                    "; install them with your package manager"
                } else {
                    ""
                }
            )
        },
        "Install in Terminal…",
        plan,
    )
}

/// Git for Windows, through mise's winget support.
fn git_for_windows() -> Check {
    let ok = find_program("git").is_some();
    let plan = find_program("winget").and_then(|_| {
        Some(FixPlan {
            steps: vec![mise_packages(&["winget:Git.Git".into()], false)?],
            terminal: false,
        })
    });
    check(
        "git",
        "Git for Windows",
        ok,
        if ok {
            "git is installed.".into()
        } else {
            "Agent plugins, skills, and tools that install from Git repositories need git.".into()
        },
        "Install",
        plan,
    )
}

/// Windows PowerShell 5.1 and PowerShell 7 each keep an execution policy. The
/// default on Windows desktops, `Restricted`, won't run `$PROFILE`, so the
/// line that turns mise on is silently skipped.
fn powershell_policy() -> Option<Check> {
    let hosts: Vec<(&str, &str, PathBuf)> = [
        ("Windows PowerShell", "powershell"),
        ("PowerShell 7", "pwsh"),
    ]
    .into_iter()
    .filter_map(|(title, name)| Some((title, name, find_program(name)?)))
    .collect();
    if hosts.is_empty() {
        return None;
    }
    let mut blocked = Vec::new();
    let mut managed = Vec::new();
    let mut steps = Vec::new();
    for (title, name, program) in hosts {
        let Some((effective, policy_set)) = execution_policy(&program) else {
            continue;
        };
        if !matches!(effective.as_str(), "Restricted" | "AllSigned") {
            continue;
        }
        blocked.push(title);
        if policy_set {
            managed.push(title);
        } else {
            steps.push(step(
                program,
                name,
                &[
                    "-NoProfile",
                    "-Command",
                    "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force",
                ],
            ));
        }
    }
    let ok = blocked.is_empty();
    Some(check(
        "powershell-policy",
        "PowerShell script policy",
        ok,
        if ok {
            "PowerShell runs $PROFILE, where the line that turns mise on goes.".into()
        } else if !managed.is_empty() {
            format!(
                "{} won’t run $PROFILE, so mise can’t turn itself on there, and your organization sets that policy. Ask your IT team, or use Command Prompt’s setup instead.",
                join_words(&blocked)
            )
        } else {
            format!(
                "{} won’t run $PROFILE (its script policy is Restricted), so the line that turns mise on is skipped. RemoteSigned for your account runs your own scripts and signed downloaded ones.",
                join_words(&blocked)
            )
        },
        "Allow Profile Scripts",
        (!steps.is_empty()).then_some(FixPlan {
            steps,
            terminal: false,
        }),
    ))
}

/// The effective policy, and whether Group Policy sets it.
fn execution_policy(program: &Path) -> Option<(String, bool)> {
    let output = Command::new(program)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-ExecutionPolicy; Get-ExecutionPolicy -Scope MachinePolicy; Get-ExecutionPolicy -Scope UserPolicy",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let effective = lines.next()?.to_owned();
    let policy_set = lines.any(|line| line != "Undefined");
    Some((effective, policy_set))
}

fn join_words(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first, last] => format!("{first} and {last}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

#[tauri::command]
pub async fn system_checks() -> CommandResult<Vec<SystemCheck>> {
    tauri::async_runtime::spawn_blocking(|| checks().into_iter().map(|check| check.view).collect())
        .await
        .map_err(message)
}

/// The fix for check `id`, worked out again here so the window only ever
/// names a check, never a command.
pub fn fix_plan(id: &str) -> CommandResult<FixPlan> {
    checks()
        .into_iter()
        .find(|check| check.view.id == id)
        .and_then(|check| check.plan)
        .ok_or_else(|| format!("nothing to fix for {id}"))
}

/// Runs a fix that needs your password in your terminal app.
#[tauri::command]
pub async fn system_fix_in_terminal(id: String) -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = fix_plan(&id)?;
        let [step] = plan.steps.as_slice() else {
            return Err("this fix doesn't run in the terminal".into());
        };
        let home = home_dir().ok_or("could not determine your home directory")?;
        crate::terminal::open(&home, &step.program, &step.arguments).map_err(message)
    })
    .await
    .map_err(message)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_words_like_prose() {
        assert_eq!(join_words(&["make"]), "make");
        assert_eq!(join_words(&["make", "git"]), "make and git");
        assert_eq!(
            join_words(&["a C compiler", "make", "git"]),
            "a C compiler, make, and git"
        );
    }

    #[test]
    fn a_check_that_passes_offers_no_fix() {
        let plan = FixPlan {
            steps: vec![step(PathBuf::from("/bin/true"), "true", &[])],
            terminal: false,
        };
        let passing = check("x", "X", true, String::new(), "Fix", Some(plan));
        assert!(passing.view.fix.is_none() && passing.plan.is_none());
    }

    #[test]
    fn shows_the_exact_command() {
        let fix = step(
            PathBuf::from("pwsh"),
            "pwsh",
            &[
                "-NoProfile",
                "-Command",
                "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force",
            ],
        );
        assert_eq!(
            fix.display,
            "pwsh -NoProfile -Command \"Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force\""
        );
    }
}
