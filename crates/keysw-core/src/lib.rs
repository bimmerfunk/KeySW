//! KeySW core: detects words typed in the wrong keyboard layout
//! (English ↔ Ukrainian / Russian) and converts them.
//!
//! This crate has no OS dependencies. The Windows and macOS apps feed it key
//! presses and apply the [`Correction`]s it returns.
//!
//! ```
//! use keysw_core::{Engine, Lang, keys_from_en};
//!
//! let mut engine = Engine::new();
//! engine.set_layout(Lang::En);
//! let fix = engine.check_word(&keys_from_en("ghbdsn")).unwrap();
//! assert_eq!(fix.replacement, "привіт");
//! assert_eq!(fix.to, Lang::Uk);
//! ```

mod buffer;
mod engine;
mod layout;
mod model;

pub use buffer::WordBuffer;
pub use engine::{Config, Correction, Engine};
pub use layout::{char_key, convert, key_char, keys_from_en, render, Key, Lang};
