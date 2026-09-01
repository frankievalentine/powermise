use std::{
    fs,
    path::{Path, PathBuf},
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
            "powermise-global-config-{}-{timestamp}-{sequence}",
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

fn run(args: &[&str], powermise_dir: &Path, mise_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_powermise"));
    command
        .current_dir(repo_root())
        .args(args)
        .args(["--powermise-dir", &powermise_dir.to_string_lossy()])
        .args(["--mise-config-dir", &mise_dir.to_string_lossy()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[test]
fn global_mise_config_survives_plan_and_apply() {
    let directory = TestDirectory::new();
    let powermise_dir = directory.0.join("powermise");
    let mise_dir = directory.0.join("mise");
    fs::create_dir_all(&mise_dir).expect("mise config dir should exist");

    let user_config = mise_dir.join("config.toml");
    let user_contents = b"[tools]\nuser-tool = 1\n\n[settings]\ntrusted = []\n";
    fs::write(&user_config, user_contents).expect("user config should write");

    let import = run(
        &[
            "selection",
            "import",
            "--selection",
            "selections/engineering-agents.toml",
        ],
        &powermise_dir,
        &mise_dir,
    )
    .output()
    .expect("import should run");
    assert!(import.status.success());

    let plan = run(
        &[
            "plan",
            "--selection",
            "selections/engineering-agents.toml",
            "--target",
            "darwin",
        ],
        &powermise_dir,
        &mise_dir,
    )
    .output()
    .expect("plan should run");
    assert!(plan.status.success(), "plan should succeed");

    let apply = run(
        &[
            "apply",
            "--name",
            "engineering-agents",
            "--target",
            "darwin",
            // Tests never install real tools on the machine: declare-only mode
            // keeps this hermetic while still exercising every file write.
            "--skip-bootstrap",
            "--yes",
        ],
        &powermise_dir,
        &mise_dir,
    )
    .output()
    .expect("apply should run");
    assert!(apply.status.success(), "apply should succeed");

    assert_eq!(
        fs::read(&user_config).expect("user config should exist"),
        user_contents,
        "the user-owned global mise config must survive plan and apply unchanged"
    );
    let generated = mise_dir.join("config.powermise-engineering-agents.toml");
    assert!(
        generated.exists(),
        "apply must create the env-scoped generated config, not modify config.toml"
    );
}

#[test]
fn apply_refuses_to_run_without_yes_on_a_non_interactive_terminal() {
    let directory = TestDirectory::new();
    let powermise_dir = directory.0.join("powermise");
    let mise_dir = directory.0.join("mise");
    fs::create_dir_all(&mise_dir).expect("mise config dir should exist");

    let import = run(
        &[
            "selection",
            "import",
            "--selection",
            "selections/engineering-agents.toml",
        ],
        &powermise_dir,
        &mise_dir,
    )
    .output()
    .expect("import should run");
    assert!(import.status.success());

    let apply = run(
        &["apply", "--name", "engineering-agents", "--target", "darwin"],
        &powermise_dir,
        &mise_dir,
    )
    .output()
    .expect("apply should run");
    assert!(
        !apply.status.success(),
        "a piped apply without --yes must be refused"
    );
    assert!(
        String::from_utf8_lossy(&apply.stderr).contains("--yes"),
        "the refusal must explain how to proceed"
    );
    assert!(
        !mise_dir.join("config.powermise-engineering-agents.toml").exists(),
        "a refused apply must not write the generated configuration"
    );
}
