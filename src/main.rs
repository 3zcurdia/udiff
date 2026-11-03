//! The shell: argv parsing, terminal lifecycle, the event loop and file I/O.
//! Everything it does is glue around the pure core — no parsing, no state
//! machine, no drawing decisions.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod app;
mod conflict;
mod git;
mod ui;

use app::{Action, App, Mode, Outcome};
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

Normal mode
  n / p        next / previous conflict (stops at the ends)
  r / l / b    take remote / local / both (both = local, then remote)
  Enter / i    enter Edit mode
  ↑ / ↓        scroll all panels one line
  PgUp / PgDn  scroll all panels ten lines
  ← / →        scroll all panels sideways
  Home         scroll back to the selected conflict
  s            save: refused while any conflict is unresolved; writes the
               file, stages it (udiff <file> form), exits 0
  q            quit without saving, exit 1 (press twice if there are edits)

Edit mode
  printable    insert at the cursor          Tab: insert a tab
  Enter        newline
  Backspace    delete before the cursor      Delete: delete at the cursor
  arrows       move the cursor               Home / End: line start / end
  Esc          back to Normal mode

exit codes
  0  saved   1  quit or error   2  usage   3  saved, but git add failed
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
        Ok(Invocation::File(path)) => finish(open_and_run(&path, true)),
        // git mergetool stages `$MERGED` itself after a zero exit.
        Ok(Invocation::Mergetool { merged, .. }) => finish(open_and_run(&merged, false)),
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
/// zero-conflict files, resolves the branch label when a conflict's local
/// label names no branch, builds the session and runs it. `stage` runs
/// `git add` after a save. Returns the process exit code.
fn open_and_run(path: &Path, stage: bool) -> Result<i32, String> {
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
    // The branch is resolved only when some conflict's local label names no
    // branch of its own — otherwise the lookup would never be shown.
    let needs_branch = file.regions.iter().any(|region| {
        matches!(
            region,
            conflict::Region::Conflict(conflict) if git::label_needs_branch(&conflict.local_label)
        )
    });
    let local_branch = if needs_branch {
        git::current_branch_label()
    } else {
        None
    };
    let mut app = App::new(path.to_path_buf(), file, local_branch);
    let mut watch = OmarchyWatch::new();
    let mut theme = watch.check().unwrap_or_else(default_theme);
    let _guard = TerminalGuard::new()?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))
            .map_err(|error| format!("udiff: cannot start the terminal: {error}"))?;

    // Redraw on input events only, plus a theme change — never on a timer.
    // The widest line is re-measured only when some resolution text changed.
    let mut redraw = true;
    let mut widest_cache: (Option<u64>, usize) = (None, 0);
    loop {
        if redraw {
            let mut clamped = app.scroll;
            terminal
                .draw(|frame| {
                    let view = ui::View::new(&app);
                    if widest_cache.0 != Some(app.revision) {
                        widest_cache = (Some(app.revision), ui::widest_line(&view));
                    }
                    clamped = ui::clamp_scroll(&app, &view, widest_cache.1, frame.area());
                    ui::render(frame, &app, &view, clamped, &theme)
                })
                .map_err(|error| format!("udiff: cannot draw: {error}"))?;
            app.scroll = clamped;
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
        match step_for(&event, app.mode) {
            Step::Skip => continue,
            Step::Redraw => redraw = true,
            Step::Dispatch(action) => match app.dispatch(action) {
                Outcome::Continue => redraw = true,
                Outcome::SaveRequested { text } => {
                    // Restore the terminal before any output so the result is
                    // readable; nothing draws after this point.
                    drop(terminal);
                    drop(_guard);
                    return save(&app.path, &text, stage);
                }
                Outcome::Quit => return Ok(1),
            },
        }
    }
}

/// What one input event asks the shell to do with it.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Hand the event's action to `App::dispatch`.
    Dispatch(Action),
    /// Redraw without touching the session (a resize).
    Redraw,
    /// Neither: an unbound key or an event nothing observes.
    Skip,
}

/// The event → step mapping. Only a key that maps to an action can change
/// state, and only a resize changes the screen on its own — everything else
/// (unbound keys, focus, mouse, paste) costs no redraw.
fn step_for(event: &Event, mode: Mode) -> Step {
    match event {
        Event::Key(key) => match ui::translate_key(*key, mode) {
            Action::Ignored => Step::Skip,
            action => Step::Dispatch(action),
        },
        Event::Resize(..) => Step::Redraw,
        _ => Step::Skip,
    }
}

/// Writes the resolved text, then stages it when `stage`. The write always
/// happens first; a staging failure is a warning and exit code 3.
fn save(path: &Path, text: &str, stage: bool) -> Result<i32, String> {
    write_atomically(path, text)
        .map_err(|error| format!("udiff: cannot write {}: {error}", path.display()))?;
    if !stage {
        println!("udiff: saved {}", path.display());
        return Ok(0);
    }
    match git::stage(path) {
        Ok(()) => {
            println!("udiff: saved and staged {}", path.display());
            Ok(0)
        }
        Err(message) => {
            eprintln!("udiff: warning: {message}");
            Ok(3)
        }
    }
}

