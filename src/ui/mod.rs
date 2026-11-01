//! Rendering and key → `Action` translation.
//!
//! This module draws `App` state and translates input; it decides nothing.

pub mod theme;

use crate::app::{Action, App, Direction, Message, Mode};
use crate::conflict::{Region, count_conflicts, count_unresolved};
use crate::git::branch_label_display;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction as LayoutDirection, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use theme::{Role, Theme};

/// Below 60x12 cells the panels are dropped and the status line says why.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;
/// Context lines kept above the selected conflict.
const CONTEXT_LINES: usize = 2;
/// Tab stops every this many cells; the text model keeps real tabs.
const TAB_WIDTH: usize = 4;
/// Lines moved by PgUp / PgDn.
const PAGE_LINES: isize = 10;
/// Columns moved by ← / →.
const SCROLL_COLUMNS: isize = 8;

/// Draws the status line and the three panels from `App` state; decides nothing.
pub fn render(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = frame.area();
    let rows = Layout::new(
        LayoutDirection::Vertical,
        [Constraint::Length(1), Constraint::Min(0)],
    )
    .split(area);
    draw_status_line(frame, rows[0], app, theme);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return;
    }
    draw_panels(frame, area, app, theme);
}

/// The remote, local and current panel areas inside the whole frame, below
/// the status line.
fn panel_areas(area: Rect) -> [Rect; 3] {
    let body = Layout::new(
        LayoutDirection::Vertical,
        [Constraint::Length(1), Constraint::Min(0)],
    )
    .split(area)[1];
    let rows = Layout::new(
        LayoutDirection::Vertical,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .split(body);
    let top = Layout::new(
        LayoutDirection::Horizontal,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .split(rows[0]);
    [top[0], top[1], rows[1]]
}

/// `app.scroll` clamped to the range where at least one panel still moves,
/// so held keys never run into a dead zone past either end.
pub fn clamp_scroll(app: &App, area: Rect) -> (isize, usize) {
    let remote = panel_areas(area)[0];
    let rows = remote.height.saturating_sub(2) as usize;
    let cols = remote.width.saturating_sub(2) as usize;
    let (mut lowest, mut highest, mut widest) = (0isize, 0isize, 0usize);
    for panel in [Panel::Remote, Panel::Local, Panel::Current] {
        let document = panel_document(app, panel);
        let base = document.selected_start.saturating_sub(CONTEXT_LINES) as isize;
        let max_top = document.lines.len().saturating_sub(rows) as isize;
        lowest = lowest.min(-base);
        highest = highest.max(max_top - base);
        for line in &document.lines {
            widest = widest.max(Line::from(expand_tabs(line)).width());
        }
    }
    (
        app.scroll.0.clamp(lowest, highest),
        app.scroll.1.min(widest.saturating_sub(cols)),
    )
}

/// Top line of a panel: anchored at its conflict, shifted by `down`,
/// clamped so the panel never scrolls past either end.
fn panel_top(selected_start: usize, down: isize, len: usize, rows: usize) -> usize {
    let base = selected_start.saturating_sub(CONTEXT_LINES) as isize;
    let max_top = len.saturating_sub(rows) as isize;
    (base + down).clamp(0, max_top) as usize
}

/// Shifts a view start so that `pos` is visible in `size` cells.
fn follow(pos: usize, start: usize, size: usize) -> usize {
    if size == 0 || pos < start {
        pos
    } else if pos >= start + size {
        pos + 1 - size
    } else {
        start
    }
}

/// Pure key → `Action` mapping per the keymap. In Edit mode printable keys
/// become text; every command key is inert there.
pub fn translate_key(key: KeyEvent, mode: Mode) -> Action {
    if key.kind == KeyEventKind::Release
        || key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Action::Ignored;
    }
    match mode {
        Mode::Normal => match key.code {
            KeyCode::Char('n') => Action::NextConflict,
            KeyCode::Char('p') => Action::PrevConflict,
            KeyCode::Char('r') => Action::TakeRemote,
            KeyCode::Char('l') => Action::TakeLocal,
            KeyCode::Char('b') => Action::TakeBoth,
            KeyCode::Enter | KeyCode::Char('i') => Action::EnterEdit,
            KeyCode::Char('s') => Action::Save,
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Up => Action::Scroll { down: -1, right: 0 },
            KeyCode::Down => Action::Scroll { down: 1, right: 0 },
            KeyCode::PageUp => Action::Scroll {
                down: -PAGE_LINES,
                right: 0,
            },
            KeyCode::PageDown => Action::Scroll {
                down: PAGE_LINES,
                right: 0,
            },
            KeyCode::Left => Action::Scroll {
                down: 0,
                right: -SCROLL_COLUMNS,
            },
            KeyCode::Right => Action::Scroll {
                down: 0,
                right: SCROLL_COLUMNS,
            },
            KeyCode::Home => Action::ScrollReset,
            _ => Action::Ignored,
        },
        Mode::Edit => match key.code {
            KeyCode::Char(character) => Action::InsertChar(character),
            KeyCode::Tab => Action::InsertChar('\t'),
            KeyCode::Enter => Action::InsertNewline,
            KeyCode::Backspace => Action::Backspace,
            KeyCode::Delete => Action::Delete,
            KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Right => Action::MoveCursor(Direction::Right),
            KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Down => Action::MoveCursor(Direction::Down),
            KeyCode::Home => Action::MoveCursor(Direction::LineStart),
            KeyCode::End => Action::MoveCursor(Direction::LineEnd),
            KeyCode::Esc => Action::ExitEdit,
            _ => Action::Ignored,
        },
    }
}

fn draw_status_line(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let bounds = frame.area();
    let too_small = bounds.width < MIN_WIDTH || bounds.height < MIN_HEIGHT;
    let role = if too_small || matches!(app.message, Some(Message::Error(_))) {
        Role::StatusError
    } else if app.dirty {
        Role::StatusModified
    } else {
        Role::Status
    };
    let text = if too_small {
        "terminal too small (need 60x12)".to_string()
    } else {
        status_text(app)
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.get(role))),
        area,
    );
}

