//! Tracks the word currently being typed, so platform layers only have to
//! forward key events.

use crate::layout::{key_char, Key, Lang};

/// Keys typed since the last word boundary.
#[derive(Default, Debug, Clone)]
pub struct WordBuffer {
    keys: Vec<Key>,
}

impl WordBuffer {
    pub fn new() -> WordBuffer {
        WordBuffer::default()
    }

    /// A character key was pressed. Keys outside the main block are ignored.
    pub fn push(&mut self, key: Key) {
        if key_char(key, Lang::En).is_some() {
            self.keys.push(key);
        }
    }

    pub fn backspace(&mut self) {
        self.keys.pop();
    }

    /// Space, Enter, Tab, a mouse click, an arrow key, a shortcut …: anything
    /// that ends or abandons the word. Returns the finished word's keys.
    pub fn take(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
