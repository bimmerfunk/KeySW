//! The decision logic: was this word typed in the wrong layout, and what
//! should it become?

use std::collections::HashSet;

use crate::layout::{render, Key, Lang};
use crate::model::{normalize, LangModel};

/// Tunable thresholds. The defaults are what the tests are calibrated for.
#[derive(Clone, Debug)]
pub struct Config {
    /// Words shorter than this (in letters) are never corrected by letter
    /// statistics alone; they need to be a dictionary word in the other layout.
    pub min_len_statistical: usize,
    /// Words shorter than this are never corrected, except one-letter words
    /// such as я / в / і / a / I.
    pub min_len: usize,
    /// How much better (avg. log-prob per letter) the other layout must score.
    pub margin: f64,
    /// The other layout's text must score at least this to look like a real word.
    pub floor: f64,
    /// When the text is a dictionary word in both layouts, switch only if the
    /// other one is this many times more frequent.
    pub both_words_ratio: f64,
    /// Dictionary words of 1-3 letters rarer than this are ignored.
    pub short_word_min_freq: f64,
    /// Words (lower-case, as typed) that are never corrected.
    pub exceptions: HashSet<String>,
}

/// Short Latin abbreviations that happen to spell common Cyrillic words
/// (db -> ви, gk -> пл …). People type these on purpose.
const DEFAULT_EXCEPTIONS: &[&str] = &[
    "db", "gb", "mb", "kb", "tb", "js", "ts", "ui", "ux", "qa", "pm", "hr", "cv", "tv", "pc", "os",
    "vs", "id", "ip", "ok", "gg", "dm", "pr", "ci", "cd", "vm", "ai", "ml",
];