/// `conflict i of N · <remote> | <local> · modified` plus the transient message.
fn status_text(app: &App) -> String {
    let conflict = app.selected_conflict();
    let local = branch_label_display(&conflict.local_label, app.local_branch.as_deref());
    let mut text = format!(
        "conflict {} of {}",
        app.selected + 1,
        count_conflicts(&app.file)
    );
    let unresolved = count_unresolved(&app.file);
    if unresolved > 0 {
        text.push_str(&format!(" · {unresolved} unresolved"));
    }
    text.push_str(&format!(" · {} | {}", conflict.remote_label, local));
    if app.dirty {
        text.push_str(" · modified");
    }
    if let Some(message) = &app.message {
        text.push_str(" · ");
        text.push_str(match message {
            Message::Error(text) => text,
        });
    }
    text
}

fn draw_panels(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let [remote, local_area, current] = panel_areas(area);
    let conflict = app.selected_conflict();
    let local = branch_label_display(&conflict.local_label, app.local_branch.as_deref());
    draw_panel(
        frame,
        remote,
        app,
        theme,
        Panel::Remote,
        &format!("remote: {}", conflict.remote_label),
    );
    draw_panel(
        frame,
        local_area,
        app,
        theme,
        Panel::Local,
        &format!("local: {local}"),
    );
    draw_panel(frame, current, app, theme, Panel::Current, "current");
}

/// Which of the three panels is being drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Remote,
    Local,
    Current,
}

