//! The session state machine: which conflict is selected, what its resolution
//! says, and what one keystroke does about it.
//!
//! Pure: no terminal, no subprocess, no `std::fs`. It decides what would be
//! saved; the shell in `src/main.rs` saves it.

use crate::conflict::{Conflict, ConflictFile, Region, count_conflicts, render_resolved};
use std::path::PathBuf;

/// One editing session over one conflicted file.
pub struct App {
    /// The file being resolved, as given on the command line.
    pub path: PathBuf,
    /// The parsed document; only `Conflict::current_text` ever changes.
    pub file: ConflictFile,
    /// 0-based internally, displayed 1-based as "conflict i of N".
    pub selected: usize,
    /// (line, column) in characters inside the selected conflict's text.
    pub cursor: (usize, usize),
    pub mode: Mode,
    /// True from the first mutation of any resolution; never resets.
    pub dirty: bool,
    /// Transient status-line message, replaced on the next key event.
    pub message: Option<Message>,
    /// The current side's branch name, resolved once at startup via git.
    pub local_branch: Option<String>,
}

/// Single-key commands in Normal mode; free text in Edit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Edit,
}

/// A one-line status message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

/// The user intent of one key event, produced by `ui::translate_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NextConflict,
    PrevConflict,
    TakeRemote,
    TakeLocal,
    TakeBoth,
    EnterEdit,
    ExitEdit,
    InsertChar(char),
    InsertNewline,
    Backspace,
    Delete,
    MoveCursor(Direction),
    Save,
    Quit,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// How the pure core tells the shell to touch the outside world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    SaveRequested { text: String },
    Quit { saved: bool },
}

impl App {
    /// Conflict 1, Normal mode, cursor at the end of the current text, clean,
    /// no message.
    pub fn new(path: PathBuf, file: ConflictFile, local_branch: Option<String>) -> App {
        let mut app = App {
            path,
            file,
            selected: 0,
            cursor: (0, 0),
            mode: Mode::Normal,
            dirty: false,
            message: None,
            local_branch,
        };
        app.cursor = end_cursor(&app.selected_conflict().current_text);
        app
    }

