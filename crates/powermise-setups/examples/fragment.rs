//! Prints the mise fragment an AI setup compiles to, for checking it against a
//! real mise:
//!
//!   cargo run -p powermise-setups --example fragment -- selections/engineering-agents.toml darwin
//!
//! With a third argument, also writes the fragment and its MCP config sources
//! into that mise config directory, as the app does (agents' configs are
//! resolved from `HOME`).
use std::{path::PathBuf, process::ExitCode};

use powermise_domain::Platform;
use powermise_setups::{CatalogPaths, compile_setup, home_dir, write_mcp_files};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(selection), Some(target)) = (args.next(), args.next()) else {
        eprintln!("usage: fragment <selection.toml> <darwin|linux|win32|wsl> [mise config dir]");
        return ExitCode::FAILURE;
    };
    let target = match target.as_str() {
        "darwin" => Platform::Darwin,
        "linux" => Platform::Linux,
        "win32" => Platform::Win32,
        "wsl" => Platform::Wsl,
        other => {
            eprintln!("unknown target `{other}`");
            return ExitCode::FAILURE;
        }
    };
    let paths = CatalogPaths::in_dir(&PathBuf::from("catalog/profiles"));
    let write_to = args.next().map(PathBuf::from);
    match compile_setup(&paths, &PathBuf::from(&selection), target, None) {
        Ok(setup) => {
            print!("{}", setup.fragment.contents());
            if let Some(dir) = write_to {
                let name = powermise_registry::load_selection(&PathBuf::from(&selection))
                    .map(|selection| selection.name)
                    .unwrap_or_default();
                let home = home_dir().unwrap_or_default();
                if let Err(error) = setup
                    .fragment
                    .write(&dir)
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        write_mcp_files(&dir, &name, &setup.compiled.mcp_files, &home, target)
                            .map_err(|error| error.to_string())
                    })
                {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