impl Default for Config {
    fn default() -> Self {
        Config {
            min_len_statistical: 3,
            min_len: 2,
            margin: 0.8,
            floor: -3.6,
            both_words_ratio: 50.0,
            short_word_min_freq: 5e-6,
            exceptions: DEFAULT_EXCEPTIONS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// What the platform layer should do: erase `erase` characters (the word as it
/// was typed), type `replacement`, and switch the system layout to `to`.
#[derive(Clone, Debug, PartialEq)]
pub struct Correction {
    pub from: Lang,
    pub to: Lang,
    pub original: String,
    pub replacement: String,
    pub erase: usize,
}

pub struct Engine {
    pub config: Config,
    models: [LangModel; 3],
    current: Lang,
    last_cyrillic: Lang,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Engine {
    pub fn new() -> Engine {
        Engine::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Engine {
        Engine {
            config,
            models: Lang::ALL.map(LangModel::builtin),
            current: Lang::En,
            last_cyrillic: Lang::Uk,
        }
    }

    fn model(&self, lang: Lang) -> &LangModel {
        &self.models[Lang::ALL.iter().position(|&l| l == lang).unwrap()]
    }

    /// Tell the engine which layout is active right now. Call it on start-up,
    /// whenever the system layout changes, and after applying a correction.
    pub fn set_layout(&mut self, lang: Lang) {
        self.current = lang;
        if lang.is_cyrillic() {
            self.last_cyrillic = lang;
        }
    }

    pub fn current_layout(&self) -> Lang {
        self.current
    }

    /// The Cyrillic layout English-layout mistakes get converted to: the one
    /// the person used most recently (Ukrainian until we have seen one).
    pub fn last_cyrillic(&self) -> Lang {
        self.last_cyrillic
    }

    /// The layout a word typed in `current` would be corrected to.
    pub fn alternative(&self) -> Lang {
        if self.current.is_cyrillic() {
            Lang::En
        } else {
            self.last_cyrillic
        }
    }

    /// Check a finished word (the keys pressed since the last word boundary)
    /// typed in the current layout.
    pub fn check_word(&self, keys: &[Key]) -> Option<Correction> {
        self.check_word_in(keys, self.current, self.alternative())
    }

    /// Same as [`check_word`](Self::check_word) with explicit layouts.
    pub fn check_word_in(&self, keys: &[Key], from: Lang, to: Lang) -> Option<Correction> {
        let original = render(keys, from);
        let replacement = render(keys, to);
        if original.is_empty() || original == replacement {
            return None;
        }
        // Words with digits are codes, versions, passwords … leave them alone.
        if original.chars().any(|c| c.is_ascii_digit()) {
            return None;
        }
        // Mixed case like "iPhone" is intentional, not a layout mistake.
        if has_odd_case(&original) {
            return None;
        }

        let cur_core = normalize(core(&original));
        let alt_core = normalize(core(&replacement));
        let cur = self.model(from);
        let alt = self.model(to);

        let n = alt_core.chars().count();
        if !alt.is_spellable(&alt_core) {
            return None;
        }
        if n == 1 && original.chars().count() == 1 {
            // Single letters: only real one-letter words (z -> я, f -> а),
            // and never when the typed letter is a word itself (a, I).
            let fix =
                is_single_letter_word(&alt_core, to) && !is_single_letter_word(&cur_core, from);
            return fix.then_some(Correction {
                from,
                to,
                erase: 1,
                original,
                replacement,
            });
        }
        if n < self.config.min_len {
            return None;
        }
        // A wrong-layout word turns into letters in the other layout from
        // start to end; punctuation in the middle means it wasn't one.
        if core(&replacement).chars().any(|c| !is_word_char(c)) {
            return None;
        }
        // Letters turning into punctuation (буг -> ,eu) means the original
        // was a real word whose first key just isn't a letter elsewhere. At the
        // end it is normal: "Hello," typed in the UK layout reads "Руддщб".
        if lost_letters_before_end(&original, &replacement) {
            return None;
        }
        if self.config.exceptions.contains(&cur_core) {
            return None;
        }

        // If punctuation around the typed word becomes letters (];fr -> їжак),
        // the typed "word" is just a fragment, so a dictionary hit means nothing.
        let fragment = letters(&replacement) > letters(&original);
        let fc = if fragment || !cur.is_spellable(&cur_core) {
            0.0
        } else {
            let f = cur.word_freq(&cur_core);
            // The subtitle lists contain junk short tokens ("lf", "fr");
            // only trust short words that are reasonably common.
            if cur_core.chars().count() <= 3 && f < self.config.short_word_min_freq {
                0.0
            } else {
                f
            }
        };
        let fa = alt.word_freq(&alt_core);

        let switch = if fa > 0.0 && fc > 0.0 {
            fa > fc * self.config.both_words_ratio
        } else if fa > 0.0 {
            true
        } else if fc > 0.0 {
            false
        } else {
            let sa = alt.score(&alt_core);
            let sc = if fragment {
                f64::NEG_INFINITY
            } else {
                cur.score(&cur_core)
            };
            n >= self.config.min_len_statistical
                && sa >= self.config.floor
                && sa - sc >= self.config.margin
        };

        switch.then(|| Correction {
            from,
            to,
            erase: original.chars().count(),
            original,
            replacement,
        })
    }

    /// Debug helper: (dictionary frequency, trigram score) of `word` in `lang`.
    pub fn explain(&self, word: &str, lang: Lang) -> (f64, f64) {
        let m = self.model(lang);
        let w = normalize(core(word));
        (m.word_freq(&w), m.score(&w))
    }
}

/// True if some letter of `original` maps to a non-letter in `replacement`
/// anywhere except in the trailing run of punctuation.
fn lost_letters_before_end(original: &str, replacement: &str) -> bool {
    let o: Vec<char> = original.chars().collect();
    let r: Vec<char> = replacement.chars().collect();
    let mut end = r.len();
    while end > 0 && !r[end - 1].is_alphabetic() {
        end -= 1;
    }
    (0..end.min(o.len())).any(|i| o[i].is_alphabetic() && !r[i].is_alphabetic())
}

fn is_single_letter_word(w: &str, lang: Lang) -> bool {
    let words = match lang {
        Lang::En => "ai",
        Lang::Uk => "явуізайо",
        Lang::Ru => "явисуаок",
    };
    w.chars().count() == 1 && words.contains(w)
}

fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphabetic()).count()
}

fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c == '\''
}

/// The word without leading/trailing punctuation.
fn core(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphabetic())
}

/// Lower-case letters after an upper-case one are fine (Title, CAPS); an
/// upper-case letter after a lower-case one is not something people mistype.
fn has_odd_case(s: &str) -> bool {
    let mut seen_lower = false;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        if c.is_lowercase() {
            seen_lower = true;
        } else if seen_lower {
            return true;
        }
    }
    false
}