    /// Applies one action. Never performs I/O: `Save` yields the rendered text
    /// for the shell to write, `Quit` yields the saved flag.
    pub fn dispatch(&mut self, action: Action) -> Outcome {
        self.message = None;
        match action {
            Action::NextConflict => {
                if self.selected + 1 < count_conflicts(&self.file) {
                    self.selected += 1;
                    self.cursor = end_cursor(&self.selected_conflict().current_text);
                }
                Outcome::Continue
            }
            Action::PrevConflict => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.cursor = end_cursor(&self.selected_conflict().current_text);
                }
                Outcome::Continue
            }
            Action::TakeRemote => {
                let text = self.selected_conflict().remote_text.clone();
                self.replace_current(text);
                Outcome::Continue
            }
            Action::TakeLocal => {
                let text = self.selected_conflict().local_text.clone();
                self.replace_current(text);
                Outcome::Continue
            }
            Action::TakeBoth => {
                let (remote, local) = {
                    let conflict = self.selected_conflict();
                    (conflict.remote_text.clone(), conflict.local_text.clone())
                };
                self.replace_current(format!("{remote}{local}"));
                Outcome::Continue
            }
            Action::EnterEdit => {
                self.mode = Mode::Edit;
                Outcome::Continue
            }
            Action::ExitEdit => {
                self.mode = Mode::Normal;
                Outcome::Continue
            }
            Action::InsertChar(character) => {
                self.edit_current(|lines, cursor| insert_char(lines, cursor, character));
                Outcome::Continue
            }
            Action::InsertNewline => {
                self.edit_current(insert_newline);
                Outcome::Continue
            }
            Action::Backspace => {
                self.edit_current(backspace);
                Outcome::Continue
            }
            Action::Delete => {
                self.edit_current(delete_forward);
                Outcome::Continue
            }
            Action::MoveCursor(direction) => {
                self.move_cursor(direction);
                Outcome::Continue
            }
            Action::Save => match self.validate_save() {
                Ok(()) => {
                    self.message = Some(Message::Info("saved and staged".to_string()));
                    Outcome::SaveRequested {
                        text: render_resolved(&self.file),
                    }
                }
                Err(text) => {
                    self.message = Some(Message::Error(text));
                    Outcome::Continue
                }
            },
            Action::Quit => Outcome::Quit { saved: !self.dirty },
            Action::Ignored => Outcome::Continue,
        }
    }

    /// Rejects any `current_text` line starting with 7+ `<`, `=` or `>`
    /// characters; the returned text goes to the status line.
    pub fn validate_save(&self) -> Result<(), String> {
        let mut conflict_number = 0;
        for region in &self.file.regions {
            let conflict = match region {
                Region::Conflict(conflict) => conflict,
                Region::Text(_) => continue,
            };
            conflict_number += 1;
            for (offset, line) in conflict.current_text.split('\n').enumerate() {
                let content = line.strip_suffix('\r').unwrap_or(line);
                if starts_with_marker(content) {
                    return Err(format!(
                        "conflict {conflict_number}: line {} looks like a conflict marker",
                        offset + 1
                    ));
                }
            }
        }
        Ok(())
    }

    /// The conflict under the selection; unreachable on an empty file, which
    /// parse plus the zero-conflict exit make impossible.
    pub fn selected_conflict(&self) -> &Conflict {
        self.conflict_at(self.selected)
    }

    /// Swaps in a whole side (remote, local or both) and marks the session dirty.
    fn replace_current(&mut self, text: String) {
        let conflict = self.conflict_at_mut(self.selected);
        conflict.current_text = text;
        let cursor = end_cursor(&conflict.current_text);
        self.dirty = true;
        self.cursor = cursor;
    }

    /// Runs one text edit over the selected conflict's current text.
    fn edit_current<F>(&mut self, edit: F)
    where
        F: FnOnce(&mut Vec<String>, &mut (usize, usize)),
    {
        let index = self.selected;
        let mut lines: Vec<String> = self
            .conflict_at(index)
            .current_text
            .split('\n')
            .map(str::to_string)
            .collect();
        let mut cursor = self.cursor;
        clamp_cursor(&mut cursor, &lines);
        edit(&mut lines, &mut cursor);
        clamp_cursor(&mut cursor, &lines);
        let conflict = self.conflict_at_mut(index);
        conflict.current_text = lines.join("\n");
        self.cursor = cursor;
        self.dirty = true;
    }

    /// Moves the cursor inside the current text, bounded by its lines. A move
    /// mutates no text, so it never sets the dirty flag.
    fn move_cursor(&mut self, direction: Direction) {
        let index = self.selected;
        let lines: Vec<String> = self
            .conflict_at(index)
            .current_text
            .split('\n')
            .map(str::to_string)
            .collect();
        let mut cursor = self.cursor;
        clamp_cursor(&mut cursor, &lines);
        let (line, column) = cursor;
        let width = lines[line].chars().count();
        cursor = match direction {
            Direction::Left if column > 0 => (line, column - 1),
            Direction::Left if line > 0 => (line - 1, lines[line - 1].chars().count()),
            Direction::Right if column < width => (line, column + 1),
            Direction::Right if line + 1 < lines.len() => (line + 1, 0),
            Direction::Up if line > 0 => (line - 1, column.min(lines[line - 1].chars().count())),
            Direction::Down if line + 1 < lines.len() => {
                (line + 1, column.min(lines[line + 1].chars().count()))
            }
            _ => cursor,
        };
        self.cursor = cursor;
    }

    fn conflict_at(&self, index: usize) -> &Conflict {
        self.file
            .regions
            .iter()
            .filter_map(|region| match region {
                Region::Conflict(conflict) => Some(conflict),
                Region::Text(_) => None,
            })
            .nth(index)
            .expect("the zero-conflict exit guarantees at least one conflict")
    }

    fn conflict_at_mut(&mut self, index: usize) -> &mut Conflict {
        self.file
            .regions
            .iter_mut()
            .filter_map(|region| match region {
                Region::Conflict(conflict) => Some(conflict),
                Region::Text(_) => None,
            })
            .nth(index)
            .expect("the zero-conflict exit guarantees at least one conflict")
    }
}

const MARKER_LEN: usize = 7;

/// Cursor position at the very end of a text: past the last newline, at the
/// end of the last line.
fn end_cursor(text: &str) -> (usize, usize) {
    let line = text.matches('\n').count();
    let column = text.rsplit('\n').next().unwrap_or_default().chars().count();
    (line, column)
}

fn clamp_cursor(cursor: &mut (usize, usize), lines: &[String]) {
    cursor.0 = cursor.0.min(lines.len().saturating_sub(1));
    cursor.1 = cursor.1.min(lines[cursor.0].chars().count());
}

/// Byte offset of the `column`-th character (or the end of the text).
fn char_offset(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map_or(text.len(), |(offset, _)| offset)
}

