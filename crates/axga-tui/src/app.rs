//! TUI application state and render pipeline.
//!
//! Layout (kimi-code inspired):
//! ```text
//! ┌─ AXGA ────────────────────── model │ 1.2K tokens │ 5.8 MB ─┐
//! │                                                             │
//! │  ✦  user prompt here                                        │
//! │                                                             │
//! │  ●  assistant response with markdown                        │
//! │     continuation line                                       │
//! │                                                             │
//! │  ⚙  tool: read_file → /app/src/main.rs (1.2 KB)             │
//! │                                                             │
//! │  ●  final answer                                            │
//! │                                                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │  >  type your message...                          [INSERT]  │
//! ╰─────────────────────────────────────────────────────────────╯
//! ```

use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub struct App {
    pub input: String,
    pub chat_lines: Vec<ChatLine>,
    pub status: StatusLine,
    pub mode: InputMode,
    pub exit: bool,
    pub scroll_offset: u16,
    pub theme: Theme,
    pub spinner_idx: usize,
    pub is_streaming: bool,
    pub cursor_pos: usize,
}

#[derive(Debug, Clone)]
pub struct StatusLine {
    pub model: String,
    pub tokens_used: u32,
    pub memory_mb: f64,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Insert,
    Normal,
    Command,
}

#[derive(Debug, Clone)]
pub enum ChatLine {
    /// User input with orange bullet
    User(String),
    /// Assistant response with blue bullet
    Assistant(String),
    /// Tool call with amber marker
    Tool { name: String, detail: String },
    /// System/info message (dimmed)
    Info(String),
    /// Error message (red)
    Error(String),
    /// Thinking indicator
    Thinking(String),
    /// Spacer
    Spacer,
}

impl App {
    pub fn new(model: &str, theme: Theme) -> Self {
        Self {
            input: String::new(),
            chat_lines: Vec::new(),
            status: StatusLine {
                model: model.to_string(),
                tokens_used: 0,
                memory_mb: 0.0,
                git_branch: None,
            },
            mode: InputMode::Insert,
            exit: false,
            scroll_offset: 0,
            theme,
            spinner_idx: 0,
            is_streaming: false,
            cursor_pos: 0,
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let area = f.area();
        let gutter = 1; // 1-cell padding on left/right

        // Main vertical layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Status bar
                Constraint::Min(3),     // Chat area
                Constraint::Length(3),  // Input area
            ])
            .split(area);

        // ── Status Bar ──
        self.render_status(f, chunks[0]);

        // ── Chat Area ──
        self.render_chat(f, chunks[1], gutter);

        // ── Input Area ──
        self.render_input(f, chunks[2], gutter);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let mut parts: Vec<Span> = Vec::new();

        // Model name
        parts.push(Span::styled(
            format!(" {} ", self.status.model),
            Style::default()
                .fg(self.theme.primary)
                .add_modifier(Modifier::BOLD),
        ));

        // Separator
        parts.push(Span::styled("│", Style::default().fg(self.theme.text_muted)));

        // Token count
        parts.push(Span::styled(
            format!(" {} tokens ", self.status.tokens_used),
            Style::default().fg(self.theme.text_dim),
        ));

        // Memory
        if self.status.memory_mb > 0.0 {
            parts.push(Span::styled("│", Style::default().fg(self.theme.text_muted)));
            parts.push(Span::styled(
                format!(" {:.1} MB ", self.status.memory_mb),
                Style::default().fg(self.theme.text_dim),
            ));
        }

        // Git branch
        if let Some(ref branch) = self.status.git_branch {
            parts.push(Span::styled("│", Style::default().fg(self.theme.text_muted)));
            parts.push(Span::styled(
                format!(" git:{} ", branch),
                Style::default().fg(self.theme.success),
            ));
        }

        // Streaming indicator
        if self.is_streaming {
            let spinner = crate::theme::SPINNER_FRAMES[self.spinner_idx % crate::theme::SPINNER_FRAMES.len()];
            parts.push(Span::styled(
                format!(" {} ", spinner),
                Style::default().fg(self.theme.accent),
            ));
        }

        // Mode badge (right-aligned via space fill)
        let mode_str = match self.mode {
            InputMode::Insert => "INSERT",
            InputMode::Normal => "NORMAL",
            InputMode::Command => "CMD",
        };
        let mode_span = Span::styled(
            format!(" {} ", mode_str),
            Style::default()
                .fg(self.theme.status_bar_fg)
                .bg(match self.mode {
                    InputMode::Insert => self.theme.primary,
                    InputMode::Normal => self.theme.text_muted,
                    InputMode::Command => self.theme.warning,
                })
                .add_modifier(Modifier::BOLD),
        );

