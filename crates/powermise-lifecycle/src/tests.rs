//! End-to-end lifecycle tests over isolated temporary layouts.
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use powermise_domain::{
    MiseBootstrapApplyOutput, MiseBootstrapApplyRequest, MiseBootstrapStatus, MiseBootstrapTool,
    MiseDryRunOutput, MiseDryRunRequest, MiseRunner, Operation, OperationAction,
    OperationOwnership, OperationProvider, OperationReversibility, OperationVerification, Platform,
    ResourceKind, ReversibilityKind, VerificationKind,
};
use powermise_state::{load_ledger, write_ledger};

use crate::{
    ApplyOptions, BootstrapOutcome, CatalogPaths, PowermiseLayout, SelectionStore, activate, apply,
    diff, doctor, exec, has_errors, install_stubs, prune, remove_stubs, status, uninstall,
};

#[derive(Debug)]
struct FakeMiseError;

impl fmt::Display for FakeMiseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fake mise error")
    }
}

impl Error for FakeMiseError {}

#[derive(Default)]
struct FakeRunner {
    pending_tool: Option<String>,
    bootstrap_fails: bool,
    /// Every real-bootstrap invocation as `(environment, config_dir)`.
    bootstrap_calls: Mutex<Vec<(String, PathBuf)>>,
    /// Ordered event log across `bootstrap_apply` and `bootstrap_status`.
    events: Mutex<Vec<String>>,
}