fn starts_with_marker(line: &str) -> bool {
    ['<', '=', '>']
        .iter()
        .any(|&marker| line.chars().take_while(|&c| c == marker).count() >= MARKER_LEN)
}

fn insert_char(lines: &mut [String], cursor: &mut (usize, usize), character: char) {
    let text = &mut lines[cursor.0];
    let column = cursor.1.min(text.chars().count());
    text.insert(char_offset(text, column), character);
    *cursor = (cursor.0, column + 1);
}

fn insert_newline(lines: &mut Vec<String>, cursor: &mut (usize, usize)) {
    let text = std::mem::take(&mut lines[cursor.0]);
    let offset = char_offset(&text, cursor.1);
    let tail = text[offset..].to_string();
    lines[cursor.0] = text[..offset].to_string();
    lines.insert(cursor.0 + 1, tail);
    *cursor = (cursor.0 + 1, 0);
}

fn backspace(lines: &mut Vec<String>, cursor: &mut (usize, usize)) {
    let (line, column) = *cursor;
    if column > 0 {
        let text = &mut lines[line];
        let end = char_offset(text, column);
        let start = char_offset(text, column - 1);
        text.replace_range(start..end, "");
        *cursor = (line, column - 1);
    } else if line > 0 {
        let tail = lines.remove(line);
        let previous = &mut lines[line - 1];
        let merged_column = previous.chars().count();
        previous.push_str(&tail);
        *cursor = (line - 1, merged_column);
    }
}

