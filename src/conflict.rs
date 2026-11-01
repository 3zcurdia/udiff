//! Marker grammar and parsing for a conflicted file.
//!
//! Pure: no I/O, no terminal, no subprocess. The file's own marker blocks are
//! the single source of truth for both sides of every conflict.

/// One parsed conflicted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictFile {
    /// Regions in document order; text runs and conflicts, never overlapping.
    pub regions: Vec<Region>,
    /// Whether the original text ended with a line terminator.
    pub ends_with_newline: bool,
}

/// One region of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// A byte-exact run of text outside any conflict. Read-only forever.
    Text(String),
    /// One marker block.
    Conflict(Conflict),
}

/// One conflict marker block: both sides plus the resolution being built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The `>>>>>>>` label, verbatim.
    pub remote_label: String,
    /// The side between `=======` and `>>>>>>>`, lines with their terminators.
    pub remote_text: String,
    /// The `<<<<<<<` label, verbatim.
    pub local_label: String,
    /// The side between `<<<<<<<` and `=======`, lines with their terminators.
    pub local_text: String,
    /// The resolution; starts as `local_text + remote_text` (local, then
    /// remote) so editing has both sides to start from. Mutated only by `App`
    /// actions.
    pub current_text: String,
    /// False until the user takes a side or edits the resolution; save is
    /// refused while any conflict is unresolved.
    pub resolved: bool,
    /// The terminator of the `<<<<<<<` line (`"\r\n"` or `"\n"`); edits join
    /// lines with it.
    pub line_ending: &'static str,
}

/// Why a file could not be parsed. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub kind: ParseErrorKind,
}

/// The ways a marker block can be malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    MissingSeparator,
    MissingClose,
    MissingOpen,
    NestedOpen,
    MisplacedBase,
}

/// Splits a conflicted file into text runs and conflicts. All-or-nothing: on
/// failure the error names the 1-based line and what went wrong.
pub fn parse(text: &str) -> Result<ConflictFile, ParseError> {
    let mut regions: Vec<Region> = Vec::new();
    let mut text_run = String::new();
    let mut open: Option<OpenConflict> = None;
    let mut open_line = 0usize;

    for (index, raw_line) in text.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let content = strip_terminator(raw_line);
        let Some(conflict) = open.as_mut() else {
            // Outside a conflict only an open marker means anything; a stray
            // close is an error, and separator or base lines are ordinary
            // text (a Markdown `=======` underline, for one).
            match classify(content) {
                Some(Marker::Open(label)) => {
                    if !text_run.is_empty() {
                        regions.push(Region::Text(std::mem::take(&mut text_run)));
                    }
                    open = Some(OpenConflict {
                        line_ending: if raw_line.ends_with("\r\n") {
                            "\r\n"
                        } else {
                            "\n"
                        },
                        local_label: label.to_string(),
                        local_text: String::new(),
                        remote_text: String::new(),
                        section: Section::Local,
                    });
                    open_line = line_number;
                }
                Some(Marker::Close(_)) => {
                    return Err(failure(line_number, ParseErrorKind::MissingOpen));
                }
                Some(Marker::Separator | Marker::Base) | None => text_run.push_str(raw_line),
            }
            continue;
        };
        match (classify(content), conflict.section) {
            (Some(Marker::Open(_)), _) => {
                return Err(failure(line_number, ParseErrorKind::NestedOpen));
            }
            (Some(Marker::Base), Section::Local) => conflict.section = Section::Base,
            (Some(Marker::Base), Section::Base | Section::Remote) => {
                return Err(failure(line_number, ParseErrorKind::MisplacedBase));
            }
            (Some(Marker::Separator), Section::Local | Section::Base) => {
                conflict.section = Section::Remote
            }
            (Some(Marker::Close(_)), Section::Local | Section::Base) => {
                return Err(failure(line_number, ParseErrorKind::MissingSeparator));
            }
            (Some(Marker::Close(label)), Section::Remote) => {
                let label = label.to_string();
                let conflict = open.take().expect("matched an open conflict");
                let current_text = format!("{}{}", conflict.local_text, conflict.remote_text);
                regions.push(Region::Conflict(Conflict {
                    remote_label: label,
                    remote_text: conflict.remote_text,
                    local_label: conflict.local_label,
                    local_text: conflict.local_text,
                    current_text,
                    resolved: false,
                    line_ending: conflict.line_ending,
                }));
            }
            // Base lines (diff3/zdiff3) have no panel in the fixed layout.
            (None, Section::Base) => {}
            (None, Section::Local) => conflict.local_text.push_str(raw_line),
            // Only the first `=======` separates the sides; a later one is
            // ordinary remote-side content (save-time validation rejects it).
            (None | Some(Marker::Separator), Section::Remote) => {
                conflict.remote_text.push_str(raw_line)
            }
        }
    }

    if open.is_some() {
        return Err(failure(open_line, ParseErrorKind::MissingClose));
    }
    if !text_run.is_empty() {
        regions.push(Region::Text(text_run));
    }
    Ok(ConflictFile {
        regions,
        ends_with_newline: text.ends_with('\n'),
    })
}

