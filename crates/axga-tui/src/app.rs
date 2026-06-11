//! TUI application state and render pipeline.
//!
//! Layout (kimi-code inspired):
//! ```text
//! ┌─ AXGA ──────────────────── model │ 1.2K tokens ─┐
//! │                                                  │
//! │  ✦  user prompt                                  │
//! │  ●  assistant response with **markdown**         │
//! │     continuation line                            │
//! │  ⚙  tool → executed                              │
//! │                                                  │
//! ├──────────────────────────────────────────────────┤
//! │  >  type your message...                [INSERT] │
//! ╰──────────────────────────────────────────────────╯
//! ```

use crate::theme::Theme;
use crate::markdown::{self, MarkdownTheme};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListState, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

pub struct App {
    pub input: String,
    pub chat_lines: Vec<ChatLine>,
    pub status: StatusLine,
    pub mode: InputMode,
    pub exit: bool,
    pub theme: Theme,
    pub spinner_idx: usize,
    pub is_streaming: bool,
    pub cursor_pos: usize,
    pub pending_gg: bool,
    pub pending_prompt: PendingPrompt,
    markdown_theme: MarkdownTheme,
    list_state: ListState,
    scrollbar_state: ScrollbarState,
}

/// Rough estimate: average tokens per conversation message.
const CHARS_PER_TOKEN: u32 = 100;
/// Default max context window (32k tokens).
const MAX_CONTEXT_TOKENS: u32 = 32768;

#[derive(Debug, Clone)]
pub struct StatusLine {
    pub model: String,
    pub tokens_used: u32,
    pub memory_mb: f64,
    pub git_branch: Option<String>,
    pub cwd: String,
    pub context_pct: u32,
    pub permission_mode: String,
    pub background_tasks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Insert,
    Normal,
    Command,
}

/// Interactive wizard state for guided provider setup and tool approvals.
#[derive(Debug, Clone)]
pub enum PendingPrompt {
    None,
    /// Waiting for API key (provider_name, optional_preferred_model)
    ApiKey { provider: String, model: Option<String> },
    /// Waiting for model selection
    Model { provider: String },
    /// Waiting for tool approval (y=yes, n=no, a=approve all)
    ApprovalDialog { tool_name: String, tool_args: String, tool_id: String },
}

