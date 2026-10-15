//! Parses JSON captured from real mise (see `fixtures/<version>/`, produced by
//! `scripts/capture-mise-fixtures.sh`). Every check runs against each captured
//! version, so a change in mise's output shape anywhere in the supported range
//! (2026.7.18 minimum, 2026.10 recommended) fails here rather than in the app.
use std::path::PathBuf;

use powermise_mise::{
    ConfigFile, Doctor, EnvReport, MiseVersion, OutdatedReport, RegistryEntry, RemoteVersion, Task,
    ToolInventory, is_untrusted_error, parse_task_details, parse_trust_show,
};

/// Fixture sets, oldest first: the supported minimum and the recommended version.
const VERSIONS: &[&str] = &["2026.7.18", "2026.10.10"];

fn dir(version: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(version)
}

fn text(version: &str, name: &str) -> String {
    let path = dir(version).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn fixture<T: serde::de::DeserializeOwned>(version: &str, name: &str) -> T {
    serde_json::from_str(&text(version, name))
        .unwrap_or_else(|error| panic!("{version}/{name}: {error}"))
}

#[test]
fn ls_reports_versions_sources_and_install_state() {
    for version in VERSIONS {
        let global: ToolInventory = fixture(version, "ls-global.json");
        let jq = &global["jq"][0];
        assert_eq!(jq.version, "1.7.1", "{version}");
        assert!(jq.installed && jq.active, "{version}");
        assert!(
            jq.source
                .as_ref()
                .unwrap()
                .path
                .ends_with(".config/mise/config.toml"),
            "{version}"
        );

        let project: ToolInventory = fixture(version, "ls-project.json");
        let requested = project["jq"]
            .iter()
            .find(|entry| entry.requested_version.is_some() && entry.source.is_some())
            .unwrap_or_else(|| panic!("{version}: project jq request"));
        assert_eq!(
            requested.requested_version.as_deref(),
            Some("1.8"),
            "{version}"
        );
        assert!(project.contains_key("shellcheck"), "{version}");
    }
}

#[test]
fn outdated_reports_bumpable_versions() {
    for version in VERSIONS {
        let bump: OutdatedReport = fixture(version, "outdated-bump-global.json");
        assert_eq!(bump["jq"].current.as_deref(), Some("1.7.1"), "{version}");
        assert!(bump["jq"].bump.is_some(), "{version}");

        let project: OutdatedReport = fixture(version, "outdated-project.json");
        assert!(project.contains_key("shellcheck"), "{version}");
    }
}

#[test]
fn registry_and_remote_versions_parse() {
    for version in VERSIONS {
        let registry: Vec<RegistryEntry> = fixture(version, "registry-sample.json");
        let jq = registry.iter().find(|entry| entry.short == "jq").unwrap();
        assert!(!jq.backends.is_empty(), "{version}");
        assert!(jq.description.is_some(), "{version}");

        let versions: Vec<RemoteVersion> = fixture(version, "ls-remote-jq.json");
        assert!(
            versions.iter().any(|entry| entry.version == "1.7.1"),
            "{version}"
        );
    }
}

#[test]
fn project_config_tasks_and_doctor_parse() {
    for version in VERSIONS {
        let configs: Vec<ConfigFile> = fixture(version, "config-ls.json");
        assert!(configs[0].path.ends_with("code/app/mise.toml"), "{version}");
        assert_eq!(configs[0].tools, ["jq", "shellcheck"], "{version}");

        let tasks: Vec<Task> = fixture(version, "tasks-ls.json");
        let build = tasks.iter().find(|task| task.name == "build").unwrap();
        assert_eq!(build.description, "Build it", "{version}");
        assert_eq!(build.run, ["echo build"], "{version}");

        let doctor: Doctor = fixture(version, "doctor.json");
        assert!(MiseVersion::parse(&doctor.version).is_some(), "{version}");
        assert!(doctor.shell.is_some(), "{version}");
    }
}

#[test]
fn project_env_trust_and_task_details_parse() {
    for version in VERSIONS {
        let env: EnvReport = fixture(version, "env-extended.json");
        assert_eq!(env["APP_ENV"].value, "dev", "{version}");
        assert!(
            env["SECRET_TOKEN"]
                .source
                .as_ref()
                .unwrap()
                .ends_with("code/app/mise.toml"),
            "{version}"
        );

        let redacted: std::collections::BTreeMap<String, String> =
            fixture(version, "env-redacted.json");
        assert!(redacted.contains_key("SECRET_TOKEN"), "{version}");

        let trusted = parse_trust_show(&text(version, "trust-show-trusted.txt")).unwrap();
        assert!(trusted.trusted, "{version}");
        assert!(trusted.path.ends_with("code/app"), "{version}");
        assert!(
            !parse_trust_show(&text(version, "trust-show-untrusted.txt"))
                .unwrap()
                .trusted,
            "{version}"
        );
        assert!(
            is_untrusted_error(&text(version, "untrusted-error.txt")),
            "{version}"
        );

        let task: serde_json::Value = fixture(version, "task-info-deploy.json");
        let details = parse_task_details(&task);
        assert_eq!(details.name, "deploy", "{version}");
        assert_eq!(details.args.len(), 1, "{version}");
        assert_eq!(details.args[0].name, "env", "{version}");
        assert!(details.args[0].required, "{version}");
        assert_eq!(details.args[0].choices, ["staging", "prod"], "{version}");
        assert_eq!(details.flags.len(), 1, "{version}");
        assert_eq!(details.flags[0].usage, "--dry-run", "{version}");
        assert!(!details.flags[0].takes_value, "{version}");
    }
}

#[test]
fn fixture_sets_stay_in_the_supported_range() {
    let newest = MiseVersion::parse(VERSIONS.last().unwrap()).unwrap();
    assert_eq!(
        MiseVersion::parse(VERSIONS[0]).unwrap(),
        MiseVersion::MINIMUM
    );
    assert!(newest >= MiseVersion::RECOMMENDED);
}

#[test]
fn mise_versions_compare_against_the_minimum() {
    let installed = MiseVersion::parse("2026.7.18 macos-arm64 (2026-07-30)").unwrap();
    assert_eq!(installed, MiseVersion::MINIMUM);
    assert!(MiseVersion::parse("2026.10.10").unwrap() > MiseVersion::MINIMUM);
    assert!(MiseVersion::parse("2025.12.0").unwrap() < MiseVersion::MINIMUM);
    assert_eq!(MiseVersion::parse("not a version"), None);
    assert!(MiseVersion::RECOMMENDED > MiseVersion::MINIMUM);
    assert!(installed < MiseVersion::RECOMMENDED);
}

#[test]
fn bootstrap_plan_and_status_parse() {
    use powermise_mise::{BootstrapPlan, BootstrapStatus};
    // Captured from 2026.10.10, the release AI Setups requires.
    let plan: BootstrapPlan = fixture("2026.10.10", "bootstrap-plan.json");
    let hello = plan
        .resources
        .iter()
        .find(|resource| resource.id.name == "brew:hello")
        .unwrap();
    assert_eq!(hello.id.kind, "package");
    assert_eq!(hello.action, "create");
    assert!(plan.summary.create >= 1);

    let status: BootstrapStatus = fixture("2026.10.10", "bootstrap-status.json");
    let brew = &status.packages["brew"];
    assert!(
        brew.packages
            .iter()
            .any(|package| package.package == "hello" && package.state == "missing")
    );
    assert!(
        status
            .tools
            .iter()
            .any(|tool| tool.tool == "jq" && tool.installed)
    );
}