/// Concatenates the text runs and every conflict's `current_text`,
/// byte-exact. Marker lines of the original file are never part of it.
pub fn render_resolved(file: &ConflictFile) -> String {
    let mut rendered = String::new();
    for region in &file.regions {
        match region {
            Region::Text(text) => rendered.push_str(text),
            Region::Conflict(conflict) => rendered.push_str(&conflict.current_text),
        }
    }
    // The marker line the grammar consumed carried the file's last line
    // terminator; when the original ended without one, so does the result.
    if !file.ends_with_newline && rendered.ends_with('\n') {
        rendered.pop();
        if rendered.ends_with('\r') {
            rendered.pop();
        }
    }
    rendered
}

/// Number of conflict regions — the N in "conflict i of N".
pub fn count_conflicts(file: &ConflictFile) -> usize {
    file.regions
        .iter()
        .filter(|region| matches!(region, Region::Conflict(_)))
        .count()
}

/// Number of conflicts the user has not resolved yet.
pub fn count_unresolved(file: &ConflictFile) -> usize {
    file.regions
        .iter()
        .filter(|region| matches!(region, Region::Conflict(conflict) if !conflict.resolved))
        .count()
}

/// A conflict block that is still being read from the file.
struct OpenConflict {
    line_ending: &'static str,
    local_label: String,
    local_text: String,
    remote_text: String,
    section: Section,
}

/// Which part of an open conflict block the next line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Between `<<<<<<<` and `|||||||` (or `=======`).
    Local,
    /// Between `|||||||` and `=======` (diff3/zdiff3); dropped.
    Base,
    /// Between `=======` and `>>>>>>>`.
    Remote,
}

/// One recognised marker line.
enum Marker<'a> {
    Open(&'a str),
    Base,
    Separator,
    Close(&'a str),
}

const MARKER_LEN: usize = 7;

/// Classifies a line without its terminator: an open, base, close or
/// separating marker, or `None` when the line is ordinary content.
fn classify(content: &str) -> Option<Marker<'_>> {
    if let Some(label) = marker_label(content, b'<') {
        return Some(Marker::Open(label));
    }
    if let Some(label) = marker_label(content, b'>') {
        return Some(Marker::Close(label));
    }
    if marker_label(content, b'|').is_some() {
        return Some(Marker::Base);
    }
    if content == "=======" {
        return Some(Marker::Separator);
    }
    None
}

/// True when a line (without its terminator) is a git conflict marker.
pub fn is_marker_line(content: &str) -> bool {
    classify(content).is_some()
}

/// The label after exactly `MARKER_LEN` marker bytes followed by the end of
/// the line or a space — git's own marker shape. `None` otherwise.
fn marker_label(content: &str, marker: u8) -> Option<&str> {
    let bytes = content.as_bytes();
    if bytes.len() < MARKER_LEN || !bytes[..MARKER_LEN].iter().all(|&byte| byte == marker) {
        return None;
    }
    match bytes.get(MARKER_LEN) {
        None => Some(""),
        Some(b' ') => Some(content[MARKER_LEN + 1..].trim()),
        Some(_) => None,
    }
}