        let line = Line::from(parts);
        let status_bar = Paragraph::new(line)
            .style(Style::default().bg(self.theme.status_bar_bg));

        f.render_widget(status_bar, area);

        // Render mode badge manually at the right
        let mode_x = area.width.saturating_sub(mode_str.len() as u16 + 3);
        let mode_area = Rect {
            x: area.x + mode_x,
            y: area.y,
            width: mode_str.len() as u16 + 2,
            height: 1,
        };
        let mode_text = Text::from(mode_span);
        let mode_p = Paragraph::new(mode_text);
        f.render_widget(mode_p, mode_area);
    }

    fn render_chat(&self, f: &mut Frame, area: Rect, gutter: u16) {
        // Calculate visible area
        let visible_height = area.height.saturating_sub(2) as usize; // minus borders
        let total_lines = self.chat_lines.len();
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll = self.scroll_offset.min(max_scroll as u16) as usize;

        let mut lines: Vec<Line> = Vec::new();

        for chat_line in self.chat_lines.iter().skip(scroll).take(visible_height + 1) {
            match chat_line {
                ChatLine::User(text) => {
                    lines.push(Line::from(vec![
                        Span::styled(" ✦  ", Style::default().fg(self.theme.role_user).add_modifier(Modifier::BOLD)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text)),
                    ]));
                }
                ChatLine::Assistant(text) => {
                    lines.push(Line::from(vec![
                        Span::styled(" ●  ", Style::default().fg(self.theme.role_assistant).add_modifier(Modifier::BOLD)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text)),
                    ]));
                }
                ChatLine::Tool { name, detail } => {
                    lines.push(Line::from(vec![
                        Span::styled(" ⚙  ", Style::default().fg(self.theme.role_tool)),
                        Span::styled(name.as_str(), Style::default().fg(self.theme.role_tool).add_modifier(Modifier::BOLD)),
                        Span::styled(" → ", Style::default().fg(self.theme.text_muted)),
                        Span::styled(detail.as_str(), Style::default().fg(self.theme.text_dim)),
                    ]));
                }
                ChatLine::Info(text) => {
                    lines.push(Line::from(vec![
                        Span::styled("  ℹ  ", Style::default().fg(self.theme.text_muted)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text_dim)),
                    ]));
                }
                ChatLine::Error(text) => {
                    lines.push(Line::from(vec![
                        Span::styled(" ✗  ", Style::default().fg(self.theme.error).add_modifier(Modifier::BOLD)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.error)),
                    ]));
                }
                ChatLine::Thinking(text) => {
                    let spinner = crate::theme::SPINNER_FRAMES[self.spinner_idx % crate::theme::SPINNER_FRAMES.len()];
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {}  ", spinner), Style::default().fg(self.theme.role_thinking)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text_dim).add_modifier(Modifier::ITALIC)),
                    ]));
                }
                ChatLine::Spacer => {
                    lines.push(Line::from(""));
                }
            }
        }

        // Padding
        let padded_lines: Vec<Line> = lines
            .into_iter()
            .map(|l| {
                let mut spans = vec![Span::raw(" ".repeat(gutter as usize))];
                spans.extend(l.spans);
                Line::from(spans)
            })
            .collect();

        let chat = Paragraph::new(padded_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.theme.border))
                .title(Span::styled(" AXGA ", Style::default().fg(self.theme.primary).add_modifier(Modifier::BOLD))),
        );

        f.render_widget(chat, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect, _gutter: u16) {
        let _ = _gutter; // available for future padding adjustment
        let _inner_width = area.width.saturating_sub(4); // borders + gutter

        let prompt = Span::styled(" > ", Style::default().fg(self.theme.primary).add_modifier(Modifier::BOLD));

        let input_display = if self.input.is_empty() {
            Span::styled(
                "type your message...",
                Style::default().fg(self.theme.text_muted).add_modifier(Modifier::ITALIC),
            )
        } else {
            Span::styled(self.input.as_str(), Style::default().fg(self.theme.text))
        };

        let text = Text::from(Line::from(vec![
            prompt,
            input_display,
        ]));

        let input_widget = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(
                    if self.mode == InputMode::Insert {
                        self.theme.border_focus
                    } else {
                        self.theme.border
                    },
                ))
                .title(Span::styled(
                    " Input ",
                    Style::default().fg(self.theme.text_dim),
                )),
        );

        f.render_widget(input_widget, area);
    }
}
