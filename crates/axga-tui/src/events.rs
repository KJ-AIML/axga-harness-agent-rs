//! Keyboard event handling (deprecated — handled inline in tui_mode.rs).
//! This file kept for future refactoring.

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
            KeyCode::Esc => self.mode = InputMode::Normal,
            KeyCode::Enter => {}
            KeyCode::Backspace => { if self.cursor_pos > 0 { self.input.remove(self.cursor_pos - 1); self.cursor_pos -= 1; } }
            KeyCode::Delete => { if self.cursor_pos < self.input.len() { self.input.remove(self.cursor_pos); } }
            KeyCode::Left => { if self.cursor_pos > 0 { self.cursor_pos -= 1; } }
            KeyCode::Right => { if self.cursor_pos < self.input.len() { self.cursor_pos += 1; } }
            KeyCode::Home => self.cursor_pos = 0,
            KeyCode::End => self.cursor_pos = self.input.len(),
            KeyCode::Char(c) => { self.input.insert(self.cursor_pos, c); self.cursor_pos += 1; }
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('i') | KeyCode::Char('a') => {
                self.mode = InputMode::Insert;
                if key.code == KeyCode::Char('a') { self.cursor_pos = self.cursor_pos.saturating_add(1).min(self.input.len()); }
            }
            KeyCode::Char(':') => { self.mode = InputMode::Command; self.input.push(':'); self.cursor_pos = 1; }
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.exit = true,
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => { self.mode = InputMode::Normal; self.input.clear(); self.cursor_pos = 0; }
            KeyCode::Enter => {}
            KeyCode::Backspace => { if self.cursor_pos > 0 { self.input.remove(self.cursor_pos - 1); self.cursor_pos -= 1; } }
            KeyCode::Char(c) => { self.input.insert(self.cursor_pos, c); self.cursor_pos += 1; }
            _ => {}
        }
    }
}
