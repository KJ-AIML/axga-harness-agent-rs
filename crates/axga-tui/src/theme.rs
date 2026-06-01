//! Theme system — semantic color tokens for dark/light modes.
//!
//! Pattern from kimi-code `tui/theme/colors.ts`.

use ratatui::style::Color;

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
        primary: Color::Rgb(21, 101, 192),        // #1565C0
        accent: Color::Rgb(0, 131, 143),          // #00838F
        text: Color::Rgb(26, 26, 26),             // #1A1A1A
        text_dim: Color::Rgb(97, 97, 97),         // #616161
        text_muted: Color::Rgb(158, 158, 158),    // #9E9E9E
        surface: Color::Rgb(250, 250, 250),       // #FAFAFA
        border: Color::Rgb(189, 189, 189),        // #BDBDBD
        border_focus: Color::Rgb(21, 101, 192),   // #1565C0
        success: Color::Rgb(46, 125, 50),         // #2E7D32
        warning: Color::Rgb(245, 124, 0),         // #F57C00
        error: Color::Rgb(198, 40, 40),           // #C62828
        diff_added: Color::Rgb(27, 94, 32),       // #1B5E20
        diff_removed: Color::Rgb(183, 28, 28),    // #B71C1C
        role_user: Color::Rgb(230, 81, 0),        // #E65100
        role_assistant: Color::Rgb(21, 101, 192), // #1565C0
        role_tool: Color::Rgb(245, 124, 0),       // #F57C00
        role_thinking: Color::Rgb(106, 27, 154),  // #6A1B9A
        bg: Color::Rgb(255, 255, 255),            // #FFFFFF
        status_bar_bg: Color::Rgb(236, 239, 241), // #ECEFF1
        status_bar_fg: Color::Rgb(55, 71, 79),    // #37474F
    }
}

/// Spinner frames for loading/streaming indicator.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