/// Writes `text` to `path` through a sibling temp file
/// (`.<file name>.udiff-<pid>.tmp`) and renames it over the target, so the
/// target file itself is touched only by the rename: a crash, a kill or a full
/// disk leaves either the old file or the new one, never a truncated one. The
/// temp file inherits the target's permission bits; symlinks are followed, so
/// the link itself is preserved. On any error the temp file is removed
/// best-effort and the original file is left byte-identical.
fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    // Follow symlinks first: the rename must replace the real file, not the link.
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let file_name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file path"))?;
    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".udiff-{}.tmp", std::process::id()));
    let temp = target.with_file_name(temp_name);

    let written = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        if let Ok(metadata) = std::fs::metadata(&target) {
            file.set_permissions(metadata.permissions())?;
        }
        file.sync_all()?;
        std::fs::rename(&temp, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Fixed one-line wording for each parse failure — never the file's contents.
fn parse_error_message(kind: &conflict::ParseErrorKind) -> &'static str {
    match kind {
        conflict::ParseErrorKind::MissingSeparator => "missing ======= separator",
        conflict::ParseErrorKind::MissingClose => "unclosed <<<<<<< conflict",
        conflict::ParseErrorKind::MissingOpen => ">>>>>>> outside a conflict",
        conflict::ParseErrorKind::NestedOpen => "<<<<<<< inside an open conflict",
        conflict::ParseErrorKind::MisplacedBase => "||||||| outside the local side of a conflict",
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

    /// A unique path under the system temp dir, outside any git repo.
    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udiff-main-{}-{name}", std::process::id()))
    }

    #[test]
    fn save_without_stage_writes_and_exits_0() {
        let path = temp_path("no-stage");
        assert_eq!(save(&path, "x\n", false), Ok(0));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x\n");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn save_with_failed_stage_exits_3() {
        let path = temp_path("failed-stage");
        assert_eq!(save(&path, "x\n", true), Ok(3));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x\n");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn save_to_unwritable_path_errors() {
        let path = std::env::temp_dir().join("udiff-missing-dir/x");
        assert!(save(&path, "x\n", false).is_err());
    }

    /// A directory of its own under the system temp dir, so the test can see
    /// every file a save leaves behind.
    fn temp_dir(name: &str) -> PathBuf {
        let directory = temp_path(name);
        std::fs::create_dir_all(&directory).expect("create the temp dir");
        directory
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let directory = temp_dir("atomic");
        let path = directory.join("resolved.txt");
        assert_eq!(save(&path, "x\n", false), Ok(0));
        let entries: Vec<_> = std::fs::read_dir(&directory)
            .expect("read the temp dir")
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "only the saved file, no temp file left behind"
        );
        assert_eq!(std::fs::read_to_string(&path).expect("read it back"), "x\n");
        std::fs::remove_dir_all(&directory).expect("remove the temp dir");
    }

    #[cfg(unix)]
    #[test]
    fn save_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let directory = temp_dir("perm");
        let path = directory.join("resolved.txt");
        std::fs::write(&path, "orig\n").expect("seed the file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("set 0755");
        assert_eq!(save(&path, "x\n", false), Ok(0));
        let mode = std::fs::metadata(&path)
            .expect("stat the saved file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755, "the permission bits survive the save");
        std::fs::remove_dir_all(&directory).expect("remove the temp dir");
    }

    #[cfg(unix)]
    #[test]
    fn save_through_symlink_keeps_link() {
        use std::os::unix::fs::symlink;
        let directory = temp_dir("symlink");
        let real = directory.join("real.txt");
        let link = directory.join("link.txt");
        std::fs::write(&real, "orig\n").expect("seed the real file");
        symlink(&real, &link).expect("link the real file");
        assert_eq!(save(&link, "x\n", false), Ok(0));
        assert!(
            std::fs::symlink_metadata(&link)
                .expect("stat the link")
                .file_type()
                .is_symlink(),
            "the path is still a symlink"
        );
        assert_eq!(
            std::fs::read_to_string(&real).expect("read the real file"),
            "x\n",
            "the target holds the resolved text"
        );
        std::fs::remove_dir_all(&directory).expect("remove the temp dir");
    }

    #[cfg(unix)]
    #[test]
    fn failed_save_leaves_original() {
        use std::os::unix::fs::PermissionsExt;
        let directory = temp_dir("readonly");
        let path = directory.join("resolved.txt");
        std::fs::write(&path, "orig\n").expect("seed the file");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555))
            .expect("make the directory read-only");

        // Root ignores the mode bits: a successful probe create means the
        // chmod had no effect, so there is nothing to assert here.
        if std::fs::File::create(directory.join("probe")).is_ok() {
            std::fs::remove_file(directory.join("probe")).expect("remove the probe");
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
                .expect("restore the directory");
            std::fs::remove_dir_all(&directory).expect("remove the temp dir");
            return;
        }

        let error = save(&path, "x\n", false).expect_err("the save must fail");
        assert!(
            error.starts_with("udiff: cannot write"),
            "the error names the write, got: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read the original"),
            "orig\n",
            "the original file is byte-identical"
        );

        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("restore the directory");
        std::fs::remove_dir_all(&directory).expect("remove the temp dir");
    }

    #[test]
    fn help_lists_every_key() {
        for needle in [
            "n / p",
            "r / l / b",
            "Enter / i",
            "PgUp",
            "Home",
            "Tab",
            "Esc",
            "exit codes",
        ] {
            assert!(HELP.contains(needle), "--help mentions {needle:?}");
        }
    }

    #[test]
    fn step_for_cases() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};

        let key = |code: KeyCode| Event::Key(KeyEvent::from(code));
        assert_eq!(
            step_for(&key(KeyCode::F(5)), Mode::Normal),
            Step::Skip,
            "an unbound key draws nothing"
        );
        assert_eq!(
            step_for(&key(KeyCode::Char('n')), Mode::Normal),
            Step::Dispatch(Action::NextConflict)
        );
        assert_eq!(step_for(&Event::Resize(80, 24), Mode::Normal), Step::Redraw);
        assert_eq!(
            step_for(&Event::FocusGained, Mode::Normal),
            Step::Skip,
            "a focus event draws nothing"
        );
    }
}
