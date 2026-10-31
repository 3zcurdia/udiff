//! OS-theme (Omarchy) follow — the only module that reads theme files.
//!
//! The active `colors.toml` is re-read only when its mtime changes; a missing
//! or unreadable theme falls back silently to the built-in palette.

use super::{Theme, map_palette, parse_color};
use ratatui::style::Color;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The flat colors of one Omarchy theme. Every key is optional; missing or
/// malformed values stay `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette {
    pub accent: Option<Color>,
    pub muted: Option<Color>,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub red: Option<Color>,
    pub cyan: Option<Color>,
    /// Legacy `color0`–`color15`.
    pub color: [Option<Color>; 16],
}

/// The theme directory to read: `UDIFF_OMARCHY_DIR` when set (it must be a
/// directory containing `colors.toml`, otherwise the built-in palette — the
/// override never falls through), else `~/.local/state/omarchy/current/theme`,
/// else `~/.config/omarchy/current/theme`. `None` means built-in.
pub fn theme_dir() -> Option<PathBuf> {
    match std::env::var_os("UDIFF_OMARCHY_DIR") {
        Some(override_dir) => {
            let directory = PathBuf::from(override_dir);
            contains_colors(&directory).then_some(directory)
        }
        None => {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            [
                ".local/state/omarchy/current/theme",
                ".config/omarchy/current/theme",
            ]
            .iter()
            .map(|relative| home.join(relative))
            .find(|directory| contains_colors(directory))
        }
    }
}

/// Tolerant flat `key = "#RRGGBB"` parse: named keys `accent`, `muted`,
/// `background`, `foreground`, `red`, `cyan` plus legacy `color0`–`color15`.
/// Unknown keys and comments are ignored; this never fails.
pub fn parse_colors_toml(text: &str) -> Palette {
    let mut palette = Palette::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches(|c| c == '"' || c == '\'');
        let color = parse_color(value);
        match key {
            "accent" => palette.accent = color,
            "muted" => palette.muted = color,
            "background" => palette.background = color,
            "foreground" => palette.foreground = color,
            "red" => palette.red = color,
            "cyan" => palette.cyan = color,
            _ => {
                if let Some(index) = key
                    .strip_prefix("color")
                    .and_then(|number| number.parse::<usize>().ok())
                    && let (Some(slot), Some(color)) = (palette.color.get_mut(index), color)
                {
                    *slot = Some(color);
                }
            }
        }
    }
    palette
}

/// Watches the active `colors.toml`: re-reads it only when its mtime (or the
/// theme directory) actually changed.
pub struct OmarchyWatch {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
}

impl OmarchyWatch {
    /// Observes the environment without loading yet; the first `check()` is
    /// the initial load.
    pub fn new() -> OmarchyWatch {
        OmarchyWatch {
            path: None,
            mtime: None,
        }
    }

    /// `Some(theme)` only when the palette actually changed since the last
    /// call (the first call is the initial load), `None` otherwise.
    pub fn check(&mut self) -> Option<Theme> {
        let path = theme_dir().map(|directory| directory.join("colors.toml"));
        let mtime = path
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok())
            .and_then(|metadata| metadata.modified().ok());
        if path == self.path && mtime == self.mtime {
            return None;
        }
        let theme = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| map_palette(&parse_colors_toml(&text)));
        self.path = path;
        self.mtime = mtime;
        theme
    }
}

