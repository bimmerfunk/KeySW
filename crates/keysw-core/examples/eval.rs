//! Measure detection quality on word lists the dictionary has not seen.
//!
//!     cargo run --release -p keysw-core --example eval -- en words_en.txt uk words_uk.txt ...
//!
//! For each word in language L it checks two things:
//!   * typed correctly in L, it must be left alone (false positive otherwise);
//!   * typed with the same keys in the other layout, it must be corrected
//!     back to L (miss otherwise).

use keysw_core::{char_key, Engine, Key, Lang};

fn keys(word: &str, lang: Lang) -> Option<Vec<Key>> {
    word.chars().map(|c| char_key(c, lang)).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config = keysw_core::Config::default();
    if let Ok(v) = std::env::var("MARGIN") {
        config.margin = v.parse().unwrap();
    }
    if let Ok(v) = std::env::var("FLOOR") {
        config.floor = v.parse().unwrap();
    }
    let engine = Engine::with_config(config);
    let verbose = std::env::var("VERBOSE").is_ok();
    for pair in args.chunks(2) {
        let lang = Lang::from_code(&pair[0]).expect("lang");
        let text = std::fs::read_to_string(&pair[1]).expect("file");
        let others: Vec<Lang> = if lang == Lang::En {
            vec![Lang::Uk, Lang::Ru]
        } else {
            vec![Lang::En]
        };
        for other in others {
            let (mut total, mut fp, mut miss) = (0, 0, 0);
            for w in text.lines().map(str::trim).filter(|w| !w.is_empty()) {
                let Some(k) = keys(w, lang) else { continue };
                total += 1;
                if let Some(c) = engine.check_word_in(&k, lang, other) {
                    fp += 1;
                    if verbose {
                        println!("FP   {lang}->{other}: {w} -> {}", c.replacement);
                    }
                }
                if engine.check_word_in(&k, other, lang).is_none() {
                    miss += 1;
                    if verbose {
                        println!(
                            "MISS {other}->{lang}: {} (meant {w})",
                            keysw_core::render(&k, other)
                        );
                    }
                }
            }
            println!(
                "{lang} words vs {other} layout: {total} words, false positives {fp} ({:.1}%), missed {miss} ({:.1}%)",
                100.0 * fp as f64 / total as f64,
                100.0 * miss as f64 / total as f64
            );
        }
    }
}
