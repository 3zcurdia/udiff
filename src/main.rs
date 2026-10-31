//! The shell: argv parsing, terminal lifecycle, the event loop and file I/O.
//! Everything it does is glue around the pure core — no parsing, no state
//! machine, no drawing decisions.

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod app;
mod conflict;
mod git;
mod ui;

use app::{App, Message, Outcome};
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::time::Duration;
use ui::theme::{default_theme, omarchy::OmarchyWatch};

const USAGE: &str = "usage: udiff <file> | udiff <remote> <local> <base> <merged>";

const HELP: &str = "\
usage: udiff <file> | udiff <remote> <local> <base> <merged>

resolve git merge conflicts in one file: remote | local over current

keys
  n / p        next / previous conflict
  r / l / b    take remote / local / both
  Enter / i    edit the current text (Esc stops editing)
  s            save the file and stage it
  q            quit without saving
";

/// What the user asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    /// `udiff <file>`
    File(PathBuf),
    /// `udiff <remote> <local> <base> <merged>` — the git mergetool form;
    /// only `<merged>` holds content, the other three sides come from the
    /// markers in it.
    Mergetool {
        remote: PathBuf,
        local: PathBuf,
        base: PathBuf,
        merged: PathBuf,
    },
    /// `--help` | `-h`
    Help,
    /// `--version` | `-V`
    Version,
}

/// RAII terminal: raw mode plus alternate screen on, restored on drop, so
/// every exit path (including an error mid-draw) returns the shell intact.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<TerminalGuard, String> {
        enable_raw_mode().map_err(|error| format!("udiff: cannot enter raw mode: {error}"))?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(format!("udiff: cannot enter the alternate screen: {error}"));
        }
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            ratatui::crossterm::cursor::Show,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&arguments) {
        Ok(Invocation::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Invocation::Version) => {
            println!("udiff {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Invocation::File(path)) => finish(open_and_run(&path)),
        Ok(Invocation::Mergetool { merged, .. }) => finish(open_and_run(&merged)),
        Err(()) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Pure argv parsing: one file, the four-argument mergetool form, or a flag.
/// Anything else is a usage error.
fn parse_args(args: &[String]) -> Result<Invocation, ()> {
    match args {
        [flag] if flag == "--help" || flag == "-h" => Ok(Invocation::Help),
        [flag] if flag == "--version" || flag == "-V" => Ok(Invocation::Version),
        [file] => Ok(Invocation::File(PathBuf::from(file))),
        [remote, local, base, merged] => Ok(Invocation::Mergetool {
            remote: PathBuf::from(remote),
            local: PathBuf::from(local),
            base: PathBuf::from(base),
            merged: PathBuf::from(merged),
        }),
        _ => Err(()),
    }
}

fn finish(result: Result<i32, String>) -> ExitCode {
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

/// Reads the file (errors name the file, parse errors the line), refuses
/// zero-conflict files, resolves the branch label, builds the session and
/// runs it. Returns the process exit code.
fn open_and_run(path: &Path) -> Result<i32, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("udiff: cannot read {}: {error}", path.display()))?;
    let file = conflict::parse(&text).map_err(|error| {
        format!(
            "udiff: line {}: {}",
            error.line,
            parse_error_message(&error.kind)
        )
    })?;
    if conflict::count_conflicts(&file) == 0 {
        return Err(format!("udiff: no conflicts in {}", path.display()));
    }
    let local_branch = git::current_branch_label();
    let mut app = App::new(path.to_path_buf(), file, local_branch);
    let mut watch = OmarchyWatch::new();
    let mut theme = watch.check().unwrap_or_else(default_theme);
    let _guard = TerminalGuard::new()?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))
            .map_err(|error| format!("udiff: cannot start the terminal: {error}"))?;

    // Redraw on input events only, plus a theme change — never on a timer.
    let mut redraw = true;
    loop {
        if redraw {
            terminal
                .draw(|frame| ui::render(frame, &app, &theme))
                .map_err(|error| format!("udiff: cannot draw: {error}"))?;
            redraw = false;
        }
        let has_input = event::poll(Duration::from_secs(1))
            .map_err(|error| format!("udiff: input error: {error}"))?;
        if !has_input {
            // The only work between keystrokes: re-check the OS theme, and
            // redraw only when it actually changed.
            if let Some(updated) = watch.check() {
                theme = updated;
                redraw = true;
            }
            continue;
        }
        let event = event::read().map_err(|error| format!("udiff: input error: {error}"))?;
        let Event::Key(key) = event else {
            redraw = true;
            continue;
        };
        match app.dispatch(ui::translate_key(key, app.mode)) {
            Outcome::Continue => redraw = true,
            Outcome::SaveRequested { text } => {
                // Restore the terminal before any output so the result is
                // readable; nothing draws after this point.
                drop(terminal);
                drop(_guard);
                save(&app.path, text)?;
                if let Some(Message::Info(note)) = &app.message {
                    println!("{note}");
                }
                return Ok(0);
            }
            Outcome::Quit { saved: _ } => return Ok(0),
        }
    }
}

/// Writes the resolved text, then stages it. The write always happens first
/// and is never lost; a staging failure only changes the exit code.
fn save(path: &Path, text: String) -> Result<i32, String> {
    std::fs::write(path, &text)
        .map_err(|error| format!("udiff: cannot write {}: {error}", path.display()))?;
    match git::stage(path) {
        Ok(()) => Ok(0),
        Err(message) => Err(format!("warning: {message}")),
    }
}

/// Fixed one-line wording for each parse failure — never the file's contents.
fn parse_error_message(kind: &conflict::ParseErrorKind) -> &'static str {
    match kind {
        conflict::ParseErrorKind::MissingSeparator => "missing ======= separator",
        conflict::ParseErrorKind::MissingClose => "unclosed <<<<<<< conflict",
        conflict::ParseErrorKind::MissingOpen => "======= or >>>>>>> outside a conflict",
        conflict::ParseErrorKind::NestedOpen => "<<<<<<< inside an open conflict",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_accepts_single_file_form() {
        let args = vec!["conflicted.txt".to_string()];
        assert_eq!(
            parse_args(&args),
            Ok(Invocation::File(PathBuf::from("conflicted.txt")))
        );
    }

    #[test]
    fn parse_args_accepts_mergetool_form() {
        let args: Vec<String> = ["remote", "local", "base", "merged"]
            .iter()
            .map(|arg| arg.to_string())
            .collect();
        assert_eq!(
            parse_args(&args),
            Ok(Invocation::Mergetool {
                remote: PathBuf::from("remote"),
                local: PathBuf::from("local"),
                base: PathBuf::from("base"),
                merged: PathBuf::from("merged"),
            })
        );
    }

    #[test]
    fn parse_args_rejects_wrong_arity() {
        let cases: [&[&str]; 4] = [
            &[],
            &["a", "b"],
            &["a", "b", "c"],
            &["a", "b", "c", "d", "e"],
        ];
        for case in cases {
            let args: Vec<String> = case.iter().map(|arg| arg.to_string()).collect();
            assert_eq!(parse_args(&args), Err(()), "arity {} rejected", case.len());
        }
    }

    #[test]
    fn parse_args_accepts_help_and_version() {
        for flag in ["--help", "-h"] {
            let args = vec![flag.to_string()];
            assert_eq!(parse_args(&args), Ok(Invocation::Help), "{flag}");
        }
        for flag in ["--version", "-V"] {
            let args = vec![flag.to_string()];
            assert_eq!(parse_args(&args), Ok(Invocation::Version), "{flag}");
        }
    }
}
