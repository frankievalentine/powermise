use std::{
    fs,
    path::{Path, PathBuf},
    process::{self, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "powermise-plan-cli-test-{}-{timestamp}-{sequence}",
            process::id(),
        ));
        fs::create_dir_all(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn create_fake_mise(directory: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join("mise");
    fs::write(
        &path,
        "#!/bin/sh\n\
if [ \"$MISE_CONFIG_DIR\" = \"$POWERMISE_TEST_CONFIG_DIR\" ]; then\n\
  echo 'user config directory leaked into dry-run' >&2\n\
  exit 9\n\
fi\n\
printf 'fake mise %s\\n' \"$*\"\n",
    )
    .expect("fake mise should be written");
    let mut permissions = fs::metadata(&path)
        .expect("fake mise metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fake mise should be executable");
    path
}

#[cfg(windows)]
fn create_fake_mise(directory: &Path) -> PathBuf {
    let path = directory.join("mise.cmd");
    fs::write(
        &path,
        "@echo off\r\n\
if \"%MISE_CONFIG_DIR%\"==\"%POWERMISE_TEST_CONFIG_DIR%\" (\r\n\
  echo user config directory leaked into dry-run 1>&2\r\n\
  exit /b 9\r\n\
)\r\n\
echo fake mise %*\r\n",
    )
    .expect("fake mise should be written");
    path
}

#[test]
fn plan_reports_changes_and_isolates_mise_without_mutating_user_state() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = TestDirectory::new();
    let config_directory = directory.0.join("mise-config");
    fs::create_dir_all(&config_directory).expect("config directory should be created");
    let baseline_path = config_directory.join("config.toml");
    let generated_path = config_directory.join("config.powermise-engineering-agents.toml");
    fs::write(&baseline_path, b"[tools]\nuser-tool = \"1\"\n").expect("baseline should be written");
    fs::write(&generated_path, b"obsolete generated config\n")
        .expect("generated fixture should be written");
    let fake_mise = create_fake_mise(&directory.0);
    let mut entries_before = fs::read_dir(&config_directory)
        .expect("config directory should be readable")
        .map(|entry| entry.expect("entry should be readable").file_name())
        .collect::<Vec<_>>();
    entries_before.sort();

    let output = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("POWERMISE_TEST_CONFIG_DIR", &config_directory)
        .args([
            "plan",
            "--selection",
            "selections/engineering-agents.toml",
            "--target",
            "darwin",
            "--mise-config-dir",
        ])
        .arg(&config_directory)
        .arg("--mise")
        .arg(&fake_mise)
        .output()
        .expect("powermise plan should execute");

    assert!(
        output.status.success(),
        "plan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("plan output should be UTF-8");
    assert!(stdout.contains("selection: engineering-agents"));
    assert!(stdout.contains("target: Darwin"));
    assert!(stdout.contains("generated config: update"));
    assert!(stdout.contains("-obsolete generated config"));
    assert!(stdout.contains("operations:"));
    assert!(stdout.contains("mise-tool:node"));
    assert!(stdout.contains("fake mise --env powermise-engineering-agents bootstrap --dry-run"));

    assert_eq!(
        fs::read(&baseline_path).expect("baseline should remain"),
        b"[tools]\nuser-tool = \"1\"\n"
    );
    assert_eq!(
        fs::read(&generated_path).expect("generated fixture should remain"),
        b"obsolete generated config\n"
    );
    let mut entries_after = fs::read_dir(&config_directory)
        .expect("config directory should be readable")
        .map(|entry| entry.expect("entry should be readable").file_name())
        .collect::<Vec<_>>();
    entries_after.sort();
    assert_eq!(entries_after, entries_before);
}

/// Plans a fixture selection for `target` with a fake mise so the run is
/// hermetic, then asserts the plan compiled and mutated nothing on disk.
fn assert_fixture_plan(target: &str, selection: &str, expected_marker: &str) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = TestDirectory::new();
    let config_directory = directory.0.join("mise-config");
    fs::create_dir_all(&config_directory).expect("config directory should be created");
    let baseline_path = config_directory.join("config.toml");
    fs::write(&baseline_path, b"[tools]\nuser-tool = \"1\"\n").expect("baseline should be written");
    let fake_mise = create_fake_mise(&directory.0);
    let entries_before = fs::read_dir(&config_directory)
        .expect("config directory should be readable")
        .map(|entry| entry.expect("entry should be readable").file_name())
        .collect::<Vec<_>>();

    let output = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("POWERMISE_TEST_CONFIG_DIR", &config_directory)
        .args(["plan", "--selection", selection, "--target", target])
        .args(["--mise-config-dir"])
        .arg(&config_directory)
        .args(["--mise"])
        .arg(&fake_mise)
        .output()
        .expect("powermise plan should execute");

    assert!(
        output.status.success(),
        "{target} plan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("plan output should be UTF-8");
    assert!(
        stdout.contains("operations:"),
        "{target} plan should render operations"
    );
    assert!(
        stdout.contains(expected_marker),
        "{target} plan should emit `{expected_marker}` operations, got:\n{stdout}"
    );
    assert_eq!(
        fs::read(&baseline_path).expect("baseline should remain"),
        b"[tools]\nuser-tool = \"1\"\n"
    );
    let mut entries_after = fs::read_dir(&config_directory)
        .expect("config directory should be readable")
        .map(|entry| entry.expect("entry should be readable").file_name())
        .collect::<Vec<_>>();
    entries_after.sort();
    assert_eq!(
        entries_after, entries_before,
        "{target} plan must not mutate the config dir"
    );
}

#[test]
fn plan_win32_fixture_compiles_with_native_providers() {
    assert_fixture_plan("win32", "selections/windows-fixture.toml", "native-winget:");
}

/// Asserts that a Windows-only fixture is explicitly rejected for the distinct
/// WSL target rather than silently treated as native Windows, proving the WSL
/// target enforces its own platform policy.
fn assert_fixture_plan_rejects(target: &str, selection: &str) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = TestDirectory::new();
    let config_directory = directory.0.join("mise-config");
    fs::create_dir_all(&config_directory).expect("config directory should be created");
    let fake_mise = create_fake_mise(&directory.0);

    let output = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("POWERMISE_TEST_CONFIG_DIR", &config_directory)
        .args(["plan", "--selection", selection, "--target", target])
        .args(["--mise-config-dir"])
        .arg(&config_directory)
        .args(["--mise"])
        .arg(&fake_mise)
        .output()
        .expect("powermise plan should execute");

    assert!(
        !output.status.success(),
        "{target} plan should reject a Windows-only fixture"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("Wsl"),
        "{target} rejection should name the Wsl target, got:\n{stderr}"
    );
}

#[test]
fn plan_rejects_windows_fixture_for_the_distinct_wsl_target() {
    assert_fixture_plan_rejects("wsl", "selections/windows-fixture.toml");
}
