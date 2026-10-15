//! Opening the person's terminal app in a folder, running one mise command.
use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};

/// Single-quotes `text` for a POSIX shell.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Opens a terminal window in `directory` that runs `mise <arguments>` and
/// stays open afterwards.
pub fn open(directory: &Path, mise: &Path, arguments: &[String]) -> io::Result<()> {
    let command_line = std::iter::once(shell_quote(&mise.to_string_lossy()))
        .chain(arguments.iter().map(|argument| shell_quote(argument)))
        .collect::<Vec<_>>()
        .join(" ");
    let directory_text = directory.to_string_lossy();

    if cfg!(target_os = "macos") {
        // Terminal.app runs the line in a new window; AppleScript strings
        // escape backslashes and double quotes.
        let script_line = format!("cd {} && {command_line}", shell_quote(&directory_text));
        let escaped = script_line.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "tell application \"Terminal\"\n  activate\n  do script \"{escaped}\"\nend tell"
        );
        return Command::new("osascript")
            .args(["-e", &script])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ());
    }

    if cfg!(windows) {
        // A new PowerShell window in the folder; `start` needs an empty title.
        let invocation = std::iter::once(format!(
            "& '{}'",
            mise.to_string_lossy().replace('\'', "''")
        ))
        .chain(
            arguments
                .iter()
                .map(|argument| format!("'{}'", argument.replace('\'', "''"))),
        )
        .collect::<Vec<_>>()
        .join(" ");
        return Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                "/D",
                &directory_text,
                "powershell",
                "-NoExit",
                "-Command",
                &invocation,
            ])
            .spawn()
            .map(|_| ());
    }

    // Linux: the first terminal that's installed. The shell stays open after
    // mise exits so the output can be read.
    let keep_open = format!("{command_line}; exec \"${{SHELL:-sh}}\"");
    let candidates: [(&str, Vec<String>); 5] = [
        (
            "x-terminal-emulator",
            vec!["-e".into(), "sh".into(), "-c".into(), keep_open.clone()],
        ),
        (
            "gnome-terminal",
            vec!["--".into(), "sh".into(), "-c".into(), keep_open.clone()],
        ),
        (
            "konsole",
            vec!["-e".into(), "sh".into(), "-c".into(), keep_open.clone()],
        ),
        (
            "xfce4-terminal",
            vec!["-x".into(), "sh".into(), "-c".into(), keep_open.clone()],
        ),
        (
            "xterm",
            vec!["-e".into(), "sh".into(), "-c".into(), keep_open.clone()],
        ),
    ];
    for (program, program_arguments) in candidates {
        if let Some(found) = crate::find_program(program) {
            return Command::new(found)
                .args(&program_arguments)
                .current_dir(directory)
                .spawn()
                .map(|_| ());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no terminal app was found (tried x-terminal-emulator, gnome-terminal, konsole, xfce4-terminal, xterm)",
    ))
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn quotes_survive_single_quotes() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("/a b/c"), "'/a b/c'");
    }
}
