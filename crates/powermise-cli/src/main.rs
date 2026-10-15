use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use powermise_domain::{
    MiseDryRunOutput, MiseDryRunRequest, MiseRunner, OperationOutcome, OperationProvider, Plan,
    Platform,
};
use powermise_lifecycle::{
    ActivationOutput, ApplyOptions, BootstrapOutcome, CatalogPaths, DiffOutput, ExecReport,
    Finding, PowermiseLayout, PruneReport, SelectionStore, StatusReport, UninstallReport, activate,
    apply, compile_context, diff, doctor, exec, has_errors, prune, status, uninstall,
};
use powermise_mise::MiseExecutable;
use powermise_providers::native_plan_command;
use powermise_registry::{ResolvedSelection, load_app_catalog, load_catalog};
use powermise_state::{GeneratedFileChange, GeneratedFileChangeKind, inspect_generated_file};

mod interactive;
mod presentation;

use presentation::{ColorMode, OutputFormat, Presentation, render_table};

#[derive(Parser)]
#[command(
    name = "powermise",
    about = "Curated AI-harness configuration for mise"
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    color: ColorMode,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    output: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    Selection {
        #[command(subcommand)]
        command: SelectionCommand,
    },
    Shell {
        #[command(subcommand)]
        command: ShellCommand,
    },
    Stub {
        #[command(subcommand)]
        command: StubCommand,
    },
    Plan {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        selection: PathBuf,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Doctor {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Status {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Exec {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
        #[arg(
            last = true,
            required = true,
            help = "the command to run inside the selection environment"
        )]
        command: Vec<String>,
    },
    Prune {
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long, help = "remove stale artifacts instead of dry-running")]
        yes: bool,
    },
    Uninstall {
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        name: String,
        #[arg(long, help = "remove artifacts instead of dry-running")]
        yes: bool,
    },
    Apply {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
        #[arg(
            long,
            help = "declare mise-managed resources without running a real `mise bootstrap --yes`"
        )]
        skip_bootstrap: bool,
        #[arg(long, help = "apply without an interactive confirmation prompt")]
        yes: bool,
    },
}

#[derive(Args)]
struct CatalogArgs {
    #[arg(
        long,
        help = "directory holding the catalog TOML files (default: ./catalog/profiles in a checkout, else the catalog built into powermise)"
    )]
    catalog_dir: Option<PathBuf>,
    #[arg(long)]
    catalog: Option<PathBuf>,
    #[arg(long)]
    tools: Option<PathBuf>,
    #[arg(long)]
    apps: Option<PathBuf>,
    #[arg(long)]
    integrations: Option<PathBuf>,
    #[arg(long)]
    mcp: Option<PathBuf>,
}

impl CatalogArgs {
    /// Resolves the catalog directory, then applies any per-file overrides.
    fn into_paths(self) -> Result<CatalogPaths, Box<dyn Error>> {
        let defaults = CatalogPaths::resolve(self.catalog_dir)?;
        Ok(CatalogPaths {
            harnesses: self.catalog.unwrap_or(defaults.harnesses),
            tools: self.tools.unwrap_or(defaults.tools),
            apps: self.apps.unwrap_or(defaults.apps),
            integrations: self.integrations.unwrap_or(defaults.integrations),
            mcp: self.mcp.unwrap_or(defaults.mcp),
        })
    }
}

#[derive(Args)]
struct EnvironmentArgs {
    #[arg(
        long,
        help = "the Powermise root directory (default: ~/.config/powermise)"
    )]
    powermise_dir: Option<PathBuf>,
    #[arg(long, help = "the mise configuration directory")]
    mise_config_dir: Option<PathBuf>,
    #[arg(
        long,
        help = "the adapters configuration directory for managed config blocks"
    )]
    adapters_config_dir: Option<PathBuf>,
    #[arg(long, default_value = "mise")]
    mise: PathBuf,
}

#[derive(Subcommand)]
enum RegistryCommand {
    Validate {
        #[command(flatten)]
        catalog: CatalogArgs,
    },
}