impl FakeRunner {
    fn bootstrap_calls(&self) -> Vec<(String, PathBuf)> {
        self.bootstrap_calls.lock().unwrap().clone()
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl MiseRunner for FakeRunner {
    type Error = FakeMiseError;

    fn bootstrap_dry_run(
        &self,
        _request: &MiseDryRunRequest,
    ) -> Result<MiseDryRunOutput, Self::Error> {
        Ok(MiseDryRunOutput {
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn bootstrap_status(
        &self,
        _request: &MiseDryRunRequest,
    ) -> Result<MiseBootstrapStatus, Self::Error> {
        self.events.lock().unwrap().push("status".into());
        let tools = match &self.pending_tool {
            Some(tool) => vec![MiseBootstrapTool {
                tool: tool.clone(),
                requested_version: "latest".into(),
                resolved_version: String::new(),
                state: "missing".into(),
                installed: false,
            }],
            None => Vec::new(),
        };
        Ok(MiseBootstrapStatus {
            packages: Vec::new(),
            tools,
        })
    }

    fn bootstrap_apply(
        &self,
        request: &MiseBootstrapApplyRequest,
    ) -> Result<MiseBootstrapApplyOutput, Self::Error> {
        self.events
            .lock()
            .unwrap()
            .push(format!("bootstrap:{}", request.environment));
        self.bootstrap_calls
            .lock()
            .unwrap()
            .push((request.environment.clone(), request.config_dir.clone()));
        if self.bootstrap_fails {
            Err(FakeMiseError)
        } else {
            Ok(MiseBootstrapApplyOutput {
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    fn exec(
        &self,
        _environment: &str,
        _command: &[String],
    ) -> Result<std::process::ExitStatus, Self::Error> {
        Ok(std::process::ExitStatus::default())
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn catalog_paths(root: &Path) -> CatalogPaths {
    CatalogPaths {
        harnesses: root.join("catalog/profiles/harnesses.toml"),
        tools: root.join("catalog/profiles/tools.toml"),
        apps: root.join("catalog/profiles/apps.toml"),
        integrations: root.join("catalog/profiles/integrations.toml"),
        mcp: root.join("catalog/profiles/mcp.toml"),
    }
}

fn temp_layout(base: &Path, name: &str) -> PowermiseLayout {
    PowermiseLayout::resolve(Some(base.join(name))).expect("layout should resolve")
}

fn read_generated_config(mise_config_dir: &Path, selection: &str) -> String {
    let path = mise_config_dir.join(format!("config.powermise-{selection}.toml"));
    String::from_utf8(fs::read(&path).expect("generated config should exist")).unwrap()
}

#[test]
fn cross_machine_transfer_recreates_desired_state() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout_a = temp_layout(&base, "machine-a");
    let layout_b = temp_layout(&base, "machine-b");
    let mise_a = base.join("mise-a");
    let mise_b = base.join("mise-b");

    SelectionStore::new(&layout_a)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed on machine A");

    activate(
        &layout_a,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_a,
    )
    .expect("activate should succeed on machine A");

    let exported = SelectionStore::new(&layout_a)
        .export("engineering-agents")
        .expect("export should succeed");
    assert!(exported.contains("name = \"engineering-agents\""));

    let transfer_path = base.join("transfer.toml");
    fs::write(&transfer_path, exported).expect("transfer file should write");

    SelectionStore::new(&layout_b)
        .import(&transfer_path, None)
        .expect("import should succeed on machine B");

    activate(
        &layout_b,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_b,
    )
    .expect("activate should succeed on machine B");

    let generated_a = read_generated_config(&mise_a, "engineering-agents");
    let generated_b = read_generated_config(&mise_b, "engineering-agents");
    assert_eq!(
        generated_a, generated_b,
        "transferred selection must reproduce identical bytes"
    );

    let manifest_a = fs::read(layout_a.manifest_path("engineering-agents")).unwrap();
    let manifest_b = fs::read(layout_b.manifest_path("engineering-agents")).unwrap();
    assert_eq!(
        manifest_a, manifest_b,
        "manifests must be identical after transfer"
    );
}

#[test]
fn activate_diff_prune_and_uninstall_are_ledger_scoped() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "main");
    let mise_config_dir = base.join("mise");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");
    activate(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("activate should succeed");

    let initial = diff(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("diff should run");
    assert_eq!(
        initial.change.kind,
        powermise_state::GeneratedFileChangeKind::Unchanged,
        "a freshly activated selection should be unchanged"
    );
    assert!(
        initial.drift.is_none(),
        "no drift expected after activation"
    );

    let config_path = mise_config_dir.join("config.powermise-engineering-agents.toml");
    let before_prune = fs::read(&config_path).unwrap();

    let prune_dry = prune(&layout, &mise_config_dir, true).expect("dry-run prune should run");
    assert!(
        prune_dry.actions.is_empty(),
        "managed selection must not be pruned"
    );
    assert!(
        !prune_dry.ledger_updated,
        "dry-run must not touch the ledger"
    );
    assert_eq!(
        fs::read(&config_path).unwrap(),
        before_prune,
        "dry-run must not modify the generated configuration"
    );

    let uninstall_dry = uninstall(&layout, "engineering-agents", &mise_config_dir, true)
        .expect("dry-run uninstall");
    assert!(
        uninstall_dry.dry_run,
        "uninstall should stay in dry-run mode"
    );
    assert!(
        !uninstall_dry.actions.is_empty(),
        "dry-run uninstall should report the actions it would take"
    );
    assert!(
        config_path.exists(),
        "dry-run uninstall must not remove the configuration"
    );

    let uninstall_real =
        uninstall(&layout, "engineering-agents", &mise_config_dir, false).expect("real uninstall");
    assert!(!uninstall_real.actions.is_empty());
    assert!(
        !config_path.exists(),
        "uninstall must remove the generated configuration"
    );
    assert!(
        !layout.manifest_path("engineering-agents").exists(),
        "uninstall must remove the manifest"
    );

    let ledger = load_ledger(&layout.ledger_path).expect("ledger should load");
    assert!(
        ledger
            .resources
            .values()
            .all(|record| record.selection != "engineering-agents"),
        "uninstall must remove the ownership entry"
    );

    let after = diff(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("diff should run after uninstall");
    assert_eq!(
        after.change.kind,
        powermise_state::GeneratedFileChangeKind::Create,
        "after uninstall the configuration should be desired again"
    );
}

#[test]
fn apply_converges_with_a_real_mise_bootstrap() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "apply-converged");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner::default();
    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: true,
        progress: None,
    };
    let report = apply(&runner, &options, "engineering-agents").expect("apply should succeed");

    assert_eq!(
        report.bootstrap,
        BootstrapOutcome::Converged,
        "a successful real bootstrap must be reported as converged"
    );
    let calls = runner.bootstrap_calls();
    assert_eq!(
        calls.len(),
        1,
        "apply must run exactly one real mise bootstrap"
    );
    assert_eq!(
        calls[0],
        (
            "powermise-engineering-agents".into(),
            mise_config_dir.clone()
        ),
        "the real bootstrap must target the selection environment in the configured mise directory"
    );
    assert!(
        report
            .results
            .iter()
            .filter(|result| result.message.as_deref() == Some("converged by real mise bootstrap"))
            .count()
            > 0,
        "mise-owned operations must report real convergence"
    );
    assert!(
        report.mise_follow_ups.is_empty(),
        "a converged selection must surface no mise follow-ups"
    );
    assert!(
        report.run_report_path.exists(),
        "run report must be persisted"
    );

    let ledger = load_ledger(&layout.ledger_path).expect("ledger should load");
    let owned = ledger
        .resources
        .values()
        .filter(|record| {
            record.selection == "engineering-agents"
                && record.provider == OperationProvider::Generated
                && record.resource_kind == ResourceKind::GeneratedFile
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owned.len(),
        1,
        "apply must record generated-config ownership"
    );

    let findings = doctor(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Path::new("mise"),
    );
    assert!(
        !has_errors(&findings),
        "doctor should report no errors after apply"
    );
}

#[test]
fn apply_runs_the_real_bootstrap_before_verifying_operations() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "apply-ordering");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner::default();
    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: true,
        progress: None,
    };
    apply(&runner, &options, "engineering-agents").expect("apply should succeed");

    let events = runner.events();
    let bootstrap_index = events
        .iter()
        .position(|event| event == "bootstrap:powermise-engineering-agents")
        .expect("the real bootstrap should have been invoked");
    assert!(
        events[bootstrap_index + 1..]
            .iter()
            .all(|event| event == "status"),
        "every post-bootstrap verification must happen after the real bootstrap"
    );
}

#[test]
fn apply_without_bootstrap_stays_declare_only_and_surfaces_follow_ups() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "apply-declare-only");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner {
        pending_tool: Some("node".into()),
        ..FakeRunner::default()
    };
    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: false,
        progress: None,
    };
    let report = apply(&runner, &options, "engineering-agents").expect("apply should succeed");

    assert!(
        runner.bootstrap_calls().is_empty(),
        "--skip-bootstrap must not invoke a real mise bootstrap"
    );
    assert_eq!(
        report.bootstrap,
        BootstrapOutcome::Skipped {
            reason: "declared only (--skip-bootstrap)".into()
        },
        "declare-only mode must be reported as skipped"
    );
    assert!(
        report
            .mise_follow_ups
            .iter()
            .any(|action| action.starts_with("node")),
        "declare-only mode must surface pending tool installs as follow-ups"
    );
    assert!(
        report
            .results
            .iter()
            .all(|result| result.outcome != powermise_domain::OperationOutcome::Failed),
        "declare-only mode must not fail operations"
    );

    let claude_block = adapters_config_dir.join("claude/settings.json");
    if claude_block.exists() {
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(&claude_block).unwrap()).unwrap();
        let managed = document
            .as_object()
            .expect("settings.json should be an object")
            .get("_powermise")
            .expect("managed blocks should live under _powermise");
        assert!(
            managed
                .as_object()
                .map(|blocks| !blocks.is_empty())
                .unwrap_or(false),
            "claude config-adapter operations should write managed blocks even without a real bootstrap"
        );
    }
}

#[test]
fn apply_reports_failure_when_the_real_mise_bootstrap_fails() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "apply-bootstrap-failure");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner {
        bootstrap_fails: true,
        ..FakeRunner::default()
    };
    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: true,
        progress: None,
    };
    let report = apply(&runner, &options, "engineering-agents").expect("apply still completes");

    let failure = match &report.bootstrap {
        BootstrapOutcome::Failed { message } => message.clone(),
        outcome => panic!("expected a failed bootstrap outcome, got {outcome:?}"),
    };
    assert!(
        failure.contains("fake mise error"),
        "the bootstrap failure must carry the underlying cause"
    );
    assert_eq!(
        runner.bootstrap_calls().len(),
        1,
        "exactly one real bootstrap attempt"
    );
    assert!(
        report.results.iter().any(|result| {
            result
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("real mise bootstrap did not converge"))
                && result.outcome == powermise_domain::OperationOutcome::Failed
        }),
        "mise-owned operations must be marked failed when the bootstrap fails"
    );
    assert!(
        report
            .recovery_guidance
            .iter()
            .any(|guidance| guidance.contains("mise bootstrap --yes")),
        "failure-boundary guidance must explain how to recover from the failed bootstrap"
    );
    assert!(
        report
            .results
            .iter()
            .any(|result| result.outcome == powermise_domain::OperationOutcome::Succeeded),
        "independent config-adapter and native operations must still be attempted"
    );
    assert!(
        report.run_report_path.exists(),
        "the run report must persist the partial failure"
    );
}

