//! Keyboard layouts and key-by-key conversion between them.
//!
//! A key is identified by the character it produces on the US QWERTY layout
//! without Shift (`'q'`, `'['`, `'`'` …). Platform layers translate their
//! scan codes / key codes to this form, so the core never deals with OS APIs.

use std::fmt;

/// An input language / keyboard layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Uk,
    Ru,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Uk, Lang::Ru];

    pub fn is_cyrillic(self) -> bool {
        !matches!(self, Lang::En)
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Uk => "uk",
            Lang::Ru => "ru",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code.to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "uk" | "ua" => Some(Lang::Uk),
            "ru" => Some(Lang::Ru),
            _ => None,
        }
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// One physical key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// The unshifted character this key produces on the US QWERTY layout.
    pub base: char,
    /// Whether the produced character is the shifted (upper) one.
    /// Platform layers should fold Caps Lock into this flag for letter keys.
    pub shift: bool,
}

impl Key {
    pub fn new(base: char, shift: bool) -> Key {
        Key { base, shift }
    }
}

/// (US lower, US upper, UK lower, UK upper, RU lower, RU upper)
///
/// UK follows "Ukrainian (Enhanced)" on Windows / "Ukrainian - PC" on macOS:
/// apostrophe on the backtick key and ґ on the backslash key.
const TABLE: &[(char, char, char, char, char, char)] = &[
    ('`', '~', '\'', '₴', 'ё', 'Ё'),
    ('1', '!', '1', '!', '1', '!'),
    ('2', '@', '2', '"', '2', '"'),
    ('3', '#', '3', '№', '3', '№'),
    ('4', '$', '4', ';', '4', ';'),
    ('5', '%', '5', '%', '5', '%'),
    ('6', '^', '6', ':', '6', ':'),
    ('7', '&', '7', '?', '7', '?'),
    ('8', '*', '8', '*', '8', '*'),
    ('9', '(', '9', '(', '9', '('),
    ('0', ')', '0', ')', '0', ')'),
    ('-', '_', '-', '_', '-', '_'),
    ('=', '+', '=', '+', '=', '+'),
    ('q', 'Q', 'й', 'Й', 'й', 'Й'),
    ('w', 'W', 'ц', 'Ц', 'ц', 'Ц'),
    ('e', 'E', 'у', 'У', 'у', 'У'),
    ('r', 'R', 'к', 'К', 'к', 'К'),
    ('t', 'T', 'е', 'Е', 'е', 'Е'),
    ('y', 'Y', 'н', 'Н', 'н', 'Н'),
    ('u', 'U', 'г', 'Г', 'г', 'Г'),
    ('i', 'I', 'ш', 'Ш', 'ш', 'Ш'),
    ('o', 'O', 'щ', 'Щ', 'щ', 'Щ'),
    ('p', 'P', 'з', 'З', 'з', 'З'),
    ('[', '{', 'х', 'Х', 'х', 'Х'),
    (']', '}', 'ї', 'Ї', 'ъ', 'Ъ'),
    ('\\', '|', 'ґ', 'Ґ', '\\', '/'),
    ('a', 'A', 'ф', 'Ф', 'ф', 'Ф'),
    ('s', 'S', 'і', 'І', 'ы', 'Ы'),
    ('d', 'D', 'в', 'В', 'в', 'В'),
    ('f', 'F', 'а', 'А', 'а', 'А'),
    ('g', 'G', 'п', 'П', 'п', 'П'),
    ('h', 'H', 'р', 'Р', 'р', 'Р'),
    ('j', 'J', 'о', 'О', 'о', 'О'),
    ('k', 'K', 'л', 'Л', 'л', 'Л'),
    ('l', 'L', 'д', 'Д', 'д', 'Д'),
    (';', ':', 'ж', 'Ж', 'ж', 'Ж'),
    ('\'', '"', 'є', 'Є', 'э', 'Э'),
    ('z', 'Z', 'я', 'Я', 'я', 'Я'),
    ('x', 'X', 'ч', 'Ч', 'ч', 'Ч'),
    ('c', 'C', 'с', 'С', 'с', 'С'),
    ('v', 'V', 'м', 'М', 'м', 'М'),
    ('b', 'B', 'и', 'И', 'и', 'И'),
    ('n', 'N', 'т', 'Т', 'т', 'Т'),
    ('m', 'M', 'ь', 'Ь', 'ь', 'Ь'),
    (',', '<', 'б', 'Б', 'б', 'Б'),
    ('.', '>', 'ю', 'Ю', 'ю', 'Ю'),
    ('/', '?', '.', ',', '.', ','),
];

fn row(base: char) -> Option<&'static (char, char, char, char, char, char)> {
    TABLE.iter().find(|r| r.0 == base)
}

/// The character `key` produces in `lang`, or `None` for keys outside the
/// main character block (those are not part of words anyway).
pub fn key_char(key: Key, lang: Lang) -> Option<char> {
    let r = row(key.base)?;
    Some(match (lang, key.shift) {
        (Lang::En, false) => r.0,
        (Lang::En, true) => r.1,
        (Lang::Uk, false) => r.2,
        (Lang::Uk, true) => r.3,
        (Lang::Ru, false) => r.4,
        (Lang::Ru, true) => r.5,
    })
}

/// Which key (and Shift state) produces `c` in `lang`.
pub fn char_key(c: char, lang: Lang) -> Option<Key> {
    TABLE.iter().find_map(|r| {
        let (lo, up) = match lang {
            Lang::En => (r.0, r.1),
            Lang::Uk => (r.2, r.3),
            Lang::Ru => (r.4, r.5),
        };
        if c == lo {
            Some(Key::new(r.0, false))
        } else if c == up {
            Some(Key::new(r.0, true))
        } else {
            None
        }
    })
}

/// Render a key sequence as text in `lang`. Keys with no character are skipped.
pub fn render(keys: &[Key], lang: Lang) -> String {
    keys.iter().filter_map(|&k| key_char(k, lang)).collect()
}

/// Re-type `text` as if it had been typed with the same keys in another layout.
/// Characters that have no key in `from` (spaces, emoji …) are kept as is.
///
/// `convert("ghbdsn", Lang::En, Lang::Uk) == "привіт"`
pub fn convert(text: &str, from: Lang, to: Lang) -> String {
    text.chars()
        .map(|c| match char_key(c, from).and_then(|k| key_char(k, to)) {
            Some(out) => out,
            None => c,
        })
        .collect()
}

/// Turn text typed on the US layout into keys (handy for tests and the CLI).
pub fn keys_from_en(text: &str) -> Vec<Key> {
    text.chars().filter_map(|c| char_key(c, Lang::En)).collect()
}
