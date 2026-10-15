//! Prints the mise fragment an AI setup compiles to, for checking it against a
//! real mise:
//!
//!   cargo run -p powermise-setups --example fragment -- selections/engineering-agents.toml darwin
use std::{path::PathBuf, process::ExitCode};

use powermise_domain::Platform;
use powermise_setups::{CatalogPaths, compile_setup};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(selection), Some(target)) = (args.next(), args.next()) else {
        eprintln!("usage: fragment <selection.toml> <darwin|linux|win32|wsl>");
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
    match compile_setup(&paths, &PathBuf::from(selection), target) {
        Ok(setup) => {
            print!("{}", setup.fragment.contents());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