#[test]
fn apply_invokes_the_progress_reporter_for_each_phase() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "progress");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let progress_messages = std::sync::Mutex::new(Vec::new());
    let progress = |message: &str| {
        progress_messages.lock().unwrap().push(message.to_owned());
    };

    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: true,
        progress: Some(&progress),
    };
    apply(&FakeRunner::default(), &options, "engineering-agents").expect("apply should succeed");

    let messages = progress_messages.into_inner().unwrap();
    assert!(
        messages
            .iter()
            .any(|message| message == "writing generated configuration"),
        "progress should report the configuration write phase"
    );
    assert!(
        messages
            .iter()
            .any(|message| message == "recording ownership ledger"),
        "progress should report the ledger phase"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("converging mise environment")),
        "progress should report the real bootstrap convergence phase"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("applying ")),
        "progress should report each operation"
    );
    assert!(
        messages
            .iter()
            .any(|message| message == "persisting run report"),
        "progress should report the run-report phase"
    );
}

#[test]
fn user_owned_global_mise_config_survives_all_commands() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "global-config");
    let mise_config_dir = base.join("mise");
    fs::create_dir_all(&mise_config_dir).expect("mise config dir should be created");

    // A user-owned global mise config that Powermise must never touch.
    let user_config = mise_config_dir.join("config.toml");
    let user_contents = b"[tools]\nuser-tool = \"1\"\n\n[settings]\ntrusted-configs = []\n";
    fs::write(&user_config, user_contents).expect("user config should write");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner::default();
    let target = Platform::Darwin;

    activate(
        &layout,
        &paths,
        "engineering-agents",
        target,
        None,
        &mise_config_dir,
    )
    .expect("activate should succeed");
    let _ = diff(
        &layout,
        &paths,
        "engineering-agents",
        target,
        None,
        &mise_config_dir,
    )
    .expect("diff should run");
    let _ = doctor(
        &layout,
        &paths,
        "engineering-agents",
        target,
        None,
        &mise_config_dir,
        Path::new("mise"),
    );
    let _ = status(
        &runner,
        &layout,
        &paths,
        "engineering-agents",
        target,
        None,
        &mise_config_dir,
    )
    .expect("status should run");

    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &base.join("adapters"),
        run_mise_bootstrap: true,
        progress: None,
    };
    apply(&runner, &options, "engineering-agents").expect("apply should succeed");

    prune(&layout, &mise_config_dir, true).expect("dry-run prune should run");
    uninstall(&layout, "engineering-agents", &mise_config_dir, true).expect("uninstall should run");

    assert_eq!(
        fs::read(&user_config).expect("user config should still exist"),
        user_contents,
        "the user-owned global mise config must survive every command byte-for-byte"
    );
}