fn strip_terminator(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn failure(line: usize, kind: ParseErrorKind) -> ParseError {
    ParseError { line, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    const ZERO_CONFLICTS: &str = "just a plain file\nwith two lines\n";

    const MISSING_SEPARATOR: &str = "<<<<<<< ours\nlocal line\n>>>>>>> theirs\n";

    const MISSING_CLOSE: &str = "<<<<<<< ours\nlocal line\n=======\nremote line\n";

    const NESTED_OPEN: &str =
        "<<<<<<< ours\nlocal line\n<<<<<<< nested\n=======\nremote line\n>>>>>>> theirs\n";

    const CRLF_SIDES: &str =
        "<<<<<<< ours\r\nlocal line\r\n=======\r\nremote line\r\n>>>>>>> theirs\r\n";

    const NO_TRAILING_NEWLINE: &str =
        "<<<<<<< ours\nlocal line\n=======\nremote line\n>>>>>>> theirs";

    const MARKER_LABELS: &str =
        "<<<<<<< HEAD\nlocal line\n=======\nremote line\n>>>>>>> feature/x\n";

    const SETEXT_OUTSIDE: &str = "Title\n==========\n\
         <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n\
         Sub\n=======\n";

    const LOOSE_MARKERS_ARE_TEXT: &str = "<<<<<<<<<<\n<<<<<<<x\n>>>>>>>>\n\
         <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n";

    // Real git 2.56 output: base `a b c d e`; ours `c d` → `Y1 shared`,
    // theirs `c d` → `X1 shared`.
    const DIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\nshared\n||||||| 00c6642\nc\nd\n\
         =======\nX1\nshared\n>>>>>>> feature\ne\n";

    const ZDIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\n||||||| 00c6642\nc\nd\n\
         =======\nX1\n>>>>>>> feature\nshared\ne\n";

    const MISPLACED_BASE: &str = "<<<<<<< ours\nlocal\n=======\nremote\n\
         ||||||| base\n>>>>>>> theirs\n";

    fn only_conflict(file: &ConflictFile) -> &Conflict {
        assert_eq!(file.regions.len(), 1);
        match &file.regions[0] {
            Region::Conflict(conflict) => conflict,
            Region::Text(text) => panic!("expected a conflict, found text: {text:?}"),
        }
    }

    fn marker_like(line: &str) -> bool {
        ['<', '=', '>']
            .iter()
            .any(|&marker| line.chars().take_while(|&c| c == marker).count() >= 7)
    }

    #[test]
    fn parses_one_conflict_with_labels() {
        let file = parse(ONE_CONFLICT_BASIC).unwrap();
        let conflict = only_conflict(&file);
        assert_eq!(conflict.local_label, "ours");
        assert_eq!(conflict.remote_label, "theirs");
        assert_eq!(conflict.local_text, "local one\nlocal two\n");
        assert_eq!(conflict.remote_text, "remote one\nremote two\n");
        assert_eq!(
            conflict.current_text,
            "local one\nlocal two\nremote one\nremote two\n"
        );
        assert!(!conflict.resolved);
        assert_eq!(count_conflicts(&file), 1);
        assert!(file.ends_with_newline);
    }

    #[test]
    fn parses_two_conflicts_preserving_text_runs() {
        let file = parse(TWO_CONFLICTS_WITH_TEXT).unwrap();
        assert_eq!(count_conflicts(&file), 2);
        assert_eq!(file.regions.len(), 5);
        assert_eq!(file.regions[0], Region::Text("before line\n".to_string()));
        assert_eq!(file.regions[2], Region::Text("between line\n".to_string()));
        assert_eq!(file.regions[4], Region::Text("after line\n".to_string()));
        match (&file.regions[1], &file.regions[3]) {
            (Region::Conflict(first), Region::Conflict(second)) => {
                assert_eq!(first.remote_text, "remote a\n");
                assert_eq!(first.local_text, "local a\n");
                assert_eq!(second.remote_text, "remote b\n");
                assert_eq!(second.local_text, "local b\n");
            }
            _ => panic!("expected conflicts at positions 1 and 3"),
        }
    }

    #[test]
    fn zero_conflicts_returns_empty_file() {
        let file = parse(ZERO_CONFLICTS).unwrap();
        assert_eq!(count_conflicts(&file), 0);
        assert_eq!(file.regions, vec![Region::Text(ZERO_CONFLICTS.to_string())]);
    }

    #[test]
    fn rejects_missing_separator_with_line() {
        let error = parse(MISSING_SEPARATOR).unwrap_err();
        assert_eq!(error.line, 3);
        assert_eq!(error.kind, ParseErrorKind::MissingSeparator);
    }

    #[test]
    fn rejects_missing_close_with_line() {
        let error = parse(MISSING_CLOSE).unwrap_err();
        assert_eq!(error.line, 1);
        assert_eq!(error.kind, ParseErrorKind::MissingClose);
    }

    #[test]
    fn rejects_nested_open_with_line() {
        let error = parse(NESTED_OPEN).unwrap_err();
        assert_eq!(error.line, 3);
        assert_eq!(error.kind, ParseErrorKind::NestedOpen);
    }

    #[test]
    fn parses_crlf_sides() {
        let file = parse(CRLF_SIDES).unwrap();
        let conflict = only_conflict(&file);
        assert_eq!(conflict.local_text, "local line\r\n");
        assert_eq!(conflict.remote_text, "remote line\r\n");
        assert_eq!(conflict.current_text, "local line\r\nremote line\r\n");
        assert_eq!(conflict.local_label, "ours");
        assert!(file.ends_with_newline);
        assert_eq!(render_resolved(&file), "local line\r\nremote line\r\n");
    }

    #[test]
    fn preserves_missing_trailing_newline() {
        let file = parse(NO_TRAILING_NEWLINE).unwrap();
        assert!(!file.ends_with_newline);
        let rendered = render_resolved(&file);
        assert_eq!(rendered, "local line\nremote line");
        assert!(!rendered.ends_with('\n'));
    }

    #[test]
    fn marker_labels_carry_branch_names() {
        let file = parse(MARKER_LABELS).unwrap();
        let conflict = only_conflict(&file);
        assert_eq!(conflict.local_label, "HEAD");
        assert_eq!(conflict.remote_label, "feature/x");
    }

    #[test]
    fn render_resolved_writes_no_marker_lines() {
        let file = parse(TWO_CONFLICTS_WITH_TEXT).unwrap();
        let rendered = render_resolved(&file);
        for line in rendered.split('\n') {
            assert!(!marker_like(line), "marker line survived: {line:?}");
        }
    }

    #[test]
    fn render_resolved_keeps_text_runs_byte_exact() {
        let file = parse(TWO_CONFLICTS_WITH_TEXT).unwrap();
        assert_eq!(
            render_resolved(&file),
            "before line\n\
             local a\n\
             remote a\n\
             between line\n\
             local b\n\
             remote b\n\
             after line\n"
        );
    }

    #[test]
    fn setext_underlines_outside_conflicts_are_text() {
        let file = parse(SETEXT_OUTSIDE).unwrap();
        assert_eq!(count_conflicts(&file), 1);
        assert_eq!(
            file.regions[0],
            Region::Text("Title\n==========\n".to_string())
        );
        assert_eq!(file.regions[2], Region::Text("Sub\n=======\n".to_string()));
    }

    #[test]
    fn loose_marker_runs_are_text() {
        let file = parse(LOOSE_MARKERS_ARE_TEXT).unwrap();
        assert_eq!(count_conflicts(&file), 1);
        assert_eq!(
            file.regions[0],
            Region::Text("<<<<<<<<<<\n<<<<<<<x\n>>>>>>>>\n".to_string())
        );
    }

    #[test]
    fn is_marker_line_matches_git_shapes() {
        for line in [
            "<<<<<<< HEAD",
            "<<<<<<<",
            "=======",
            "||||||| base",
            ">>>>>>> feature/x",
        ] {
            assert!(is_marker_line(line), "{line:?} is a marker");
        }
        for line in ["==========", "<<<<<<<x", "======= ", "<<<<<<", ""] {
            assert!(!is_marker_line(line), "{line:?} is not a marker");
        }
    }

    fn conflict_at(file: &ConflictFile, index: usize) -> &Conflict {
        match &file.regions[index] {
            Region::Conflict(conflict) => conflict,
            Region::Text(text) => panic!("expected a conflict, found text: {text:?}"),
        }
    }

    #[test]
    fn parses_diff3_dropping_base() {
        let file = parse(DIFF3).unwrap();
        assert_eq!(file.regions.len(), 3);
        assert_eq!(file.regions[0], Region::Text("a\nb\n".to_string()));
        let conflict = conflict_at(&file, 1);
        assert_eq!(conflict.local_text, "Y1\nshared\n");
        assert_eq!(conflict.remote_text, "X1\nshared\n");
        assert_eq!(conflict.local_label, "HEAD");
        assert_eq!(conflict.remote_label, "feature");
        assert_eq!(file.regions[2], Region::Text("e\n".to_string()));
    }

    #[test]
    fn parses_zdiff3_dropping_base() {
        let file = parse(ZDIFF3).unwrap();
        assert_eq!(file.regions.len(), 3);
        let conflict = conflict_at(&file, 1);
        assert_eq!(conflict.local_text, "Y1\n");
        assert_eq!(conflict.remote_text, "X1\n");
        assert_eq!(file.regions[2], Region::Text("shared\ne\n".to_string()));
    }

    #[test]
    fn diff3_render_has_no_markers() {
        for fixture in [DIFF3, ZDIFF3] {
            let rendered = render_resolved(&parse(fixture).unwrap());
            for line in rendered.split('\n') {
                assert!(!marker_like(line), "marker line survived: {line:?}");
            }
        }
    }

    #[test]
    fn rejects_misplaced_base_with_line() {
        let error = parse(MISPLACED_BASE).unwrap_err();
        assert_eq!(error.line, 5);
        assert_eq!(error.kind, ParseErrorKind::MisplacedBase);
    }

    #[test]
    fn line_ending_comes_from_open_marker() {
        assert_eq!(
            only_conflict(&parse(CRLF_SIDES).unwrap()).line_ending,
            "\r\n"
        );
        assert_eq!(
            only_conflict(&parse(ONE_CONFLICT_BASIC).unwrap()).line_ending,
            "\n"
        );
    }
}
