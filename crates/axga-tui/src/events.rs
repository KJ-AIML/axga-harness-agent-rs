//! Keyboard and terminal event handling.
//!
//! Maps crossterm events to application actions.
//! All keybindings are configurable (ADR-aligned).

use crate::app::{App, InputMode};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::io;

impl App {
    pub fn handle_event(&mut self, event: Event) -> io::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(_, _) => {} // ratatui handles resize automatically
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
                let input = std::mem::take(&mut self.input);
                self.chat_lines.push(format!("> {}", input));
                // In full implementation: send to agent loop
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('i') => {
                self.mode = InputMode::Insert;
            }
            KeyCode::Char(':') => {
                self.mode = InputMode::Command;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit = true;
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(&mut self.input);
                match cmd.as_str() {
                    "q" | "quit" => self.exit = true,
                    _ => {
                        self.chat_lines.push(format!("Unknown command: {}", cmd));
                    }
                }
                self.mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            _ => {}
        }
    }
}
