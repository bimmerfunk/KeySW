//! `keysw` — try the engine without installing anything.
//!
//!     keysw ghbdsn ckfdf                 # typed in EN, meant Ukrainian
//!     keysw --last ru ghbdtn             # last Cyrillic layout was Russian
//!     keysw --layout uk руддщ цщкдв      # typed in UK, meant English
//!     echo "Ghbdsn, zr cghfdb?" | keysw  # whole lines from stdin
//!     keysw --explain ghbdsn             # show the scores behind a decision

use std::io::{self, BufRead};

use keysw_core::{char_key, Engine, Key, Lang};

fn main() {
    let mut engine = Engine::new();
    let mut layout = Lang::En;
    let mut explain = false;
    let mut words = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--layout" | "-l" => layout = parse_lang(args.next()),
            "--last" => {
                let l = parse_lang(args.next());
                engine.set_layout(l);
            }
            "--explain" | "-e" => explain = true,
            "--help" | "-h" => {
                println!("usage: keysw [--layout en|uk|ru] [--last uk|ru] [--explain] [WORDS...]");
                println!("Without WORDS, reads lines from stdin.");
                return;
            }
            _ => words.push(a),
        }
    }

    if words.is_empty() {
        for line in io::stdin().lock().lines() {
            let line = line.expect("stdin");
            println!("{}", fix_line(&mut engine, layout, &line, explain));
        }
    } else {
        println!(
            "{}",
            fix_line(&mut engine, layout, &words.join(" "), explain)
        );
    }
}

fn parse_lang(s: Option<String>) -> Lang {
    s.as_deref().and_then(Lang::from_code).unwrap_or_else(|| {
        eprintln!("expected en, uk or ru");
        std::process::exit(2)
    })
}

/// Simulates typing a line: every space ends a word, a correction switches
/// the layout for the following words, like the real app does.
fn fix_line(engine: &mut Engine, start: Lang, line: &str, explain: bool) -> String {
    engine.set_layout(start);
    let mut out = Vec::new();
    for word in line.split(' ') {
        let layout = engine.current_layout();
        // Re-create the key presses as they would be in the current layout.
        let typed: String = if layout == start {
            word.to_string()
        } else {
            keysw_core::convert(word, start, layout)
        };
        let keys: Vec<Key> = typed.chars().filter_map(|c| char_key(c, layout)).collect();
        if explain {
            let alt = engine.alternative();
            let shown = keysw_core::render(&keys, alt);
            let (fc, sc) = engine.explain(&typed, layout);
            let (fa, sa) = engine.explain(&shown, alt);
            eprintln!(
                "{typed:>14} [{layout}] freq={fc:.2e} score={sc:.2}   {shown:>14} [{alt}] freq={fa:.2e} score={sa:.2}"
            );
        }
        match engine.check_word(&keys) {
            Some(fix) => {
                out.push(fix.replacement);
                engine.set_layout(fix.to);
            }
            None => out.push(typed),
        }
    }
    out.join(" ")
}