#[test]
fn status_reports_expected_state_after_apply_and_surfaces_follow_ups() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "status");
    let mise_config_dir = base.join("mise");
    let adapters_config_dir = base.join("adapters");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let runner = FakeRunner {
        pending_tool: Some("node".into()),
        ..FakeRunner::default()
    };
    let options = ApplyOptions {
        layout: &layout,
        paths: &paths,
        target: Platform::Darwin,
        machine_role: None,
        mise_config_dir: &mise_config_dir,
        adapters_config_dir: &adapters_config_dir,
        run_mise_bootstrap: false,
        progress: None,
    };
    apply(&runner, &options, "engineering-agents").expect("apply should succeed");

    let report = status(
        &runner,
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("status should run");
    assert_eq!(report.environment, "powermise-engineering-agents");
    assert!(
        report.ledger_owned,
        "status must observe the ownership entry"
    );
    assert!(
        report.run_report_exists,
        "status must observe the run report"
    );
    assert_eq!(
        report.change.kind,
        powermise_state::GeneratedFileChangeKind::Unchanged,
        "a freshly applied selection should be unchanged"
    );
    assert!(report.drift.is_none(), "no drift expected after apply");
    assert!(
        report.native.is_empty(),
        "darwin selection has no native operations"
    );
    assert!(
        !report.satisfied,
        "pending mise follow-ups mean the selection is not yet satisfied"
    );

    let satisfied = status(
        &FakeRunner::default(),
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("status should run");
    assert!(
        satisfied.satisfied,
        "a fully converged selection with no follow-ups should be satisfied"
    );
}

#[test]
fn exec_runs_a_command_in_the_selection_environment() {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let base = tempfile_dir();
    let layout = temp_layout(&base, "exec");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");

    let report = exec(
        &FakeRunner::default(),
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &["mise".into(), "ls".into(), "--current".into()],
    )
    .expect("exec should run");
    assert_eq!(report.environment, "powermise-engineering-agents");
    assert!(report.status.success(), "fake runner should report success");
}

fn tempfile_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let unique = format!(
        "{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("powermise-test-{unique}"));
    fs::create_dir_all(&dir).expect("temp directory should be created");
    dir
}

#[allow(dead_code)]
fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

fn stubbed_selection(base: &Path) -> (PowermiseLayout, CatalogPaths, PathBuf, PathBuf, PathBuf) {
    let root = repo_root();
    let paths = catalog_paths(&root);
    let layout = temp_layout(base, "stubs");
    let mise_config_dir = base.join("mise");
    let bin_dir = base.join("bin");

    SelectionStore::new(&layout)
        .import(&root.join("selections/engineering-agents.toml"), None)
        .expect("import should succeed");
    activate(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("activate should succeed");
    (layout, paths, mise_config_dir, bin_dir, root)
}

fn stub_test_operation(selection: &str, resource_id: &str) -> Operation {
    Operation {
        id: format!("stub:{selection}:fabricated"),
        provider: OperationProvider::Generated,
        resource_kind: ResourceKind::StubScript,
        resource_id: resource_id.into(),
        action: OperationAction::Write,
        requested_state: "fingerprint".into(),
        depends_on: Vec::new(),
        ownership: OperationOwnership {
            owner: "powermise".into(),
            selection: selection.into(),
        },
        verification: OperationVerification {
            kind: VerificationKind::FileFingerprint,
            expected: "fingerprint".into(),
        },
        reversibility: OperationReversibility {
            kind: ReversibilityKind::DeclarationOnly,
            note: "test".into(),
        },
    }
}

#[test]
fn stubs_install_idempotently_and_remove_is_ledger_scoped() {
    let base = tempfile_dir();
    let (layout, paths, mise_config_dir, bin_dir, _root) = stubbed_selection(&base);

    // The engineering-agents selection resolves exactly these mise-managed
    // tools on darwin: claude-code, fd, jq, node, and ripgrep.
    let expected_commands = ["claude", "fd", "jq", "node", "rg"];

    let report = install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect("stub install should succeed");
    let created_names = report
        .created
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(created_names, expected_commands);
    assert_eq!(report.bin_dir, bin_dir);

    let claude_stub = fs::read_to_string(bin_dir.join("claude")).unwrap();
    assert!(
        claude_stub.contains(
            "exec mise x --env powermise-engineering-agents claude-code -- claude \"$@\""
        ),
        "stub must lazily run the tool through the selection environment, got: {claude_stub}"
    );
    let rg_stub = fs::read_to_string(bin_dir.join("rg")).unwrap();
    assert!(rg_stub.contains("mise x --env powermise-engineering-agents ripgrep -- rg"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(bin_dir.join("claude"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "stubs must be executable");
    }

    let ledger = load_ledger(&layout.ledger_path).unwrap();
    let stub_records = ledger
        .resources
        .values()
        .filter(|record| record.resource_kind == ResourceKind::StubScript)
        .count();
    assert_eq!(
        stub_records,
        expected_commands.len(),
        "every stub must be ledger-owned"
    );

    let second = install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect("second install should succeed");
    assert!(second.created.is_empty(), "reinstall must create nothing");
    assert_eq!(second.unchanged.len(), expected_commands.len());
    assert!(!second.ledger_updated);

    let dry = remove_stubs(&layout, "engineering-agents", true).expect("dry-run remove");
    assert!(dry.dry_run);
    assert_eq!(dry.removed.len(), expected_commands.len());
    assert!(bin_dir.join("rg").exists(), "dry-run must not remove stubs");

    let real = remove_stubs(&layout, "engineering-agents", false).expect("real remove");
    assert!(!real.dry_run);
    assert_eq!(real.removed.len(), expected_commands.len());
    assert!(
        !bin_dir.join("rg").exists(),
        "real remove must delete stubs"
    );
    assert!(
        mise_config_dir
            .join("config.powermise-engineering-agents.toml")
            .exists(),
        "remove_stubs must never touch the generated configuration"
    );
    let ledger = load_ledger(&layout.ledger_path).unwrap();
    assert!(
        ledger
            .resources
            .values()
            .all(|record| record.resource_kind != ResourceKind::StubScript),
        "stub ledger entries must be removed"
    );
}

#[test]
fn stub_install_refuses_to_overwrite_unowned_files() {
    let base = tempfile_dir();
    let (layout, paths, mise_config_dir, bin_dir, _root) = stubbed_selection(&base);

    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join("jq"), "#!/bin/sh\necho mine\n").unwrap();

    let error = install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect_err("an unowned stub target must be refused");
    assert!(error.to_string().contains("does not own"));
    assert_eq!(
        fs::read_to_string(bin_dir.join("jq")).unwrap(),
        "#!/bin/sh\necho mine\n",
        "the user file must be preserved"
    );
}

#[test]
fn stub_install_refuses_files_owned_by_another_selection() {
    let base = tempfile_dir();
    let (layout, paths, mise_config_dir, bin_dir, _root) = stubbed_selection(&base);

    let rg_path = bin_dir.join("rg");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(&rg_path, "#!/bin/sh\necho other\n").unwrap();
    let mut ledger = load_ledger(&layout.ledger_path).unwrap();
    ledger
        .record_verified(
            &stub_test_operation("other-selection", &rg_path.to_string_lossy()),
            None,
        )
        .expect("fabricated ownership should record");
    write_ledger(&layout.ledger_path, &ledger).expect("ledger should write");

    let error = install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect_err("a stub owned by another selection must be refused");
    assert!(
        error
            .to_string()
            .contains("already owned by selection `other-selection`")
    );
}

#[test]
fn uninstall_and_prune_remove_ledger_owned_stubs() {
    let base = tempfile_dir();
    let (layout, paths, mise_config_dir, bin_dir, _root) = stubbed_selection(&base);

    install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect("stub install should succeed");

    let dry_uninstall = uninstall(&layout, "engineering-agents", &mise_config_dir, true)
        .expect("dry-run uninstall");
    assert!(
        dry_uninstall
            .actions
            .iter()
            .any(|action| action.path.parent() == Some(bin_dir.as_path())),
        "dry-run uninstall must report the stubs it would remove"
    );
    assert!(
        bin_dir.join("claude").exists(),
        "dry-run must not remove stubs"
    );

    uninstall(&layout, "engineering-agents", &mise_config_dir, false).expect("real uninstall");
    assert!(
        !bin_dir.join("claude").exists(),
        "uninstall must remove stubs"
    );
    assert!(!bin_dir.join("rg").exists(), "uninstall must remove stubs");
    let ledger = load_ledger(&layout.ledger_path).unwrap();
    assert!(
        ledger
            .resources
            .values()
            .all(|record| record.selection != "engineering-agents"),
        "no ownership entries may survive uninstall"
    );

    // Prune covers stubs of selections that are no longer managed. Re-activate
    // first: uninstall removed the generated configuration the stubs require.
    activate(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
    )
    .expect("re-activate should succeed");
    install_stubs(
        &layout,
        &paths,
        "engineering-agents",
        Platform::Darwin,
        None,
        &mise_config_dir,
        Some(&bin_dir),
    )
    .expect("stub reinstall should succeed");
    SelectionStore::new(&layout)
        .remove("engineering-agents")
        .expect("selection removal should succeed");

    let dry_prune = prune(&layout, &mise_config_dir, true).expect("dry-run prune");
    assert!(
        dry_prune
            .actions
            .iter()
            .any(|action| matches!(action, crate::PruneAction::Remove { path, .. } if path.parent() == Some(bin_dir.as_path()))),
        "dry-run prune must report the stubs it would remove"
    );
    assert!(
        bin_dir.join("claude").exists(),
        "dry-run prune must not remove stubs"
    );

    prune(&layout, &mise_config_dir, false).expect("real prune");
    assert!(
        !bin_dir.join("claude").exists(),
        "prune must remove stale stubs"
    );
    assert!(
        !bin_dir.join("rg").exists(),
        "prune must remove stale stubs"
    );
}