fn draw_panel(frame: &mut Frame, area: Rect, app: &App, theme: &Theme, panel: Panel, title: &str) {
    let (border_role, title_role, border_bold) = match panel {
        Panel::Remote => (Role::Border, Role::LabelRemote, false),
        Panel::Local => (Role::Border, Role::LabelLocal, false),
        Panel::Current => (Role::BorderActive, Role::LabelCurrent, true),
    };
    let mut border_style = Style::default().fg(theme.get(border_role));
    if border_bold {
        border_style = border_style.add_modifier(Modifier::BOLD);
    }
    let block = Block::bordered()
        .border_style(border_style)
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(theme.get(title_role)),
        ));

    let document = panel_document(app, panel);
    let rows = area.height.saturating_sub(2) as usize;
    let cols = area.width.saturating_sub(2) as usize;
    let mut top = panel_top(
        document.selected_start,
        app.scroll.0,
        document.lines.len(),
        rows,
    );
    let mut x = app.scroll.1;
    // While editing, the current panel follows the caret; the other two keep
    // the shared scroll.
    let caret = if panel == Panel::Current && app.mode == Mode::Edit {
        let cursor_line = document.selected_start + app.cursor.0;
        top = follow(cursor_line, top, rows);
        let text = document.lines.get(cursor_line).map_or("", String::as_str);
        let before: String = text.chars().take(app.cursor.1).collect();
        x = follow(Line::from(expand_tabs(&before)).width(), x, cols);
        Some(cursor_line)
    } else {
        None
    };

    let lines: Vec<Line> = document
        .lines
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let mut style = Style::default();
            let region = document.line_region.get(index).copied().flatten();
            if region == Some(document.selected_region) {
                style = style
                    .bg(theme.get(Role::Selection))
                    .add_modifier(Modifier::BOLD);
            }
            let unresolved = matches!(
                region.and_then(|region| app.file.regions.get(region)),
                Some(Region::Conflict(conflict)) if !conflict.resolved
            );
            if panel == Panel::Current && unresolved {
                style = style.add_modifier(Modifier::DIM | Modifier::ITALIC);
            }
            match caret {
                Some(cursor_line) if cursor_line == index => caret_line(
                    &expand_tabs(text),
                    expanded_column(text, app.cursor.1),
                    style,
                ),
                _ => Line::from(Span::styled(expand_tabs(text), style)),
            }
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((
            top.min(u16::MAX as usize) as u16,
            x.min(u16::MAX as usize) as u16,
        )),
        area,
    );
}

/// One panel's whole document: its lines, which region owns each line, and
/// where the selected conflict starts. Text runs are byte-identical in all
/// three panels; only the conflict sides differ.
struct PanelDocument {
    lines: Vec<String>,
    line_region: Vec<Option<usize>>,
    selected_region: usize,
    selected_start: usize,
}

fn panel_document(app: &App, panel: Panel) -> PanelDocument {
    let selected_region = region_of_conflict(app, app.selected);
    let mut document = String::new();
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    let mut newlines = 0usize;
    for (index, region) in app.file.regions.iter().enumerate() {
        let start = newlines;
        let piece: &str = match region {
            Region::Text(text) => text,
            Region::Conflict(conflict) => match panel {
                Panel::Remote => &conflict.remote_text,
                Panel::Local => &conflict.local_text,
                Panel::Current => &conflict.current_text,
            },
        };
        newlines += piece.matches('\n').count();
        let end = newlines + usize::from(!piece.is_empty() && !piece.ends_with('\n'));
        document.push_str(piece);
        spans.push((start, end, index));
    }

    let mut lines: Vec<String> = document
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    if document.ends_with('\n') {
        lines.pop();
    }
    let mut line_region: Vec<Option<usize>> = vec![None; lines.len()];
    let selected_start = spans_start(&spans, selected_region);
    for (start, end, index) in spans {
        let end = end.min(lines.len());
        for slot in line_region.iter_mut().take(end).skip(start) {
            *slot = Some(index);
        }
    }
    PanelDocument {
        lines,
        line_region,
        selected_region,
        selected_start,
    }
}

/// Region index of the `n`-th conflict, or 0 when there is none.
fn region_of_conflict(app: &App, conflict_number: usize) -> usize {
    let mut seen = 0;
    for (index, region) in app.file.regions.iter().enumerate() {
        if matches!(region, Region::Conflict(_)) {
            if seen == conflict_number {
                return index;
            }
            seen += 1;
        }
    }
    0
}

fn spans_start(spans: &[(usize, usize, usize)], region: usize) -> usize {
    spans
        .iter()
        .find(|(_, _, index)| *index == region)
        .map(|(start, _, _)| *start)
        .unwrap_or(0)
}

/// Tabs replaced by spaces up to the next multiple of `TAB_WIDTH`.
fn expand_tabs(text: &str) -> String {
    let mut expanded = String::with_capacity(text.len());
    let mut width = 0;
    for character in text.chars() {
        if character == '\t' {
            let spaces = TAB_WIDTH - width % TAB_WIDTH;
            expanded.extend(std::iter::repeat_n(' ', spaces));
            width += spaces;
        } else {
            expanded.push(character);
            width += 1;
        }
    }
    expanded
}