fn contains_colors(directory: &Path) -> bool {
    directory.is_dir() && directory.join("colors.toml").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};
    use std::time::Duration;

    const OMARCHY_COLORS_TOML: &str = "# omarchy palette\n\
        accent = \"#5e81ac\"\n\
        muted = \"#4c566a\"\n\
        background = \"#2e3440\"\n\
        foreground = \"#d8dee9\"\n\
        red = \"#bf616a\"\n\
        cyan = \"#88c0d0\"\n";

    const OMARCHY_LEGACY_COLORS_TOML: &str = "color0 = \"#2e3440\"\n\
        color1 = \"#3b4252\"\n\
        color2 = \"#434c5e\"\n\
        color3 = \"#4c566a\"\n\
        color4 = \"#d8dee9\"\n\
        color5 = \"#e5e9f0\"\n\
        color6 = \"#eceff4\"\n\
        color7 = \"#8fbcbb\"\n\
        color8 = \"#88c0d0\"\n\
        color9 = \"#81a1c1\"\n\
        color10 = \"#5e81ac\"\n\
        color11 = \"#bf616a\"\n\
        color12 = \"#d08770\"\n\
        color13 = \"#ebcb8b\"\n\
        color14 = \"#a3be8c\"\n\
        color15 = \"#b48ead\"\n";

    /// The process environment is global; both env-dependent tests share it.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sets `UDIFF_OMARCHY_DIR` for the duration of a test, restoring the
    /// previous value (if any) on drop.
    struct EnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(directory: &Path) -> EnvGuard {
            let previous = std::env::var_os("UDIFF_OMARCHY_DIR");
            // SAFETY: the env lock guarantees no other test reads or writes
            // UDIFF_OMARCHY_DIR while this call runs.
            unsafe { std::env::set_var("UDIFF_OMARCHY_DIR", directory) };
            EnvGuard { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: same env lock as EnvGuard::set.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var("UDIFF_OMARCHY_DIR", value),
                    None => std::env::remove_var("UDIFF_OMARCHY_DIR"),
                }
            }
        }
    }

    /// `udiff-test-<pid>-omarchy-<counter>`, removed by the test that made it.
    fn temp_theme_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "udiff-test-{}-omarchy-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create the temp theme dir");
        directory
    }

    #[test]
    fn parse_colors_toml_reads_named_keys() {
        let palette = parse_colors_toml(OMARCHY_COLORS_TOML);
        assert_eq!(palette.accent, Some(Color::Rgb(94, 129, 172)));
        assert_eq!(palette.muted, Some(Color::Rgb(76, 86, 106)));
        assert_eq!(palette.background, Some(Color::Rgb(46, 52, 64)));
        assert_eq!(palette.foreground, Some(Color::Rgb(216, 222, 233)));
        assert_eq!(palette.red, Some(Color::Rgb(191, 97, 106)));
        assert_eq!(palette.cyan, Some(Color::Rgb(136, 192, 208)));
        assert_eq!(palette.color, [None; 16]);
    }

    #[test]
    fn parse_colors_toml_reads_legacy_color_keys() {
        let palette = parse_colors_toml(OMARCHY_LEGACY_COLORS_TOML);
        assert_eq!(palette.accent, None);
        assert_eq!(palette.color[0], Some(Color::Rgb(46, 52, 64)));
        assert_eq!(palette.color[8], Some(Color::Rgb(136, 192, 208)));
        assert_eq!(palette.color[15], Some(Color::Rgb(180, 142, 173)));
        assert_eq!(
            palette.color.iter().filter(|slot| slot.is_some()).count(),
            16,
            "all sixteen legacy keys are read"
        );
    }

    #[test]
    fn theme_dir_prefers_env_override() {
        let _lock = lock_env();
        let directory = temp_theme_dir();
        fs::write(directory.join("colors.toml"), OMARCHY_COLORS_TOML).expect("write colors.toml");
        let _guard = EnvGuard::set(&directory);
        assert_eq!(theme_dir(), Some(directory.clone()));
        drop(_guard);
        fs::remove_dir_all(&directory).expect("remove the temp theme dir");
    }

    #[test]
    fn check_returns_some_only_on_change() {
        let _lock = lock_env();
        let directory = temp_theme_dir();
        let colors = directory.join("colors.toml");
        fs::write(&colors, OMARCHY_COLORS_TOML).expect("write colors.toml");
        let _guard = EnvGuard::set(&directory);

        let mut watch = OmarchyWatch::new();
        let first = watch.check().expect("the first check loads the theme");
        assert_eq!(
            first.get(super::super::Role::BorderActive),
            Color::Rgb(94, 129, 172),
            "the named-key palette is applied"
        );
        assert!(watch.check().is_none(), "no change, no reload");

        fs::write(&colors, OMARCHY_LEGACY_COLORS_TOML).expect("rewrite colors.toml");
        let later = SystemTime::now() + Duration::from_secs(10);
        File::options()
            .write(true)
            .open(&colors)
            .and_then(|file| file.set_modified(later))
            .expect("bump the mtime");
        let second = watch.check().expect("an mtime change reloads the theme");
        assert_eq!(
            second.get(super::super::Role::BorderActive),
            Color::White,
            "the legacy-only palette falls back to the built-in roles"
        );
        assert!(watch.check().is_none(), "only a change returns Some");

        drop(_guard);
        fs::remove_dir_all(&directory).expect("remove the temp theme dir");
    }
}
