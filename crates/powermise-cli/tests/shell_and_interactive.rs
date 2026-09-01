use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{self, Command, Stdio},
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
            "powermise-shell-test-{}-{timestamp}-{sequence}",
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn start_marker(selection: &str) -> String {
    format!("# >>> powermise:{selection} >>>")
}

#[test]
fn shell_install_is_idempotent_and_uninstall_is_dry_run_first() {
    let home = TestDirectory::new();
    let repository = repo_root();

    let install = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("HOME", &home.0)
        .args(["shell", "install", "--name", "engineering-agents", "--shell", "zsh"])
        .output()
        .expect("shell install should execute");
    assert!(
        install.status.success(),
        "shell install failed: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let rc_path = home.0.join(".zshrc");
    let rc = fs::read_to_string(&rc_path).expect("rc file should be written");
    assert!(rc.contains(&start_marker("engineering-agents")));
    assert!(rc.contains("export MISE_ENV=powermise-engineering-agents"));

    let second = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("HOME", &home.0)
        .args(["shell", "install", "--name", "engineering-agents", "--shell", "zsh"])
        .output()
        .expect("second shell install should execute");
    assert!(second.status.success());
    let rc = fs::read_to_string(&rc_path).expect("rc file should be readable");
    assert_eq!(
        rc.matches(&start_marker("engineering-agents")).count(),
        1,
        "repeated install must not duplicate the block"
    );

    let dry = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("HOME", &home.0)
        .args(["shell", "uninstall", "--name", "engineering-agents", "--shell", "zsh"])
        .output()
        .expect("shell uninstall should execute");
    assert!(dry.status.success());
    assert!(
        fs::read_to_string(&rc_path)
            .expect("rc file should be readable")
            .contains(&start_marker("engineering-agents")),
        "dry-run uninstall must leave the block in place"
    );

    let real = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("HOME", &home.0)
        .args(["shell", "uninstall", "--name", "engineering-agents", "--shell", "zsh", "--yes"])
        .output()
        .expect("real shell uninstall should execute");
    assert!(real.status.success());
    let rc = fs::read_to_string(&rc_path).expect("rc file should be readable");
    assert!(!rc.contains(&start_marker("engineering-agents")));
    assert!(!rc.contains("MISE_ENV=powermise-engineering-agents"));
}

#[test]
fn shell_install_json_reports_rc_path_and_inserted() {
    let home = TestDirectory::new();
    let repository = repo_root();

    let output = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .env("HOME", &home.0)
        .args(["shell", "install", "--name", "engineering-agents", "--shell", "bash", "--output", "json"])
        .output()
        .expect("shell install should execute");
    assert!(output.status.success());
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(document["command"], "shell-install");
    assert_eq!(document["payload"]["inserted"], true);
    assert!(
        document["payload"]["rc_path"]
            .as_str()
            .map(|path| path.ends_with(".bashrc"))
            .unwrap_or(false),
        "rc_path should point at the bash startup file"
    );
}

#[test]
fn interactive_flow_writes_the_same_selection_model() {
    let directory = TestDirectory::new();
    let repository = repo_root();
    let powermise_dir = directory.0.join("powermise");

    let mut child = Command::new(env!("CARGO_BIN_EXE_powermise"))
        .current_dir(&repository)
        .args([
            "selection",
            "interactive",
            "--name",
            "my-agent",
            "--catalog",
        ])
        .arg(repository.join("catalog/profiles/harnesses.toml"))
        .args(["--tools"])
        .arg(repository.join("catalog/profiles/tools.toml"))
        .args(["--apps"])
        .arg(repository.join("catalog/profiles/apps.toml"))
        .args(["--integrations"])
        .arg(repository.join("catalog/profiles/integrations.toml"))
        .args(["--mcp"])
        .arg(repository.join("catalog/profiles/mcp.toml"))
        .args(["--powermise-dir"])
        .arg(&powermise_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("interactive selection should spawn");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"1,3\n")
        .expect("answers should be written");
    let output = child.wait_with_output().expect("interactive flow should finish");

    assert!(
        output.status.success(),
        "interactive flow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let written = powermise_dir.join("selections/my-agent.toml");
    let contents = fs::read_to_string(&written).expect("managed selection should be written");
    assert!(contents.contains("name = \"my-agent\""));
    assert!(
        contents.contains("harnesses = [\"antigravity\", \"claude\"]"),
        "menu numbers must map to harness ids, got: {contents}"
    );
    assert!(contents.contains("schema_version = 1"));
}