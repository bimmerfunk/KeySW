//! Per-language statistics: a word-frequency dictionary and a character
//! trigram model trained from it.

use std::collections::HashMap;

use crate::layout::Lang;

const EN_TSV: &str = include_str!("../data/en.tsv");
const UK_TSV: &str = include_str!("../data/uk.tsv");
const RU_TSV: &str = include_str!("../data/ru.tsv");

const START: char = '^';
const END: char = '$';

/// Interpolation weights for trigram / bigram / unigram estimates.
const L3: f64 = 0.6;
const L2: f64 = 0.3;
const L1: f64 = 0.1;

pub struct LangModel {
    alphabet: Vec<char>,
    /// word -> relative frequency (0..1)
    words: HashMap<String, f64>,
    tri: HashMap<[char; 3], f64>,
    bi: HashMap<[char; 2], f64>,
    /// counts of a 2-char context as the first part of a trigram
    bi_ctx: HashMap<[char; 2], f64>,
    uni: HashMap<char, f64>,
    /// counts of a char as the first part of a bigram
    uni_ctx: HashMap<char, f64>,
    uni_total: f64,
}

impl LangModel {
    pub fn builtin(lang: Lang) -> LangModel {
        let tsv = match lang {
            Lang::En => EN_TSV,
            Lang::Uk => UK_TSV,
            Lang::Ru => RU_TSV,
        };
        LangModel::from_tsv(lang, tsv)
    }

    pub fn from_tsv(lang: Lang, tsv: &str) -> LangModel {
        let alphabet: Vec<char> = match lang {
            Lang::En => "abcdefghijklmnopqrstuvwxyz".chars().collect(),
            Lang::Uk => "абвгґдеєжзиіїйклмнопрстуфхцчшщьюя".chars().collect(),
            Lang::Ru => "абвгдеёжзийклмнопрстуфхцчшщъыьэюя".chars().collect(),
        };
        let mut m = LangModel {
            alphabet,
            words: HashMap::new(),
            tri: HashMap::new(),
            bi: HashMap::new(),
            bi_ctx: HashMap::new(),
            uni: HashMap::new(),
            uni_ctx: HashMap::new(),
            uni_total: 0.0,
        };
        let mut total = 0.0;
        let mut entries = Vec::new();
        for line in tsv.lines() {
            let Some((w, c)) = line.split_once('\t') else {
                continue;
            };
            let Ok(c) = c.trim().parse::<f64>() else {
                continue;
            };
            total += c;
            entries.push((w, c));
        }
        for (w, c) in entries {
            m.words.insert(w.to_string(), c / total);
            // Damp the counts so a few very common words don't dominate the
            // letter statistics.
            m.train(&normalize(w), c.sqrt());
        }
        m
    }

    fn train(&mut self, word: &str, weight: f64) {
        let seq = padded(word);
        for i in 2..seq.len() {
            let (a, b, c) = (seq[i - 2], seq[i - 1], seq[i]);
            *self.tri.entry([a, b, c]).or_default() += weight;
            *self.bi_ctx.entry([a, b]).or_default() += weight;
            *self.bi.entry([b, c]).or_default() += weight;
            *self.uni_ctx.entry(b).or_default() += weight;
            *self.uni.entry(c).or_default() += weight;
            self.uni_total += weight;
        }
    }

    /// Relative frequency of a (lower-case) word in the dictionary, 0 if absent.
    pub fn word_freq(&self, word: &str) -> f64 {
        self.words.get(word).copied().unwrap_or(0.0)
    }

    /// Whether every character is a letter of this language (apostrophes allowed
    /// inside the word).
    pub fn is_spellable(&self, word: &str) -> bool {
        let n = normalize(word);
        !n.is_empty() && n.chars().all(|c| self.alphabet.contains(&c))
    }

    /// Average log-probability per character transition (higher = more
    /// natural for this language). Returns `f64::NEG_INFINITY` for words that
    /// cannot be spelled in this language.
    pub fn score(&self, word: &str) -> f64 {
        if !self.is_spellable(word) {
            return f64::NEG_INFINITY;
        }
        let seq = padded(&normalize(word));
        let v = self.alphabet.len() as f64 + 1.0;
        let mut sum = 0.0;
        for i in 2..seq.len() {
            let (a, b, c) = (seq[i - 2], seq[i - 1], seq[i]);
            let p1 = (self.uni.get(&c).copied().unwrap_or(0.0) + 1.0) / (self.uni_total + v);
            let p2 = ratio(self.bi.get(&[b, c]), self.uni_ctx.get(&b));
            let p3 = ratio(self.tri.get(&[a, b, c]), self.bi_ctx.get(&[a, b]));
            sum += (L3 * p3 + L2 * p2 + L1 * p1).ln();
        }
        sum / (seq.len() - 2) as f64
    }
}

fn ratio(num: Option<&f64>, den: Option<&f64>) -> f64 {
    match (num, den) {
        (Some(n), Some(d)) if *d > 0.0 => n / d,
        _ => 0.0,
    }
}

/// Lower-case and drop apostrophes (п'ять / don't): the subtitle corpus
/// splits words on them, so the model never saw them.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|&c| c != '\'')
        .flat_map(char::to_lowercase)
        .collect()
}

fn padded(word: &str) -> Vec<char> {
    let mut v = vec![START, START];
    v.extend(word.chars());
    v.push(END);
    v
}
