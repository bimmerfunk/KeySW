//! The typing session: what the Windows and macOS apps talk to.
//!
//! The app forwards keyboard events (before they reach the focused window)
//! and performs the [`Action`]s it gets back. All logic, including undo,
//! lives here so it is the same on every OS and covered by tests.

use crate::buffer::WordBuffer;
use crate::engine::{Correction, Engine};
use crate::layout::{Key, Lang};
use crate::model::normalize;

/// Something the app must do, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Press Backspace this many times.
    Erase(usize),
    /// Type this text (as Unicode, independent of the layout).
    Type(String),
    /// Switch the system input layout.
    SwitchLayout(Lang),
}

/// The app's answer to one keyboard event.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Response {
    /// Drop the original key event instead of passing it on (the actions
    /// already re-type it where needed).
    pub swallow: bool,
    pub actions: Vec<Action>,
}

impl Response {
    fn pass() -> Response {
        Response::default()
    }
}

/// Two Shift taps within this time (with nothing in between) undo the last
/// automatic correction.
pub const DOUBLE_SHIFT_MS: u64 = 400;

struct LastFix {
    fix: Correction,
    /// Word boundary typed after the correction (usually a space).
    boundary: String,
}

pub struct Session {
    pub engine: Engine,
    buffer: WordBuffer,
    last_fix: Option<LastFix>,
    last_shift_tap: Option<u64>,
    /// Undo by pressing Backspace right after a correction (off by default:
    /// people also press Backspace just to remove the space).
    pub undo_on_backspace: bool,
}

impl Default for Session {
    fn default() -> Self {
        Session::new(Engine::new())
    }
}

impl Session {
    pub fn new(engine: Engine) -> Session {
        Session {
            engine,
            buffer: WordBuffer::new(),
            last_fix: None,
            last_shift_tap: None,
            undo_on_backspace: false,
        }
    }

    /// The system layout changed (by the person, or after our own switch).
    pub fn layout_changed(&mut self, lang: Lang) {
        self.engine.set_layout(lang);
    }

    /// A character key of the main block was pressed.
    pub fn on_char(&mut self, key: Key) -> Response {
        self.last_fix = None;
        self.last_shift_tap = None;
        self.buffer.push(key);
        Response::pass()
    }

    /// Space, Enter or Tab was pressed: the word is finished. Call this before
    /// the key reaches the app; if a correction is made the key is swallowed
    /// and re-typed after the fixed word.
    pub fn on_boundary(&mut self, boundary: char) -> Response {
        self.last_shift_tap = None;
        self.last_fix = None;
        let keys = self.buffer.take();
        let Some(fix) = self.engine.check_word(&keys) else {
            return Response::pass();
        };
        self.engine.set_layout(fix.to);
        let boundary = boundary.to_string();
        let actions = vec![
            Action::Erase(fix.erase),
            Action::Type(fix.replacement.clone()),
            Action::SwitchLayout(fix.to),
            Action::Type(boundary.clone()),
        ];
        self.last_fix = Some(LastFix { fix, boundary });
        Response {
            swallow: true,
            actions,
        }
    }

    pub fn on_backspace(&mut self) -> Response {
        self.last_shift_tap = None;
        if self.undo_on_backspace && self.last_fix.is_some() {
            return self.undo();
        }
        self.last_fix = None;
        self.buffer.backspace();
        Response::pass()
    }

    /// Shift was pressed and released on its own. A second tap within
    /// [`DOUBLE_SHIFT_MS`] undoes the last correction.
    pub fn on_shift_tap(&mut self, now_ms: u64) -> Response {
        match self.last_shift_tap.take() {
            Some(prev) if now_ms.saturating_sub(prev) <= DOUBLE_SHIFT_MS => self.undo(),
            _ => {
                self.last_shift_tap = Some(now_ms);
                Response::pass()
            }
        }
    }

    /// Anything that moves the cursor or the focus: mouse click, arrows,
    /// Home/End, a shortcut with Ctrl/Cmd, switching windows.
    pub fn on_reset(&mut self) {
        self.buffer.clear();
        self.last_fix = None;
        self.last_shift_tap = None;
    }

    /// Whether [`undo`](Self::undo) would do something right now.
    pub fn can_undo(&self) -> bool {
        self.last_fix.is_some()
    }

    /// Undo the last automatic correction: put back what was typed, return to
    /// the previous layout and don't correct that word again this session.
    /// Works only right after the correction, before anything else is typed.
    /// Bind the app's undo hotkey to this.
    pub fn undo(&mut self) -> Response {
        self.last_shift_tap = None;
        let Some(LastFix { fix, boundary }) = self.last_fix.take() else {
            return Response::pass();
        };
        self.engine.set_layout(fix.from);
        self.engine.config.exceptions.insert(normalize(
            fix.original.trim_matches(|c: char| !c.is_alphabetic()),
        ));
        let erase = fix.replacement.chars().count() + boundary.chars().count();
        Response {
            swallow: true,
            actions: vec![
                Action::Erase(erase),
                Action::Type(fix.original),
                Action::SwitchLayout(fix.from),
                Action::Type(boundary),
            ],
        }
    }
}
