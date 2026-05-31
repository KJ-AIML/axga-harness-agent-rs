//! TUI application state.
//!
//! # Memory
//! Chat history is a `VecDeque` capped at 100 lines in the TUI buffer.
//! The actual conversation lives in `axga-core::Conversation`.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct App {
    pub input: String,
    pub chat_lines: Vec<String>,
    pub status: StatusLine,
    pub mode: InputMode,
    pub exit: bool,
    pub scroll_offset: u16,
}

pub struct StatusLine {
    pub model: String,
    pub tokens_used: u32,
    pub memory_mb: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
}

impl App {
    pub fn new(model: &str) -> Self {
        Self {
            input: String::new(),
            chat_lines: Vec::with_capacity(100),
            status: StatusLine {
                model: model.to_string(),
                tokens_used: 0,
                memory_mb: 0.0,
            },
            mode: InputMode::Insert,
            exit: false,
            scroll_offset: 0,
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Chat
                Constraint::Length(1), // Status
                Constraint::Length(3), // Input
            ])
            .split(f.area());

        // Chat pane
        let chat_lines: Vec<Line> = self
            .chat_lines
            .iter()
            .map(|l| Line::from(l.as_str()))
            .collect();

        let chat = Paragraph::new(chat_lines)
            .block(Block::default().borders(Borders::ALL).title("AXGA"))
            .scroll((self.scroll_offset, 0));

        f.render_widget(chat, chunks[0]);

        // Status bar
        let status_text = format!(
            " {} | {} tokens | {:.1} MB ",
            self.status.model, self.status.tokens_used, self.status.memory_mb
        );
        let status = Paragraph::new(status_text)
            .style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray),
            );
        f.render_widget(status, chunks[1]);

        // Input pane
        let mode_str = match self.mode {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
            InputMode::Command => "CMD",
        };
        let input_text = if self.input.is_empty() {
            Text::from(format!("-- {} --  Start typing...", mode_str))
        } else {
            Text::from(self.input.as_str())
        };

        let input = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Input"))
            .style(Style::default().fg(Color::White));

        f.render_widget(input, chunks[2]);
    }
}