fn delete_forward(lines: &mut Vec<String>, cursor: &mut (usize, usize)) {
    let (line, column) = *cursor;
    let width = lines[line].chars().count();
    if column < width {
        let text = &mut lines[line];
        let start = char_offset(text, column);
        let end = char_offset(text, column + 1);
        text.replace_range(start..end, "");
    } else if line + 1 < lines.len() {
        let tail = lines.remove(line + 1);
        lines[line].push_str(&tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conflict::parse;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ONE_CONFLICT_BASIC: &str = "<<<<<<< ours\n\
         local one\n\
         local two\n\
         =======\n\
         remote one\n\
         remote two\n\
         >>>>>>> theirs\n";

    const TWO_CONFLICTS_WITH_TEXT: &str = "before line\n\
         <<<<<<< ours\n\
         local a\n\
         =======\n\
         remote a\n\
         >>>>>>> theirs\n\
         between line\n\
         <<<<<<< ours\n\
         local b\n\
         =======\n\
         remote b\n\
         >>>>>>> theirs\n\
         after line\n";

    /// The line a user types into the current panel that must never be saved.
    const TYPED_MARKER_LINE: &str = ">>>>>>> typed-away\n";

    /// `udiff-test-<pid>-<counter>`, removed by the test that wrote it.
    fn temp_path() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("udiff-test-{}-{sequence}", std::process::id()))
    }

    fn app_over(text: &str) -> App {
        App::new(
            PathBuf::from("conflicted.txt"),
            parse(text).expect("fixture must parse"),
            Some("feature/x".to_string()),
        )
    }

    fn current_of(app: &App, conflict: usize) -> String {
        app.file
            .regions
            .iter()
            .filter_map(|region| match region {
                Region::Conflict(inner) => Some(inner),
                Region::Text(_) => None,
            })
            .nth(conflict)
            .expect("conflict exists")
            .current_text
            .clone()
    }

    #[test]
    fn starts_on_first_conflict() {
        let app = app_over(ONE_CONFLICT_BASIC);
        assert_eq!(app.selected, 0);
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.dirty);
        assert!(app.message.is_none());
        assert_eq!(
            app.cursor,
            (4, 0),
            "cursor starts at the end of the current text"
        );
        assert_eq!(
            app.selected_conflict().current_text,
            "remote one\nremote two\nlocal one\nlocal two\n"
        );
    }

    #[test]
    fn next_stops_at_last_prev_stops_at_first() {
        let mut app = app_over(TWO_CONFLICTS_WITH_TEXT);
        app.dispatch(Action::NextConflict);
        assert_eq!(app.selected, 1);
        app.dispatch(Action::NextConflict);
        assert_eq!(app.selected, 1, "no wrap past the last conflict");
        app.dispatch(Action::PrevConflict);
        assert_eq!(app.selected, 0);
        app.dispatch(Action::PrevConflict);
        assert_eq!(app.selected, 0, "no wrap before the first conflict");
    }

    #[test]
    fn take_remote_sets_current() {
        let mut app = app_over(ONE_CONFLICT_BASIC);
        app.dispatch(Action::TakeRemote);
        assert_eq!(current_of(&app, 0), "remote one\nremote two\n");
    }

    #[test]
    fn take_local_sets_current() {
        let mut app = app_over(ONE_CONFLICT_BASIC);
        app.dispatch(Action::TakeLocal);
        assert_eq!(current_of(&app, 0), "local one\nlocal two\n");
    }

    #[test]
    fn take_both_concatenates_remote_then_local() {
        let mut app = app_over(ONE_CONFLICT_BASIC);
        app.dispatch(Action::TakeBoth);
        assert_eq!(
            current_of(&app, 0),
            "remote one\nremote two\nlocal one\nlocal two\n"
        );
    }

    #[test]
    fn edit_changes_only_selected_conflict() {
        let mut app = app_over(TWO_CONFLICTS_WITH_TEXT);
        let second_before = current_of(&app, 1);
        app.dispatch(Action::EnterEdit);
        app.dispatch(Action::InsertChar('X'));
        let edited = current_of(&app, 0);
        assert!(
            edited.ends_with("X"),
            "the character was inserted: {edited:?}"
        );
        assert_eq!(
            current_of(&app, 1),
            second_before,
            "the other conflict is untouched"
        );
        assert_eq!(
            app.file.regions[0],
            Region::Text("before line\n".to_string())
        );
        assert_eq!(
            app.file.regions[2],
            Region::Text("between line\n".to_string())
        );
        assert_eq!(
            app.file.regions[4],
            Region::Text("after line\n".to_string())
        );
    }

    #[test]
    fn typed_marker_line_is_rejected_on_save() {
        let mut app = app_over(ONE_CONFLICT_BASIC);
        app.dispatch(Action::EnterEdit);
        for character in TYPED_MARKER_LINE.chars() {
            if character == '\n' {
                app.dispatch(Action::InsertNewline);
            } else {
                app.dispatch(Action::InsertChar(character));
            }
        }
        let outcome = app.dispatch(Action::Save);
        assert_eq!(
            outcome,
            Outcome::Continue,
            "the write must not be requested"
        );
        match &app.message {
            Some(Message::Error(text)) => assert!(text.contains("marker"), "got: {text}"),
            other => panic!("expected an error message, got {other:?}"),
        }
    }

    #[test]
    fn save_text_equals_original_when_untouched() {
        let mut app = app_over(TWO_CONFLICTS_WITH_TEXT);
        let expected = "before line\n\
             remote a\n\
             local a\n\
             between line\n\
             remote b\n\
             local b\n\
             after line\n";
        let outcome = app.dispatch(Action::Save);
        let text = match outcome {
            Outcome::SaveRequested { text } => text,
            other => panic!("expected SaveRequested, got {other:?}"),
        };
        assert_eq!(
            text, expected,
            "untouched save keeps every text run byte-exact"
        );
        // The shell writes exactly this string; prove the bytes over a real file.
        let path = temp_path();
        fs::write(&path, &text).expect("write the saved text");
        assert_eq!(fs::read(&path).expect("read it back"), expected.as_bytes());
        fs::remove_file(&path).expect("remove the temp file");
    }

    #[test]
    fn dirty_flag_set_on_first_edit() {
        let mut app = app_over(ONE_CONFLICT_BASIC);
        assert!(!app.dirty);
        app.dispatch(Action::EnterEdit);
        assert!(!app.dirty, "entering edit mode mutates nothing");
        app.dispatch(Action::InsertChar('a'));
        assert!(app.dirty, "the first mutation sets the flag");
        app.dispatch(Action::ExitEdit);
        assert!(app.dirty, "the flag never resets");
    }

    #[test]
    fn quit_without_save_produces_no_write() {
        let path = temp_path();
        fs::write(&path, TWO_CONFLICTS_WITH_TEXT).expect("seed the file");
        let on_disk = fs::read(&path).expect("read the seeded bytes");
        let mut app = App::new(
            path.clone(),
            parse(TWO_CONFLICTS_WITH_TEXT).expect("fixture must parse"),
            None,
        );
        app.dispatch(Action::EnterEdit);
        app.dispatch(Action::InsertChar('q'));
        let outcome = app.dispatch(Action::Quit);
        match outcome {
            Outcome::Quit { saved } => assert!(!saved, "dirty session, nothing saved"),
            other => panic!("expected Quit, got {other:?}"),
        }
        // The shell writes only on SaveRequested; this session produced none.
        assert_eq!(
            fs::read(&path).expect("read it back"),
            on_disk,
            "the file is byte-identical"
        );
        fs::remove_file(&path).expect("remove the temp file");
    }
}