/// Number of chars of `expand_tabs(text)` that come before `text`'s
/// `column`-th char — where the caret sits in the expanded line.
fn expanded_column(text: &str, column: usize) -> usize {
    let before: String = text.chars().take(column).collect();
    expand_tabs(&before).chars().count()
}

/// The cursor line, split so the caret cell can be shown reversed.
fn caret_line(text: &str, column: usize, style: Style) -> Line<'static> {
    let caret = style.add_modifier(Modifier::REVERSED);
    let characters: Vec<char> = text.chars().collect();
    if column >= characters.len() {
        return Line::from(vec![
            Span::styled(text.to_string(), style),
            Span::styled(" ", caret),
        ]);
    }
    let before: String = characters[..column].iter().collect();
    let at = characters[column];
    let after: String = characters[column + 1..].iter().collect();
    Line::from(vec![
        Span::styled(before, style),
        Span::styled(at.to_string(), caret),
        Span::styled(after, style),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn tab_inserts_in_edit_mode() {
        assert_eq!(
            translate_key(key(KeyCode::Tab), Mode::Edit),
            Action::InsertChar('\t')
        );
    }

    #[test]
    fn tab_is_ignored_in_normal_mode() {
        assert_eq!(
            translate_key(key(KeyCode::Tab), Mode::Normal),
            Action::Ignored
        );
    }

    #[test]
    fn expand_tabs_cases() {
        assert_eq!(expand_tabs("\tx"), "    x");
        assert_eq!(expand_tabs("ab\tc"), "ab  c");
        assert_eq!(expand_tabs("abcd\t"), "abcd    ");
        assert_eq!(expand_tabs(""), "");
    }

    #[test]
    fn expanded_column_cases() {
        assert_eq!(expanded_column("\tx", 1), 4);
        assert_eq!(expanded_column("ab\tc", 3), 4);
        assert_eq!(expanded_column("abc", 2), 2);
    }

    #[test]
    fn home_end_map_in_edit_mode() {
        assert_eq!(
            translate_key(key(KeyCode::Home), Mode::Edit),
            Action::MoveCursor(Direction::LineStart)
        );
        assert_eq!(
            translate_key(key(KeyCode::End), Mode::Edit),
            Action::MoveCursor(Direction::LineEnd)
        );
    }

    #[test]
    fn panel_top_cases() {
        assert_eq!(panel_top(10, 0, 100, 20), 8);
        assert_eq!(panel_top(10, -50, 100, 20), 0);
        assert_eq!(panel_top(10, 500, 100, 20), 80);
        assert_eq!(panel_top(0, 0, 5, 20), 0);
    }

    #[test]
    fn follow_cases() {
        assert_eq!(follow(5, 0, 10), 0);
        assert_eq!(follow(12, 0, 10), 3);
        assert_eq!(follow(2, 5, 10), 2);
    }

    #[test]
    fn arrows_scroll_in_normal_mode() {
        assert_eq!(
            translate_key(key(KeyCode::PageDown), Mode::Normal),
            Action::Scroll {
                down: PAGE_LINES,
                right: 0
            }
        );
        assert_eq!(
            translate_key(key(KeyCode::Home), Mode::Normal),
            Action::ScrollReset
        );
    }

    #[test]
    fn clamp_scroll_saturates() {
        let remote: String = (0..30).map(|line| format!("remote {line}\n")).collect();
        let text = format!("<<<<<<< ours\nlocal\n=======\n{remote}>>>>>>> theirs\n");
        let mut app = App::new(
            std::path::PathBuf::from("conflicted.txt"),
            crate::conflict::parse(&text).expect("fixture must parse"),
            None,
        );
        let area = Rect::new(0, 0, 80, 24);
        app.scroll = (1000, 1000);
        let clamped = clamp_scroll(&app, area);
        assert!(clamped.0 < 1000 && clamped.0 > 0, "got {clamped:?}");
        assert_eq!(clamped.1, 0, "every line fits the panel width");
        app.scroll = clamped;
        assert_eq!(clamp_scroll(&app, area), clamped, "idempotent");
        app.scroll = (-1000, 0);
        assert_eq!(clamp_scroll(&app, area).0, 0, "nothing above the conflict");
    }
}
