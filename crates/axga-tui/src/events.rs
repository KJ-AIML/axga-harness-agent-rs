//! Keyboard event handling.
//!
//! Maps crossterm events to application actions.

use crate::app::{App, InputMode};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::io;

impl App {
    pub fn handle_event(&mut self, event: Event) -> io::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            InputMode::Insert => self.handle_insert_key(key),
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Command => self.handle_command_key(key),
        }
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                // Handled by TUI loop — just prevents newline in input
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input.len() {
                    self.input.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.input.len();
            }
            KeyCode::Char(c) => {
                // Allow all printable chars
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('i') | KeyCode::Char('a') => {
                self.mode = InputMode::Insert;
                if key.code == KeyCode::Char('a') {
                    self.cursor_pos = self.cursor_pos.saturating_add(1).min(self.input.len());
                }
            }
            KeyCode::Char(':') => {
                self.mode = InputMode::Command;
                self.input.push(':');
                self.cursor_pos = 1;
            }
            KeyCode::Char('/') => {
                self.mode = InputMode::Command;
                self.input.push('/');
                self.cursor_pos = 1;
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
            }
            KeyCode::End => {
                self.scroll_offset = u16::MAX; // Will be clamped in render
            }
            KeyCode::Enter => {
                // Submit current input
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
                self.input.clear();
                self.cursor_pos = 0;
            }
            KeyCode::Enter => {
                // Command will be processed by TUI loop
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
    }
}
