//! Powermise-owned native installers and typed configuration adapters.
mod config;
mod native;

pub use config::{
    ClaudeMcpServer, ClaudeSettingsAdapter, ConfigAdapterError, JsonManagedAdapter,
    ManagedBlockAdapter, VscodeSettingsAdapter, claude_settings_path, vscode_settings_path,
};
pub use native::{
    NativeProviderError, NpmProvider, ScoopProvider, VerifiedScriptProvider, WingetProvider,
    native_check_command, native_plan_command, npm_check_args, npm_install_args, scoop_check_args,
    scoop_install_args, verify_sha256, winget_check_args, winget_install_args,
};