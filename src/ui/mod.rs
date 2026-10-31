//! Rendering and key → `Action` translation.
//!
//! This module draws `App` state and translates input; it decides nothing.

pub mod theme;

use crate::app::{Action, App, Direction, Message, Mode};
use crate::conflict::{Region, count_conflicts};
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
    draw_panels(frame, rows[1], app, theme);
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
            _ => Action::Ignored,
        },
        Mode::Edit => match key.code {
            KeyCode::Char(character) => Action::InsertChar(character),
            KeyCode::Enter => Action::InsertNewline,
            KeyCode::Backspace => Action::Backspace,
            KeyCode::Delete => Action::Delete,
            KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Right => Action::MoveCursor(Direction::Right),
            KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Down => Action::MoveCursor(Direction::Down),
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
        "conflict {} of {} · {} | {}",
        app.selected + 1,
        count_conflicts(&app.file),
        conflict.remote_label,
        local
    );
    if app.dirty {
        text.push_str(" · modified");
    }
    if let Some(message) = &app.message {
        text.push_str(" · ");
        text.push_str(match message {
            Message::Info(text) | Message::Error(text) => text,
        });
    }
    text
}

fn draw_panels(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let rows = Layout::new(
        LayoutDirection::Vertical,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .split(area);
    let top = Layout::new(
        LayoutDirection::Horizontal,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .split(rows[0]);
    let conflict = app.selected_conflict();
    let local = branch_label_display(&conflict.local_label, app.local_branch.as_deref());
    draw_panel(
        frame,
        top[0],
        app,
        theme,
        Panel::Remote,
        &format!("remote: {}", conflict.remote_label),
    );
    draw_panel(
        frame,
        top[1],
        app,
        theme,
        Panel::Local,
        &format!("local: {local}"),
    );
    draw_panel(frame, rows[1], app, theme, Panel::Current, "current");
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
    let viewport = area.height.saturating_sub(2) as usize;
    let mut offset = document.selected_start.saturating_sub(CONTEXT_LINES);
    let end_of_file = document.lines.len().saturating_sub(viewport);
    offset = offset.min(end_of_file);
    let caret = if panel == Panel::Current && app.mode == Mode::Edit {
        let cursor_line = document.selected_start + app.cursor.0;
        if cursor_line >= offset + viewport && viewport > 0 {
            offset = cursor_line + 1 - viewport;
            offset = offset.min(end_of_file);
        }
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
            if document.line_region.get(index) == Some(&Some(document.selected_region)) {
                style = style
                    .bg(theme.get(Role::Selection))
                    .add_modifier(Modifier::BOLD);
            }
            match caret {
                Some(cursor_line) if cursor_line == index => caret_line(text, app.cursor.1, style),
                _ => Line::from(Span::styled(text.clone(), style)),
            }
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((offset.min(u16::MAX as usize) as u16, 0)),
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

    let mut lines: Vec<String> = document.split('\n').map(str::to_string).collect();
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