#[derive(Subcommand)]
enum SelectionCommand {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        selection: PathBuf,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    Import {
        #[arg(long)]
        selection: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    List {
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    Show {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Export {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    Validate {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[arg(long)]
        selection: PathBuf,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Compile {
        #[command(flatten)]
        catalog: CatalogArgs,
        #[arg(long)]
        selection: PathBuf,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Activate {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Diff {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
    },
    Interactive {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
        #[arg(long)]
        machine_role: Option<String>,
    },
}

#[derive(Subcommand)]
enum ShellCommand {
    /// Installs a marker-guarded block in the shell startup file that selects
    /// the selection's mise environment. Idempotent.
    Install {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum, default_value_t = ShellArg::Auto, help = "the shell to integrate with (default: detect from $SHELL)")]
        shell: ShellArg,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    /// Removes the selection's managed shell block. Dry-run by default;
    /// `--yes` performs the removal.
    Uninstall {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum, default_value_t = ShellArg::Auto)]
        shell: ShellArg,
        #[arg(long, help = "remove the managed block instead of dry-running")]
        yes: bool,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ShellArg {
    Auto,
    Zsh,
    Bash,
    Powershell,
}

impl From<ShellArg> for powermise_lifecycle::Shell {
    fn from(shell: ShellArg) -> Self {
        match shell {
            ShellArg::Auto => powermise_lifecycle::Shell::detect(),
            ShellArg::Zsh => powermise_lifecycle::Shell::Zsh,
            ShellArg::Bash => powermise_lifecycle::Shell::Bash,
            ShellArg::Powershell => powermise_lifecycle::Shell::Powershell,
        }
    }
}

#[derive(Subcommand)]
enum StubCommand {
    /// Installs lazy-install stubs (`<command>` scripts that run the tool
    /// through the selection's mise environment, installing it on first use)
    /// for every mise-managed tool in the selection. Idempotent.
    Install {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long)]
        machine_role: Option<String>,
        #[arg(long, help = "the directory for stub scripts (default: ~/.local/bin)")]
        bin_dir: Option<PathBuf>,
        #[command(flatten)]
        catalog: CatalogArgs,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
    /// Removes the selection's ledger-owned stubs. Dry-run by default;
    /// `--yes` performs the removal.
    Remove {
        #[arg(long)]
        name: String,
        #[arg(long, help = "remove the stubs instead of dry-running")]
        yes: bool,
        #[command(flatten)]
        environment: EnvironmentArgs,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    Darwin,
    Linux,
    Win32,
    Wsl,
}

impl From<Target> for Platform {
    fn from(target: Target) -> Self {
        match target {
            Target::Darwin => Self::Darwin,
            Target::Linux => Self::Linux,
            Target::Win32 => Self::Win32,
            Target::Wsl => Self::Wsl,
        }
    }
}

struct PlanOutput {
    resolved: ResolvedSelection,
    change: GeneratedFileChange,
    plan: Plan,
    mise: MiseDryRunOutput,
}

enum Output {
    Valid,
    Resolved(ResolvedSelection),
    Compiled(String),
    Plan(PlanOutput),
    Created(PathBuf),
    Listed(Vec<String>),
    Exported(String),
    Imported(PathBuf),
    Activated(ActivationOutput),
    Diffed(DiffOutput),
    Doctored(Vec<Finding>),
    Statused(StatusReport),
    Executed(ExecReport),
    Pruned(PruneReport),
    Uninstalled(UninstallReport),
    Applied(powermise_lifecycle::ApplyReport),
    ShellInstalled(powermise_lifecycle::ShellInstallReport),
    ShellUninstalled(powermise_lifecycle::ShellUninstallReport),
    StubInstalled(powermise_lifecycle::StubInstallReport),
    StubRemoved(powermise_lifecycle::StubRemoveReport),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let presentation = Presentation::new(cli.color, cli.output);
    match run(cli.command, &presentation) {
        Ok(output) => print_output(output, &presentation),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command, presentation: &Presentation) -> Result<Output, Box<dyn Error>> {
    let output: Output = match command {
        Command::Registry {
            command: RegistryCommand::Validate { catalog },
        } => {
            let paths = catalog.into_paths()?;
            validate_registry_files(&paths)?;
            Output::Valid
        }
        Command::Selection {
            command:
                SelectionCommand::Create {
                    name,
                    selection,
                    environment,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let imported = SelectionStore::new(&layout).import(&selection, Some(&name))?;
            Output::Created(imported)
        }
        Command::Selection {
            command:
                SelectionCommand::Import {
                    selection,
                    name,
                    environment,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let imported = SelectionStore::new(&layout).import(&selection, name.as_deref())?;
            Output::Imported(imported)
        }
        Command::Selection {
            command: SelectionCommand::List { environment },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let names = SelectionStore::new(&layout).list()?;
            Output::Listed(names)
        }
        Command::Selection {
            command: SelectionCommand::Export { name, environment },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let contents = SelectionStore::new(&layout).export(&name)?;
            Output::Exported(contents)
        }
        Command::Selection {
            command:
                SelectionCommand::Show {
                    name,
                    catalog,
                    environment,
                    target,
                    machine_role,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let context = compile_context(
                &paths,
                &layout.selection_path(&name),
                target.into(),
                machine_role.as_deref(),
            )?;
            Output::Resolved(context.resolved)
        }
        Command::Selection {
            command:
                SelectionCommand::Validate {
                    catalog,
                    selection,
                    target,
                    machine_role,
                },
        } => {
            let paths = catalog.into_paths()?;
            validate_selection_files(&paths, &selection, target.into(), machine_role.as_deref())?;
            Output::Valid
        }
        Command::Selection {
            command:
                SelectionCommand::Compile {
                    catalog,
                    selection,
                    target,
                    machine_role,
                },
        } => {
            let paths = catalog.into_paths()?;
            let context =
                compile_context(&paths, &selection, target.into(), machine_role.as_deref())?;
            Output::Compiled(context.artifact.contents().into())
        }
        Command::Selection {
            command:
                SelectionCommand::Activate {
                    name,
                    catalog,
                    environment,
                    target,
                    machine_role,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            Output::Activated(activate(
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &mise_config_dir,
            )?)
        }
        Command::Selection {
            command:
                SelectionCommand::Diff {
                    name,
                    catalog,
                    environment,
                    target,
                    machine_role,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            Output::Diffed(diff(
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &mise_config_dir,
            )?)
        }
        Command::Selection {
            command:
                SelectionCommand::Interactive {
                    name,
                    catalog,
                    environment,
                    machine_role,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let harnesses = load_catalog(&paths.harnesses)?;
            let apps = load_app_catalog(&paths.apps)?;
            let selection = interactive::run_interactive_selection(
                &mut io::stdin().lock(),
                &name,
                &harnesses,
                &apps,
                machine_role.as_deref(),
            )
            .map_err(Box::<dyn Error>::from)?;
            let written = SelectionStore::new(&layout).create(&selection)?;
            println!(
                "{}",
                presentation.ok(format!(
                    "created selection `{name}` from {} harness(es) at {}",
                    selection.harnesses.len(),
                    written.display()
                ))
            );
            Output::Created(written)
        }
        Command::Shell {
            command:
                ShellCommand::Install {
                    name,
                    shell,
                    environment,
                },
        } => {
            let _ = environment;
            let shell: powermise_lifecycle::Shell = shell.into();
            Output::ShellInstalled(powermise_lifecycle::install_shell_integration(
                &shell, &name, None,
            )?)
        }
        Command::Shell {
            command:
                ShellCommand::Uninstall {
                    name,
                    shell,
                    yes,
                    environment,
                },
        } => {
            let _ = environment;
            let shell: powermise_lifecycle::Shell = shell.into();
            Output::ShellUninstalled(powermise_lifecycle::uninstall_shell_integration(
                &shell, &name, None, yes,
            )?)
        }
        Command::Stub {
            command:
                StubCommand::Install {
                    name,
                    target,
                    machine_role,
                    bin_dir,
                    catalog,
                    environment,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            Output::StubInstalled(powermise_lifecycle::install_stubs(
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &mise_config_dir,
                bin_dir.as_deref(),
            )?)
        }
        Command::Stub {
            command:
                StubCommand::Remove {
                    name,
                    yes,
                    environment,
                },
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            Output::StubRemoved(powermise_lifecycle::remove_stubs(&layout, &name, !yes)?)
        }
        Command::Plan {
            catalog,
            environment,
            selection,
            target,
            machine_role,
        } => {
            let paths = catalog.into_paths()?;
            let target = target.into();
            let context = compile_context(&paths, &selection, target, machine_role.as_deref())?;
            let config_directory = resolve_mise_config_directory(environment.mise_config_dir)?;
            let config_path = context.artifact.config_path(&config_directory);
            let change =
                inspect_generated_file(&config_path, context.artifact.contents().as_bytes())?;
            let request = MiseDryRunRequest {
                environment: context.artifact.manifest().environment.clone(),
                config_file_name: context.artifact.manifest().generated_config_file.clone(),
                config_contents: context.artifact.contents().into(),
            };
            let mise = MiseExecutable::new(environment.mise).bootstrap_dry_run(&request)?;

            Output::Plan(PlanOutput {
                resolved: context.resolved,
                change,
                plan: context.plan,
                mise,
            })
        }
        Command::Doctor {
            catalog,
            environment,
            name,
            target,
            machine_role,
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            let findings = doctor(
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &mise_config_dir,
                &environment.mise,
            );
            Output::Doctored(findings)
        }
        Command::Status {
            catalog,
            environment,
            name,
            target,
            machine_role,
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            let runner = MiseExecutable::new(environment.mise);
            Output::Statused(status(
                &runner,
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &mise_config_dir,
            )?)
        }
        Command::Exec {
            catalog,
            environment,
            name,
            target,
            machine_role,
            command,
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let runner = MiseExecutable::new(environment.mise);
            Output::Executed(exec(
                &runner,
                &layout,
                &paths,
                &name,
                target.into(),
                machine_role.as_deref(),
                &command,
            )?)
        }
        Command::Prune { environment, yes } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            Output::Pruned(prune(&layout, &mise_config_dir, !yes)?)
        }
        Command::Uninstall {
            environment,
            name,
            yes,
        } => {
            let layout = resolve_layout(environment.powermise_dir)?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            Output::Uninstalled(uninstall(&layout, &name, &mise_config_dir, !yes)?)
        }
        Command::Apply {
            catalog,
            environment,
            name,
            target,
            machine_role,
            skip_bootstrap,
            yes,
        } => {
            if !yes {
                confirm_apply(&name, presentation)?;
            }
            let layout = resolve_layout(environment.powermise_dir)?;
            let paths = catalog.into_paths()?;
            let mise_config_dir = resolve_mise_config_directory(environment.mise_config_dir)?;
            let adapters_config_dir =
                resolve_adapters_config_directory(environment.adapters_config_dir, &layout)?;
            let runner = MiseExecutable::new(environment.mise);
            let options = ApplyOptions {
                layout: &layout,
                paths: &paths,
                target: target.into(),
                machine_role: machine_role.as_deref(),
                mise_config_dir: &mise_config_dir,
                adapters_config_dir: &adapters_config_dir,
                run_mise_bootstrap: !skip_bootstrap,
                progress: Some(&|message: &str| {
                    if presentation.show_progress() {
                        eprintln!("{message}");
                    }
                }),
            };
            Output::Applied(apply(&runner, &options, &name)?)
        }
    };

    Ok(output)
}

fn print_output(output: Output, presentation: &Presentation) -> ExitCode {
    if presentation.json() {
        let document = json_document(&output);
        println!(
            "{}",
            serde_json::to_string_pretty(&document).expect("output is serializable")
        );
        return exit_code(&output);
    }
    match output {
        Output::Valid => {
            println!("{}", presentation.ok("valid"));
            ExitCode::SUCCESS
        }
        Output::Resolved(resolved) => {
            print_resolved(&resolved, presentation);
            ExitCode::SUCCESS
        }
        Output::Compiled(toml) => {
            print!("{toml}");
            ExitCode::SUCCESS
        }
        Output::Plan(plan) => {
            print_plan(plan, presentation);
            ExitCode::SUCCESS
        }
        Output::Created(path) => {
            println!(
                "{}",
                presentation.ok(format!("created selection at {}", path.display()))
            );
            ExitCode::SUCCESS
        }
        Output::Listed(names) => {
            if names.is_empty() {
                println!("{}", presentation.dim("(no managed selections)"));
            } else {
                println!(
                    "{}",
                    render_table(
                        &["selection"],
                        &names.into_iter().map(|name| vec![name]).collect::<Vec<_>>()
                    )
                );
            }
            ExitCode::SUCCESS
        }
        Output::Exported(contents) => {
            print!("{contents}");
            ExitCode::SUCCESS
        }
        Output::Imported(path) => {
            println!(
                "{}",
                presentation.ok(format!("imported selection at {}", path.display()))
            );
            ExitCode::SUCCESS
        }
        Output::Activated(activation) => {
            println!(
                "activated {}: {} {}",
                activation.manifest_path.display(),
                change_label(activation.change.kind, presentation),
                activation.config_path.display()
            );
            if activation.ledger_updated {
                println!("{}", presentation.dim("ownership ledger updated"));
            }
            ExitCode::SUCCESS
        }
        Output::Diffed(diff) => {
            println!(
                "{}: {} {}",
                diff.selection,
                change_label(diff.change.kind, presentation),
                diff.change.path.display()
            );
            if !diff.change.diff.is_empty() {
                println!("{}", presentation.dim(diff.change.diff.trim_end()));
            }
            if let Some(drift) = diff.drift {
                println!("drift: {}", presentation.warning(drift));
            }
            ExitCode::SUCCESS
        }
        Output::Doctored(findings) => {
            for finding in &findings {
                println!(
                    "{} {}",
                    severity_label(finding.severity, presentation),
                    finding.message
                );
            }
            if has_errors(&findings) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Output::Statused(report) => {
            println!("selection: {}", report.selection);
            println!("environment: {}", report.environment);
            println!(
                "generated config: {} {}",
                change_label(report.change.kind, presentation),
                report.change.path.display()
            );
            if let Some(drift) = report.drift {
                println!("drift: {}", presentation.warning(drift));
            }
            println!(
                "ownership ledger: {}",
                if report.ledger_owned {
                    presentation.ok("recorded")
                } else {
                    presentation.error("missing")
                }
            );
            println!(
                "run report: {}",
                if report.run_report_exists {
                    presentation.ok("present")
                } else {
                    presentation.dim("absent")
                }
            );
            if report.mise_follow_ups.is_empty() {
                println!("mise follow-ups: {}", presentation.ok("none"));
            } else {
                println!("mise follow-ups:");
                for follow_up in &report.mise_follow_ups {
                    println!("- {}", presentation.yellow(follow_up));
                }
            }
            if !report.native.is_empty() {
                let rows = report
                    .native
                    .iter()
                    .map(|entry| {
                        vec![
                            entry.operation_id.clone(),
                            format!("{:?}", entry.provider),
                            entry.package.clone(),
                            if entry.present {
                                "present".into()
                            } else {
                                "missing".into()
                            },
                            entry.detail.clone(),
                        ]
                    })
                    .collect::<Vec<_>>();
                print!(
                    "{}",
                    render_table(
                        &["operation", "provider", "package", "state", "detail"],
                        &rows
                    )
                );
            }
            let satisfied = if report.satisfied {
                presentation.ok("satisfied")
            } else {
                presentation.warning("pending")
            };
            println!("status: {satisfied}");
            if report.satisfied {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Output::Executed(report) => {
            if !report.status.success() {
                eprintln!(
                    "command exited with status {}",
                    report
                        .status
                        .code()
                        .map_or_else(|| "signal".into(), |code| code.to_string())
                );
                // Pass the child's own exit code through; a signal (no
                // code) or an out-of-range code maps to the generic failure.
                report
                    .status
                    .code()
                    .and_then(|code| u8::try_from(code).ok())
                    .filter(|code| *code != 0)
                    .map_or(ExitCode::FAILURE, ExitCode::from)
            } else {
                ExitCode::SUCCESS
            }
        }
        Output::Pruned(report) => {
            let mode = if report.dry_run {
                "would remove"
            } else {
                "removed"
            };
            for action in &report.actions {
                match action {
                    powermise_lifecycle::PruneAction::Remove { path, reason } => {
                        println!(
                            "{} {mode} {} ({reason})",
                            presentation.warning("prune:"),
                            path.display()
                        );
                    }
                    powermise_lifecycle::PruneAction::SkipUnmanaged { path } => {
                        println!(
                            "{} skipped unmanaged {}",
                            presentation.dim("skip:"),
                            path.display()
                        );
                    }
                }
            }
            println!(
                "{}: {} {} artifact(s), {} unmanaged skipped, ledger_updated={}",
                presentation.accent("prune"),
                report.removed(),
                if report.dry_run {
                    "candidates"
                } else {
                    "removed"
                },
                report.skipped_unmanaged(),
                report.ledger_updated
            );
            ExitCode::SUCCESS
        }
        Output::Uninstalled(report) => {
            let mode = if report.dry_run {
                "would remove"
            } else {
                "removed"
            };
            for action in &report.actions {
                println!(
                    "{} {mode} {} ({})",
                    presentation.warning("uninstall:"),
                    action.path.display(),
                    action.reason
                );
            }
            println!(
                "{} {}: {} artifact(s), ledger_updated={}",
                presentation.accent("uninstall"),
                report.selection,
                report.actions.len(),
                report.ledger_updated
            );
            ExitCode::SUCCESS
        }
        Output::Applied(report) => {
            println!(
                "applied {}: {} {}",
                report.selection,
                change_label(report.change.kind, presentation),
                report.change.path.display()
            );
            if !report.change.diff.is_empty() {
                println!("{}", presentation.dim(report.change.diff.trim_end()));
            }
            match &report.bootstrap {
                BootstrapOutcome::Converged => {
                    println!(
                        "{} {}",
                        presentation.accent("mise bootstrap:"),
                        presentation.ok("converged")
                    );
                }
                BootstrapOutcome::Failed { message } => {
                    println!(
                        "{} {}",
                        presentation.error("mise bootstrap:"),
                        presentation.warning(message)
                    );
                }
                BootstrapOutcome::Skipped { reason } => {
                    println!(
                        "{} {}",
                        presentation.dim("mise bootstrap:"),
                        presentation.dim(reason)
                    );
                }
            }
            if !report.results.is_empty() {
                let rows = report
                    .results
                    .iter()
                    .map(|result| {
                        vec![
                            result.operation_id.clone(),
                            format!("{:?}", result.outcome),
                            result.message.clone().unwrap_or_default(),
                        ]
                    })
                    .collect::<Vec<_>>();
                println!("operations:");
                println!(
                    "{}",
                    render_table(&["operation", "outcome", "message"], &rows)
                );
            }
            if !report.mise_follow_ups.is_empty() {
                println!("mise follow-up actions:");
                for follow_up in &report.mise_follow_ups {
                    println!("- {}", presentation.yellow(follow_up));
                }
            }
            if !report.recovery_guidance.is_empty() {
                println!("{}", presentation.error("recovery guidance:"));
                for guidance in &report.recovery_guidance {
                    println!("- {guidance}");
                }
            }
            println!("run report: {}", report.run_report_path.display());
            ExitCode::SUCCESS
        }
        Output::ShellInstalled(report) => {
            if report.inserted {
                println!(
                    "{} installed shell integration for selection in {}",
                    presentation.ok("shell:"),
                    report.rc_path.display()
                );
            } else {
                println!(
                    "{} already installed in {}",
                    presentation.dim("shell:"),
                    report.rc_path.display()
                );
            }
            ExitCode::SUCCESS
        }
        Output::ShellUninstalled(report) => {
            let mode = if report.dry_run {
                "would remove"
            } else {
                "removed"
            };
            if report.removed {
                println!(
                    "{} {mode} managed shell block from {}",
                    presentation.warning("shell:"),
                    report.rc_path.display()
                );
            } else {
                println!(
                    "{} no managed shell block found in {}",
                    presentation.dim("shell:"),
                    report.rc_path.display()
                );
            }
            ExitCode::SUCCESS
        }
        Output::StubInstalled(report) => {
            if report.created.is_empty() && report.unchanged.is_empty() {
                println!(
                    "{} no mise-managed tools to stub for {}",
                    presentation.dim("stub:"),
                    report.selection
                );
            }
            for path in &report.created {
                println!(
                    "{} created lazy-install stub {}",
                    presentation.ok("stub:"),
                    path.display()
                );
            }
            for path in &report.unchanged {
                println!(
                    "{} stub unchanged {}",
                    presentation.dim("stub:"),
                    path.display()
                );
            }
            if report.ledger_updated {
                println!("{}", presentation.dim("ownership ledger updated"));
            }
            ExitCode::SUCCESS
        }
        Output::StubRemoved(report) => {
            let mode = if report.dry_run {
                "would remove"
            } else {
                "removed"
            };
            if report.removed.is_empty() {
                println!(
                    "{} no stubs owned by selection {}",
                    presentation.dim("stub:"),
                    report.selection
                );
            }
            for path in &report.removed {
                println!(
                    "{} {mode} stub {}",
                    presentation.warning("stub:"),
                    path.display()
                );
            }
            if report.ledger_updated {
                println!("{}", presentation.dim("ownership ledger updated"));
            }
            ExitCode::SUCCESS
        }
    }
}

fn validate_registry_files(paths: &CatalogPaths) -> Result<(), powermise_registry::RegistryError> {
    powermise_registry::load_catalog(&paths.harnesses).and_then(|catalog| {
        powermise_registry::validate_catalog(&catalog).and_then(|()| {
            powermise_registry::load_tool_catalog(&paths.tools).and_then(|tools| {
                powermise_registry::load_app_catalog(&paths.apps).and_then(|apps| {
                    powermise_registry::load_integration_catalog(&paths.integrations).and_then(
                        |integrations| {
                            powermise_registry::load_mcp_catalog(&paths.mcp).and_then(|mcp| {
                                powermise_registry::validate_registry(
                                    &catalog,
                                    &tools,
                                    &integrations,
                                    &mcp,
                                    &apps,
                                )
                            })
                        },
                    )
                })
            })
        })
    })
}

fn validate_selection_files(
    paths: &CatalogPaths,
    selection: &Path,
    target: Platform,
    machine_role: Option<&str>,
) -> Result<(), powermise_registry::RegistryError> {
    powermise_registry::load_catalog(&paths.harnesses).and_then(|catalog| {
        powermise_registry::validate_catalog(&catalog).and_then(|()| {
            powermise_registry::load_selection(selection).and_then(|selection| {
                powermise_registry::validate_selection(&selection, &catalog, target, machine_role)
            })
        })
    })
}

fn resolve_layout(override_dir: Option<PathBuf>) -> Result<PowermiseLayout, Box<dyn Error>> {
    Ok(PowermiseLayout::resolve(override_dir)?)
}

fn resolve_mise_config_directory(
    override_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn Error>> {
    Ok(powermise_lifecycle::resolve_mise_config_dir(override_path)?)
}

fn resolve_adapters_config_directory(
    override_path: Option<PathBuf>,
    layout: &PowermiseLayout,
) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    Ok(layout.root.join("adapters"))
}

/// BUILD_PLAN Apply step 1: require confirmation unless `--yes` was supplied.
/// Non-interactive terminals (scripts, CI) must pass `--yes` explicitly so an
/// apply that installs real packages can never run by accident.
fn confirm_apply(name: &str, presentation: &Presentation) -> Result<(), Box<dyn Error>> {
    if !io::stdin().is_terminal() {
        return Err(
            "refusing to apply without --yes because the terminal is not interactive; \
             re-run with --yes to proceed"
                .into(),
        );
    }
    eprintln!(
        "{}",
        presentation.warning(format!(
            "! apply converges `{name}` for real: it runs `mise bootstrap --yes`, which \
             installs tools and packages, and applies native package operations"
        ))
    );
    eprint!("continue? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let confirmed = matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "ye" | "yes"
    );
    if confirmed {
        Ok(())
    } else {
        Err("apply aborted before any change; nothing was modified".into())
    }
}

fn print_resolved(resolved: &ResolvedSelection, presentation: &Presentation) {
    println!(
        "harnesses: {}",
        presentation.accent(resolved.harness_ids.join(", "))
    );
    println!(
        "tools: {}",
        presentation.accent(resolved.tool_ids.join(", "))
    );
    if !resolved.desktop_app_ids.is_empty() {
        println!(
            "desktop apps: {}",
            presentation.accent(resolved.desktop_app_ids.join(", "))
        );
    } else {
        println!(
            "desktop apps: {}",
            presentation.dim("none (opt in with preferences.include_desktop_apps)")
        );
    }
    if !resolved.unavailable_desktop_ids.is_empty() {
        println!(
            "unavailable desktop apps on this target: {}",
            presentation.warning(resolved.unavailable_desktop_ids.join(", "))
        );
    }
    println!("apps: {}", presentation.accent(resolved.app_ids.join(", ")));
    println!(
        "integrations: {}",
        presentation.accent(resolved.integration_ids.join(", "))
    );
    println!(
        "mcp servers: {}",
        presentation.accent(resolved.mcp_server_ids.join(", "))
    );
}

fn print_plan(output: PlanOutput, presentation: &Presentation) {
    println!("selection: {}", output.plan.selection);
    println!("target: {:?}", output.plan.platform);
    print_resolved(&output.resolved, presentation);
    println!(
        "generated config: {} {}",
        change_label(output.change.kind, presentation),
        output.change.path.display()
    );
    if !output.change.diff.is_empty() {
        println!("{}", presentation.dim(output.change.diff.trim_end()));
    }
    let operations = output.plan.operations.operations();
    if !operations.is_empty() {
        let rows = operations
            .iter()
            .map(|operation| {
                vec![
                    operation.id.clone(),
                    format!("{:?}", operation.provider),
                    operation.resource_id.clone(),
                    operation.requested_state.clone(),
                ]
            })
            .collect::<Vec<_>>();
        println!("operations:");
        println!(
            "{}",
            render_table(&["operation", "provider", "resource", "requested"], &rows)
        );
    }
    let native_operations = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.provider,
                OperationProvider::Winget
                    | OperationProvider::Scoop
                    | OperationProvider::Npm
                    | OperationProvider::VerifiedScript
            )
        })
        .collect::<Vec<_>>();
    if !native_operations.is_empty() {
        println!("native dry-run:");
        for operation in native_operations {
            match native_plan_command(
                operation.provider,
                &operation.resource_id,
                &operation.requested_state,
            ) {
                Ok(command) => println!("{command}"),
                Err(_) => println!("(no command available for {})", operation.id),
            }
        }
    }
    println!("mise dry-run:");
    if output.mise.stdout.trim().is_empty() {
        println!("{}", presentation.dim("(no operations reported)"));
    } else {
        println!("{}", output.mise.stdout.trim_end());
    }
    if !output.mise.stderr.trim().is_empty() {
        eprintln!("{}", output.mise.stderr.trim_end());
    }
}

fn severity_label(
    severity: powermise_lifecycle::FindingSeverity,
    presentation: &Presentation,
) -> String {
    match severity {
        powermise_lifecycle::FindingSeverity::Ok => presentation.ok("ok:"),
        powermise_lifecycle::FindingSeverity::Warning => presentation.warning("warning:"),
        powermise_lifecycle::FindingSeverity::Error => presentation.error("error:"),
    }
}

fn change_label(kind: GeneratedFileChangeKind, presentation: &Presentation) -> String {
    match kind {
        GeneratedFileChangeKind::Create => presentation.ok("create"),
        GeneratedFileChangeKind::Update => presentation.warning("update"),
        GeneratedFileChangeKind::Unchanged => presentation.dim("unchanged"),
    }
}

/// Builds the schema-stable JSON document for a command result. The envelope
/// is `{ "command", "schema_version": 1, "payload" }` and is always color-free.
fn json_document(output: &Output) -> serde_json::Value {
    let (command, payload) = match output {
        Output::Valid => ("valid", serde_json::json!({ "valid": true })),
        Output::Resolved(resolved) => ("selection-show", resolved_json(resolved)),
        Output::Compiled(toml) => ("selection-compile", serde_json::json!({ "toml": toml })),
        Output::Plan(plan) => ("plan", plan_json(plan)),
        Output::Created(path) => ("selection-create", serde_json::json!({ "path": path })),
        Output::Listed(names) => ("selection-list", serde_json::json!({ "selections": names })),
        Output::Exported(contents) => ("selection-export", serde_json::json!({ "toml": contents })),
        Output::Imported(path) => ("selection-import", serde_json::json!({ "path": path })),
        Output::Activated(activation) => ("selection-activate", activation_json(activation)),
        Output::Diffed(diff) => ("selection-diff", diff_json(diff)),
        Output::Doctored(findings) => ("doctor", doctor_json(findings)),
        Output::Statused(report) => ("status", status_json(report)),
        Output::Executed(report) => ("exec", exec_json(report)),
        Output::Pruned(report) => ("prune", prune_json(report)),
        Output::Uninstalled(report) => ("uninstall", uninstall_json(report)),
        Output::Applied(report) => ("apply", apply_json(report)),
        Output::ShellInstalled(report) => ("shell-install", shell_installed_json(report)),
        Output::ShellUninstalled(report) => ("shell-uninstall", shell_uninstalled_json(report)),
        Output::StubInstalled(report) => ("stub-install", stub_installed_json(report)),
        Output::StubRemoved(report) => ("stub-remove", stub_removed_json(report)),
    };
    serde_json::json!({
        "command": command,
        "schema_version": 1,
        "payload": payload,
    })
}

fn resolved_json(resolved: &ResolvedSelection) -> serde_json::Value {
    serde_json::json!({
        "harnesses": resolved.harness_ids,
        "tools": resolved.tool_ids,
        "desktop_apps": resolved.desktop_app_ids,
        "unavailable_desktop_apps": resolved.unavailable_desktop_ids,
        "apps": resolved.app_ids,
        "integrations": resolved.integration_ids,
        "mcp_servers": resolved.mcp_server_ids,
    })
}

fn plan_json(plan: &PlanOutput) -> serde_json::Value {
    let operations = plan
        .plan
        .operations
        .operations()
        .iter()
        .map(|operation| {
            serde_json::json!({
                "id": operation.id,
                "provider": operation.provider,
                "resource_kind": operation.resource_kind,
                "resource_id": operation.resource_id,
                "requested_state": operation.requested_state,
                "action": operation.action,
                "depends_on": operation.depends_on,
                "owner": operation.ownership.owner,
                "selection": operation.ownership.selection,
                "verification": operation.verification.kind,
                "reversibility": operation.reversibility.kind,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "selection": plan.plan.selection,
        "platform": plan.plan.platform,
        "resolved": resolved_json(&plan.resolved),
        "generated_config": serde_json::to_value(&plan.change).expect("change is serializable"),
        "operations": operations,
        "native_dry_run": native_dry_run_json(&plan.plan),
        "mise_dry_run": {
            "stdout": plan.mise.stdout,
            "stderr": plan.mise.stderr,
        },
    })
}

fn native_dry_run_json(plan: &Plan) -> Vec<serde_json::Value> {
    plan.operations
        .operations()
        .iter()
        .filter(|operation| {
            matches!(
                operation.provider,
                OperationProvider::Winget
                    | OperationProvider::Scoop
                    | OperationProvider::Npm
                    | OperationProvider::VerifiedScript
            )
        })
        .map(|operation| {
            let command = native_plan_command(
                operation.provider,
                &operation.resource_id,
                &operation.requested_state,
            )
            .ok();
            serde_json::json!({
                "operation": operation.id,
                "provider": operation.provider,
                "command": command,
            })
        })
        .collect()
}

fn activation_json(activation: &ActivationOutput) -> serde_json::Value {
    serde_json::json!({
        "change": serde_json::to_value(&activation.change).expect("change is serializable"),
        "config_path": activation.config_path,
        "manifest_path": activation.manifest_path,
        "ledger_updated": activation.ledger_updated,
    })
}

fn diff_json(diff: &DiffOutput) -> serde_json::Value {
    serde_json::json!({
        "selection": diff.selection,
        "change": serde_json::to_value(&diff.change).expect("change is serializable"),
        "drift": diff.drift,
    })
}

fn doctor_json(findings: &[Finding]) -> serde_json::Value {
    let findings_json = findings
        .iter()
        .map(|finding| {
            let severity = match finding.severity {
                powermise_lifecycle::FindingSeverity::Ok => "ok",
                powermise_lifecycle::FindingSeverity::Warning => "warning",
                powermise_lifecycle::FindingSeverity::Error => "error",
            };
            serde_json::json!({
                "severity": severity,
                "message": finding.message,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "findings": findings_json,
        "has_errors": has_errors(findings),
    })
}

fn status_json(report: &StatusReport) -> serde_json::Value {
    let native = report
        .native
        .iter()
        .map(|entry| {
            serde_json::json!({
                "operation": entry.operation_id,
                "provider": entry.provider,
                "package": entry.package,
                "present": entry.present,
                "detail": entry.detail,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "selection": report.selection,
        "platform": report.target,
        "environment": report.environment,
        "change": serde_json::to_value(&report.change).expect("change is serializable"),
        "drift": report.drift,
        "ledger_owned": report.ledger_owned,
        "run_report_exists": report.run_report_exists,
        "mise_follow_ups": report.mise_follow_ups,
        "native": native,
        "satisfied": report.satisfied,
    })
}

fn exec_json(report: &ExecReport) -> serde_json::Value {
    serde_json::json!({
        "selection": report.selection,
        "environment": report.environment,
        "success": report.status.success(),
        "status": report.status.code(),
    })
}

fn prune_json(report: &PruneReport) -> serde_json::Value {
    let actions = report
        .actions
        .iter()
        .map(|action| match action {
            powermise_lifecycle::PruneAction::Remove { path, reason } => {
                serde_json::json!({
                    "kind": "remove",
                    "path": path,
                    "reason": reason,
                })
            }
            powermise_lifecycle::PruneAction::SkipUnmanaged { path } => {
                serde_json::json!({
                    "kind": "skip-unmanaged",
                    "path": path,
                })
            }
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "dry_run": report.dry_run,
        "ledger_updated": report.ledger_updated,
        "removed": report.removed(),
        "skipped_unmanaged": report.skipped_unmanaged(),
        "actions": actions,
    })
}

fn uninstall_json(report: &UninstallReport) -> serde_json::Value {
    let actions = report
        .actions
        .iter()
        .map(|action| {
            serde_json::json!({
                "path": action.path,
                "reason": action.reason,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "selection": report.selection,
        "dry_run": report.dry_run,
        "ledger_updated": report.ledger_updated,
        "actions": actions,
    })
}

fn apply_json(report: &powermise_lifecycle::ApplyReport) -> serde_json::Value {
    let bootstrap = match &report.bootstrap {
        BootstrapOutcome::Converged => serde_json::json!({ "mode": "converged" }),
        BootstrapOutcome::Failed { message } => {
            serde_json::json!({ "mode": "failed", "message": message })
        }
        BootstrapOutcome::Skipped { reason } => {
            serde_json::json!({ "mode": "skipped", "reason": reason })
        }
    };
    serde_json::json!({
        "selection": report.selection,
        "change": serde_json::to_value(&report.change).expect("change is serializable"),
        "bootstrap": bootstrap,
        "results": serde_json::to_value(&report.results).expect("results are serializable"),
        "run_report_path": report.run_report_path,
        "mise_follow_ups": report.mise_follow_ups,
        "recovery_guidance": report.recovery_guidance,
        "ledger_updated": report.ledger_updated,
    })
}

fn shell_installed_json(report: &powermise_lifecycle::ShellInstallReport) -> serde_json::Value {
    serde_json::json!({
        "rc_path": report.rc_path,
        "inserted": report.inserted,
    })
}

fn shell_uninstalled_json(report: &powermise_lifecycle::ShellUninstallReport) -> serde_json::Value {
    serde_json::json!({
        "rc_path": report.rc_path,
        "dry_run": report.dry_run,
        "removed": report.removed,
    })
}

fn stub_installed_json(report: &powermise_lifecycle::StubInstallReport) -> serde_json::Value {
    serde_json::json!({
        "selection": report.selection,
        "bin_dir": report.bin_dir,
        "created": report.created,
        "unchanged": report.unchanged,
        "ledger_updated": report.ledger_updated,
    })
}

fn stub_removed_json(report: &powermise_lifecycle::StubRemoveReport) -> serde_json::Value {
    serde_json::json!({
        "selection": report.selection,
        "dry_run": report.dry_run,
        "removed": report.removed,
        "ledger_updated": report.ledger_updated,
    })
}

/// Mirrors the table-mode exit codes for JSON output.
fn exit_code(output: &Output) -> ExitCode {
    match output {
        Output::Doctored(findings) => {
            if has_errors(findings) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Output::Statused(report) => {
            if report.satisfied {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Output::Executed(report) => {
            if report.status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(u8::MAX)
            }
        }
        Output::Applied(report) => {
            let operation_failed = report
                .results
                .iter()
                .any(|result| result.outcome == OperationOutcome::Failed);
            let bootstrap_failed = matches!(report.bootstrap, BootstrapOutcome::Failed { .. });
            if operation_failed || bootstrap_failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        _ => ExitCode::SUCCESS,
    }
}
