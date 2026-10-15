//! Parses JSON captured from real mise 2026.7.18 (see `fixtures/`), so a
//! change in mise's output shape fails here rather than in the app.
use powermise_mise::{
    ConfigFile, Doctor, MiseVersion, OutdatedReport, RegistryEntry, RemoteVersion, Task,
    ToolInventory,
};

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let contents = std::fs::read_to_string(&path).expect("fixture should exist");
    serde_json::from_str(&contents).unwrap_or_else(|error| panic!("{name}: {error}"))
}

#[test]
fn ls_reports_versions_sources_and_install_state() {
    let global: ToolInventory = fixture("ls-global.json");
    let jq = &global["jq"][0];
    assert_eq!(jq.version, "1.7.1");
    assert!(jq.installed && jq.active);
    assert!(
        jq.source
            .as_ref()
            .unwrap()
            .path
            .ends_with(".config/mise/config.toml")
    );

    let project: ToolInventory = fixture("ls-project.json");
    let versions = &project["jq"];
    assert!(
        versions
            .iter()
            .any(|version| version.source.is_none() && version.installed)
    );
    let requested = versions
        .iter()
        .find(|version| version.requested_version.is_some())
        .unwrap();
    assert_eq!(requested.requested_version.as_deref(), Some("1.8"));
    assert!(!requested.installed);
}

#[test]
fn outdated_reports_missing_and_bumpable_versions() {
    let project: OutdatedReport = fixture("outdated-project.json");
    assert_eq!(project["jq"].current, None);
    assert_eq!(project["jq"].latest, "1.8.2");

    let bump: OutdatedReport = fixture("outdated-bump-global.json");
    assert_eq!(bump["jq"].current.as_deref(), Some("1.7.1"));
    assert_eq!(bump["jq"].bump.as_deref(), Some("1.8.2"));

    let none: OutdatedReport = fixture("outdated-global.json");
    assert!(none.is_empty());
}

#[test]
fn registry_and_remote_versions_parse() {
    let registry: Vec<RegistryEntry> = fixture("registry-sample.json");
    let jq = registry.iter().find(|entry| entry.short == "jq").unwrap();
    assert!(!jq.backends.is_empty());
    assert!(jq.description.is_some());

    let versions: Vec<RemoteVersion> = fixture("ls-remote-jq.json");
    assert!(versions.iter().any(|version| version.version == "1.7.1"));
}

#[test]
fn project_config_tasks_and_doctor_parse() {
    let configs: Vec<ConfigFile> = fixture("config-ls.json");
    assert_eq!(configs[0].tools, ["jq", "shellcheck"]);

    let tasks: Vec<Task> = fixture("tasks-ls.json");
    assert_eq!(tasks[0].name, "build");
    assert_eq!(tasks[0].description, "Build it");
    assert_eq!(tasks[0].run, ["echo build"]);

    let doctor: Doctor = fixture("doctor.json");
    assert!(!doctor.activated);
    assert_eq!(doctor.errors.len(), 2);
    assert!(doctor.self_update_available);
    assert_eq!(doctor.shell.unwrap().name, "/bin/zsh");
}

#[test]
fn mise_versions_compare_against_the_minimum() {
    let installed = MiseVersion::parse("2026.7.18 macos-arm64 (2026-07-30)").unwrap();
    assert_eq!(installed, MiseVersion::MINIMUM);
    assert!(MiseVersion::parse("2026.10.10").unwrap() > MiseVersion::MINIMUM);
    assert!(MiseVersion::parse("2025.12.0").unwrap() < MiseVersion::MINIMUM);
    assert_eq!(MiseVersion::parse("not a version"), None);
}