#[derive(Debug, Clone)]
pub enum ChatLine {
    User(String),
    Assistant(String),
    Tool { name: String, detail: String },
    Info(String),
    Error(String),
    Thinking(String),
    Diff(String),
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
                cwd: String::new(),
                context_pct: 0,
                permission_mode: String::from("MANUAL"),
                background_tasks: 0,
            },
            mode: InputMode::Insert,
            exit: false,
            theme,
            spinner_idx: 0,
            is_streaming: false,
            cursor_pos: 0,
            pending_gg: false,
            pending_prompt: PendingPrompt::None,
            markdown_theme: MarkdownTheme::default(),
            list_state: ListState::default(),
            scrollbar_state: ScrollbarState::default(),
        }
    }

    /// Scroll by a delta. Positive = down, negative = up.
    pub fn scroll_by(&mut self, delta: i32) {
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let new = (current + delta).max(0);
        self.list_state.select(Some(new as usize));
        self.scrollbar_state = self.scrollbar_state.position(new as usize);
        self.pending_gg = false;
    }

    /// Scroll to the top (earliest content).
    pub fn scroll_to_top(&mut self) {
        self.list_state.select(Some(0));
        self.scrollbar_state = self.scrollbar_state.position(0);
        self.pending_gg = false;
    }

    /// Scroll to the bottom (latest content).
    pub fn scroll_to_bottom(&mut self) {
        let total = self.chat_lines.len().saturating_sub(1);
        self.list_state.select(Some(total));
        self.scrollbar_state = self.scrollbar_state.position(total);
    }

    /// Get current scroll position.
    pub fn scroll_pos(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    /// Update context usage percentage from conversation length.
    pub fn update_context(&mut self, conversation_len: usize) {
        let estimated = conversation_len as u32 * CHARS_PER_TOKEN;
        self.status.context_pct = if MAX_CONTEXT_TOKENS > 0 {
            ((estimated as f64 / MAX_CONTEXT_TOKENS as f64) * 100.0).min(100.0) as u32
        } else {
            0
        };
    }

    pub fn render(&self, f: &mut Frame) {
        let area = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_status(f, chunks[0]);
        self.render_chat(f, chunks[1]);
        self.render_input(f, chunks[2]);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        self.render_footer_line1(f, chunks[0]);
        self.render_footer_line2(f, chunks[1]);
    }

    fn render_footer_line1(&self, f: &mut Frame, area: Rect) {
        let mut parts: Vec<Span> = Vec::new();

        // Permission badge
        let perm_str = &self.status.permission_mode;
        let perm_color = if perm_str == "AUTO" { self.theme.warning } else { self.theme.success };
        parts.push(Span::styled(
            format!("[{perm_str}]"),
            Style::default().fg(perm_color).add_modifier(Modifier::BOLD),
        ));
        parts.push(Span::styled(" ", Style::default()));

        // Model
        parts.push(Span::styled(
            self.status.model.as_str(),
            Style::default().fg(self.theme.primary).add_modifier(Modifier::BOLD),
        ));

        // CWD (last 3 segments)
        if !self.status.cwd.is_empty() {
            parts.push(Span::styled(" │ ", Style::default().fg(self.theme.text_muted)));
            let trimmed = trim_cwd(&self.status.cwd);
            parts.push(Span::styled(trimmed, Style::default().fg(self.theme.text_dim)));
        }

        // Git branch
        if let Some(ref branch) = self.status.git_branch {
            parts.push(Span::styled(" │ ", Style::default().fg(self.theme.text_muted)));
            parts.push(Span::styled(
                format!("⎇ {branch}"),
                Style::default().fg(self.theme.text_dim),
            ));
        }

        let line = Line::from(parts);
        let bg = Paragraph::new(line).style(Style::default().bg(self.theme.status_bar_bg));
        f.render_widget(bg, area);

        // Mode badge (right-aligned)
        let mode_str = match self.mode {
            InputMode::Insert => "INSERT",
            InputMode::Normal => "NORMAL",
            InputMode::Command => "CMD",
        };
        let mode_span = Span::styled(
            format!(" {mode_str} "),
            Style::default().fg(self.theme.status_bar_fg).bg(match self.mode {
                InputMode::Insert => self.theme.primary,
                InputMode::Normal => self.theme.text_muted,
                InputMode::Command => self.theme.warning,
            }).add_modifier(Modifier::BOLD),
        );
        let mode_x = area.width.saturating_sub(mode_str.len() as u16 + 3);
        let mode_area = Rect { x: area.x + mode_x, y: area.y, width: mode_str.len() as u16 + 2, height: 1 };
        f.render_widget(Paragraph::new(Text::from(mode_span)), mode_area);
    }

    fn render_footer_line2(&self, f: &mut Frame, area: Rect) {
        // Context usage: "context: 45% (1.2k/32k)"
        let estimated = if MAX_CONTEXT_TOKENS > 0 {
            format_tokens(self.status.context_pct as u64 * MAX_CONTEXT_TOKENS as u64 / 100)
        } else {
            String::from("0")
        };
        let max_display = format_tokens(MAX_CONTEXT_TOKENS as u64);

        let mut parts: Vec<Span> = Vec::new();

        // Spinner if streaming
        if self.is_streaming {
            let spinner = crate::theme::SPINNER_FRAMES[self.spinner_idx % crate::theme::SPINNER_FRAMES.len()];
            parts.push(Span::styled(
                format!("{spinner} "),
                Style::default().fg(self.theme.accent),
            ));
        }

        let context_str = format!("context: {}% ({}/{})", self.status.context_pct, estimated, max_display);
        parts.push(Span::styled(context_str, Style::default().fg(self.theme.text_dim)));

        let line = Line::from(parts);
        // Right-align the context info
        let line_width = line.width() as u16;
        let x = area.x + area.width.saturating_sub(line_width);
        let right_area = Rect { x, y: area.y, width: line_width.min(area.width), height: 1 };
        f.render_widget(
            Paragraph::new(line).style(Style::default().bg(self.theme.status_bar_bg)),
            right_area,
        );
    }
}

