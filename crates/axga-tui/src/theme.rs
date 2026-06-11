//! Theme system — semantic color tokens for dark/light modes.
//!
//! Pattern from kimi-code `tui/theme/colors.ts`.

use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::style::Color;

use crate::markdown::MarkdownTheme;

/// Global dark/light toggle.  `true` = dark (default), `false` = light.
static IS_DARK: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone)]
pub struct Theme {
    pub primary: Color,
    pub accent: Color,
    pub text: Color,
    pub text_dim: Color,
    pub text_muted: Color,
    pub surface: Color,
    pub border: Color,
    pub border_focus: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub diff_added: Color,
    pub diff_removed: Color,
    pub role_user: Color,
    pub role_assistant: Color,
    pub role_tool: Color,
    pub role_thinking: Color,
    pub bg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
}

// ── Palettes ────────────────────────────────────────────────────────────

pub fn dark_theme() -> Theme {
    Theme {
        primary: Color::Rgb(79, 168, 255),       // #4FA8FF blue
        accent: Color::Rgb(0, 217, 255),          // #00D9FF cyan
        text: Color::Rgb(245, 245, 245),          // #F5F5F5
        text_dim: Color::Rgb(158, 158, 158),      // #9E9E9E
        text_muted: Color::Rgb(97, 97, 97),       // #616161
        surface: Color::Rgb(30, 30, 30),          // #1E1E1E
        border: Color::Rgb(66, 66, 66),           // #424242
        border_focus: Color::Rgb(79, 168, 255),   // #4FA8FF
        success: Color::Rgb(76, 175, 80),         // #4CAF50
        warning: Color::Rgb(255, 193, 7),         // #FFC107 amber
        error: Color::Rgb(244, 67, 54),           // #F44336
        diff_added: Color::Rgb(46, 125, 50),      // #2E7D32
        diff_removed: Color::Rgb(198, 40, 40),    // #C62828
        role_user: Color::Rgb(255, 152, 0),       // #FF9800 orange
        role_assistant: Color::Rgb(79, 168, 255), // #4FA8FF
        role_tool: Color::Rgb(255, 193, 7),       // #FFC107 amber
        role_thinking: Color::Rgb(156, 39, 176),  // #9C27B0 purple
        bg: Color::Rgb(18, 18, 18),               // #121212
        status_bar_bg: Color::Rgb(38, 50, 56),    // #263238 blue-grey
        status_bar_fg: Color::Rgb(176, 190, 197), // #B0BEC5
    }
}

pub fn light_theme() -> Theme {
    Theme {
        primary: Color::Rgb(25, 118, 210),        // #1976D2 blue — bold on white
        accent: Color::Rgb(0, 137, 123),           // #00897B teal
        text: Color::Rgb(33, 33, 33),              // #212121 near-black
        text_dim: Color::Rgb(117, 117, 117),       // #757575 medium grey
        text_muted: Color::Rgb(158, 158, 158),     // #9E9E9E
        surface: Color::Rgb(250, 250, 250),        // #FAFAFA off-white
        border: Color::Rgb(189, 189, 189),         // #BDBDBD
        border_focus: Color::Rgb(25, 118, 210),    // #1976D2
        success: Color::Rgb(46, 125, 50),          // #2E7D32
        warning: Color::Rgb(245, 124, 0),          // #F57C00
        error: Color::Rgb(198, 40, 40),            // #C62828
        diff_added: Color::Rgb(27, 94, 32),        // #1B5E20
        diff_removed: Color::Rgb(183, 28, 28),     // #B71C1C
        role_user: Color::Rgb(230, 74, 25),        // #E64A19 deep-orange
        role_assistant: Color::Rgb(25, 118, 210),  // #1976D2
        role_tool: Color::Rgb(245, 124, 0),        // #F57C00
        role_thinking: Color::Rgb(106, 27, 154),   // #6A1B9A purple
        bg: Color::Rgb(255, 255, 255),             // #FFFFFF pure white
        status_bar_bg: Color::Rgb(236, 239, 241),  // #ECEFF1 blue-grey 50
        status_bar_fg: Color::Rgb(55, 71, 79),     // #37474F blue-grey 800
    }
}

// ── Runtime switching ───────────────────────────────────────────────────

/// Whether the dark palette is active.
pub fn is_dark() -> bool {
    IS_DARK.load(Ordering::Relaxed)
}

/// Toggle the global palette.
pub fn set_dark(dark: bool) {
    IS_DARK.store(dark, Ordering::Relaxed);
}

/// Return the palette that matches the current global setting.
///
/// Call this at each render — it always reads the atomic.
pub fn current_theme() -> Theme {
    if IS_DARK.load(Ordering::Relaxed) {
        dark_theme()
    } else {
        light_theme()
    }
}

/// Return a `MarkdownTheme` tuned for the current palette.
pub fn current_markdown_theme() -> MarkdownTheme {
    if IS_DARK.load(Ordering::Relaxed) {
        MarkdownTheme::default()
    } else {
        MarkdownTheme {
            text: Color::Rgb(33, 33, 33),
            code_bg: Color::Rgb(236, 239, 241),     // #ECEFF1
            code_fg: Color::Rgb(0, 105, 92),        // #00695C dark-teal
            heading: Color::Rgb(25, 118, 210),       // #1976D2
            bold: Color::Rgb(0, 0, 0),               // #000000
            link: Color::Rgb(25, 118, 210),           // #1976D2
            list_bullet: Color::Rgb(117, 117, 117),   // #757575
        }
    }
}

/// Spinner frames for loading/streaming indicator.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
