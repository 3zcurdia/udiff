//! Theme roles — the only place a color value may be declared.
//!
//! Widgets never name a raw color: they ask a `Theme` for a `Role`.

pub mod omarchy;

use ratatui::style::Color;

/// The nine color roles every widget consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Border,
    BorderActive,
    LabelRemote,
    LabelLocal,
    LabelCurrent,
    Selection,
    Status,
    StatusModified,
    StatusError,
}

/// The palette in effect for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    border: Color,
    border_active: Color,
    label_remote: Color,
    label_local: Color,
    label_current: Color,
    selection: Color,
    status: Color,
    status_modified: Color,
    status_error: Color,
}

impl Theme {
    /// The color of one role. This is how widgets read a color — never a
    /// field, never a literal.
    pub fn get(&self, role: Role) -> Color {
        match role {
            Role::Border => self.border,
            Role::BorderActive => self.border_active,
            Role::LabelRemote => self.label_remote,
            Role::LabelLocal => self.label_local,
            Role::LabelCurrent => self.label_current,
            Role::Selection => self.selection,
            Role::Status => self.status,
            Role::StatusModified => self.status_modified,
            Role::StatusError => self.status_error,
        }
    }
}

/// The built-in fallback palette: `Color::Reset` and named colors only.
pub fn default_theme() -> Theme {
    Theme {
        border: Color::Reset,
        border_active: Color::White,
        label_remote: Color::Cyan,
        label_local: Color::Green,
        label_current: Color::White,
        selection: Color::DarkGray,
        status: Color::Gray,
        status_modified: Color::Yellow,
        status_error: Color::Red,
    }
}

/// Omarchy palette → roles: accent → border_active/selection, foreground →
/// status/labels, red → status_error, muted → border, cyan → label_remote.
/// A role within `CONTRAST_DISTANCE` of the theme background is lightened by
/// `LIGHTEN_AMOUNT` so it stays readable.
pub fn map_palette(palette: &omarchy::Palette) -> Theme {
    let mut theme = default_theme();
    if let Some(color) = palette.accent {
        theme.border_active = color;
        theme.selection = color;
    }
    if let Some(color) = palette.muted {
        theme.border = color;
    }
    if let Some(color) = palette.foreground {
        theme.status = color;
        theme.label_local = color;
        theme.label_current = color;
    }
    if let Some(color) = palette.red {
        theme.status_error = color;
    }
    if let Some(color) = palette.cyan {
        theme.label_remote = color;
    }
    if let Some(background) = palette.background {
        let readable = |color| {
            if distance(color, background) < CONTRAST_DISTANCE {
                lighten(color, LIGHTEN_AMOUNT)
            } else {
                color
            }
        };
        theme.border = readable(theme.border);
        theme.border_active = readable(theme.border_active);
        theme.label_remote = readable(theme.label_remote);
        theme.label_local = readable(theme.label_local);
        theme.label_current = readable(theme.label_current);
        theme.selection = readable(theme.selection);
        theme.status = readable(theme.status);
        theme.status_modified = readable(theme.status_modified);
        theme.status_error = readable(theme.status_error);
    }
    theme
}

/// `"#RRGGBB"` (quotes optional) → `Color::Rgb`, anything else → `None`.
fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(red, green, blue))
}

/// Euclidean distance in RGB space; colors that are not RGB count as the
/// neutral gray midpoint.
fn distance(a: Color, b: Color) -> f32 {
    let (ar, ag, ab) = rgb_components(a);
    let (br, bg, bb) = rgb_components(b);
    let dr = f32::from(ar) - f32::from(br);
    let dg = f32::from(ag) - f32::from(bg);
    let db = f32::from(ab) - f32::from(bb);
    (dr * dr + dg * dg + db * db).sqrt()
}

/// Moves a color toward white by `amount` (0.0 = unchanged, 1.0 = white).
fn lighten(color: Color, amount: f32) -> Color {
    match color {
        Color::Rgb(red, green, blue) => {
            Color::Rgb(lift(red, amount), lift(green, amount), lift(blue, amount))
        }
        other => other,
    }
}

/// A role within this RGB distance of the background is too close to read.
const CONTRAST_DISTANCE: f32 = 40.0;
/// How far a guarded role is lifted toward white.
const LIGHTEN_AMOUNT: f32 = 0.35;

fn lift(component: u8, amount: f32) -> u8 {
    (f32::from(component) + (255.0 - f32::from(component)) * amount)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// RGB value of any color for distance math; `Reset` and terminal indexes
/// count as the neutral gray midpoint.
fn rgb_components(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (127, 127, 127),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        _ => (128, 128, 128),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_resolves_every_role() {
        let theme = default_theme();
        let expected = [
            (Role::Border, Color::Reset),
            (Role::BorderActive, Color::White),
            (Role::LabelRemote, Color::Cyan),
            (Role::LabelLocal, Color::Green),
            (Role::LabelCurrent, Color::White),
            (Role::Selection, Color::DarkGray),
            (Role::Status, Color::Gray),
            (Role::StatusModified, Color::Yellow),
            (Role::StatusError, Color::Red),
        ];
        for (role, color) in expected {
            assert_eq!(theme.get(role), color, "role {role:?}");
        }
    }

    #[test]
    fn map_palette_maps_roles_with_contrast_guard() {
        let palette = omarchy::Palette {
            accent: Some(Color::Rgb(94, 129, 172)),
            muted: Some(Color::Rgb(46, 52, 64)),
            background: Some(Color::Rgb(46, 52, 64)),
            foreground: Some(Color::Rgb(216, 222, 233)),
            red: Some(Color::Rgb(191, 97, 106)),
            cyan: Some(Color::Rgb(136, 192, 208)),
            color: [None; 16],
        };
        let theme = map_palette(&palette);
        assert_eq!(theme.get(Role::BorderActive), Color::Rgb(94, 129, 172));
        assert_eq!(theme.get(Role::Selection), Color::Rgb(94, 129, 172));
        assert_eq!(theme.get(Role::Status), Color::Rgb(216, 222, 233));
        assert_eq!(theme.get(Role::LabelLocal), Color::Rgb(216, 222, 233));
        assert_eq!(theme.get(Role::LabelCurrent), Color::Rgb(216, 222, 233));
        assert_eq!(theme.get(Role::LabelRemote), Color::Rgb(136, 192, 208));
        assert_eq!(theme.get(Role::StatusError), Color::Rgb(191, 97, 106));
        assert_eq!(
            theme.get(Role::Border),
            Color::Rgb(119, 123, 131),
            "muted equals the background, so the guard lightens it"
        );
    }
}