/// Trim a path to its last 3 segments for compact display.
fn trim_cwd(path: &str) -> String {
    let sep = if path.contains('\\') { '\\' } else { '/' };
    let segments: Vec<&str> = path.split(sep).filter(|s| !s.is_empty()).collect();
    if segments.len() <= 3 {
        path.to_string()
    } else {
        let last3 = &segments[segments.len() - 3..];
        format!(".../{}/{}/{}", last3[0], last3[1], last3[2])
    }
}

/// Format a token count with human-readable suffix (k).
fn format_tokens(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

impl App {

    fn render_chat(&self, f: &mut Frame, area: Rect) {
        let pad = "  ";

        // Build list items from chat lines
        let items: Vec<ListItem> = self.chat_lines.iter().map(|chat_line| {
            match chat_line {
                ChatLine::User(text) => {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{pad}✦  "), Style::default().fg(self.theme.role_user).add_modifier(Modifier::BOLD)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text)),
                    ]))
                }
                ChatLine::Assistant(text) => {
                    let md_text = markdown::render_markdown(text, &self.markdown_theme);
                    ListItem::new(md_text).style(Style::default())
                }
                ChatLine::Tool { name, detail } => {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{pad}⚙  "), Style::default().fg(self.theme.role_tool)),
                        Span::styled(name.as_str(), Style::default().fg(self.theme.role_tool).add_modifier(Modifier::BOLD)),
                        Span::styled(" → ", Style::default().fg(self.theme.text_muted)),
                        Span::styled(detail.as_str(), Style::default().fg(self.theme.text_dim)),
                    ]))
                }
                ChatLine::Info(text) => {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{pad}ℹ  "), Style::default().fg(self.theme.text_muted)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text_dim)),
                    ]))
                }
                ChatLine::Error(text) => {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{pad}✗  "), Style::default().fg(self.theme.error).add_modifier(Modifier::BOLD)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.error)),
                    ]))
                }
                ChatLine::Thinking(text) => {
                    let spinner = crate::theme::SPINNER_FRAMES[self.spinner_idx % crate::theme::SPINNER_FRAMES.len()];
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{pad}{spinner}  "), Style::default().fg(self.theme.role_thinking)),
                        Span::styled(text.as_str(), Style::default().fg(self.theme.text_dim).add_modifier(Modifier::ITALIC)),
                    ]))
                }
                ChatLine::Diff(text) => {
                    let md_text = markdown::render_diff(
                        text,
                        self.theme.diff_added,
                        self.theme.diff_removed,
                        self.theme.text_dim,
                    );
                    ListItem::new(md_text)
                }
                ChatLine::Spacer => {
                    ListItem::new("")
                }
            }
        }).collect();

        let list = List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.theme.border))
                .title(Span::styled(" AXGA ", Style::default().fg(self.theme.primary).add_modifier(Modifier::BOLD))))
            .highlight_style(Style::default())
            .scroll_padding(0);

        // Render list with state
        let mut state = self.list_state.clone();
        f.render_stateful_widget(list, area, &mut state);

        // Scrollbar
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = self.scrollbar_state
            .content_length(self.chat_lines.len());
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(2),
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(2),
        };
        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let prompt = Span::styled(" > ", Style::default().fg(self.theme.primary).add_modifier(Modifier::BOLD));

        let input_display = if self.input.is_empty() {
            Span::styled("type your message...", Style::default().fg(self.theme.text_muted).add_modifier(Modifier::ITALIC))
        } else {
            Span::styled(self.input.as_str(), Style::default().fg(self.theme.text))
        };

        let text = Text::from(Line::from(vec![prompt, input_display]));
        let input_widget = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(
                    if self.mode == InputMode::Insert { self.theme.border_focus } else { self.theme.border }
                ))
                .title(Span::styled(" Input ", Style::default().fg(self.theme.text_dim))),
        );

        f.render_widget(input_widget, area);
    }
}
