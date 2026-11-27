//! Real phrases typed in the wrong layout, and text that must be left alone.

use keysw_core::{char_key, convert, Engine, Key, Lang, WordBuffer};

fn keys(word: &str, lang: Lang) -> Vec<Key> {
    word.chars()
        .map(|c| char_key(c, lang).expect("typeable"))
        .collect()
}

/// Feed `typed` word by word in `layout`, applying corrections the way the
/// apps do (the layout switches after a fix).
fn type_text(engine: &mut Engine, layout: Lang, typed: &str) -> String {
    engine.set_layout(layout);
    typed
        .split(' ')
        .map(|w| {
            let cur = engine.current_layout();
            let k = keys(&convert(w, layout, cur), cur);
            match engine.check_word(&k) {
                Some(fix) => {
                    engine.set_layout(fix.to);
                    fix.replacement
                }
                None => keysw_core::render(&k, cur),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn ukrainian_typed_in_english_layout() {
    let mut e = Engine::new();
    e.set_layout(Lang::Uk);
    let typed = convert(
        "Привіт, як справи? Я пишу нову програму для клавіатури",
        Lang::Uk,
        Lang::En,
    );
    assert_eq!(
        type_text(&mut e, Lang::En, &typed),
        "Привіт, як справи? Я пишу нову програму для клавіатури"
    );
}

#[test]
fn russian_typed_in_english_layout_when_last_cyrillic_was_russian() {
    let mut e = Engine::new();
    e.set_layout(Lang::Ru);
    let typed = convert(
        "Привет, как дела? Сегодня хорошая погода",
        Lang::Ru,
        Lang::En,
    );
    assert_eq!(
        type_text(&mut e, Lang::En, &typed),
        "Привет, как дела? Сегодня хорошая погода"
    );
}

#[test]
fn english_typed_in_cyrillic_layouts() {
    for lang in [Lang::Uk, Lang::Ru] {
        let mut e = Engine::new();
        let typed = convert(
            "Hello, how are you? This keyboard switcher works",
            Lang::En,
            lang,
        );
        assert_eq!(
            type_text(&mut e, lang, &typed),
            "Hello, how are you? This keyboard switcher works",
            "typed in {lang}"
        );
    }
}

#[test]
fn last_used_cyrillic_decides_the_target() {
    let mut e = Engine::new();
    assert_eq!(e.last_cyrillic(), Lang::Uk, "Ukrainian by default");
    e.set_layout(Lang::Ru);
    e.set_layout(Lang::En);
    assert_eq!(e.alternative(), Lang::Ru);
    assert_eq!(
        e.check_word(&keys("ghbdtn", Lang::En)).unwrap().replacement,
        "привет"
    );
    e.set_layout(Lang::Uk);
    e.set_layout(Lang::En);
    assert_eq!(
        e.check_word(&keys("ghbdsn", Lang::En)).unwrap().replacement,
        "привіт"
    );
}

#[test]
fn correct_text_is_left_alone() {
    let mut e = Engine::new();
    let en = "I think the meeting is at five o'clock, see you there";
    assert_eq!(type_text(&mut e, Lang::En, en), en);
    let uk = "Добрий день, я сьогодні працюю з дому";
    assert_eq!(type_text(&mut e, Lang::Uk, uk), uk);
    let ru = "Добрый день, я сегодня работаю из дома";
    assert_eq!(type_text(&mut e, Lang::Ru, ru), ru);
}

#[test]
fn tech_words_codes_and_names_are_left_alone() {
    let mut e = Engine::new();
    for w in [
        "db", "api", "json", "html", "github", "docker", "sql", "iPhone", "v2", "x86", "abc123",
        "ok",
    ] {
        e.set_layout(Lang::En);
        assert_eq!(e.check_word(&keys(w, Lang::En)), None, "{w}");
    }
}

#[test]
fn single_letters() {
    let mut e = Engine::new();
    // Real one-letter words are fixed …
    for (typed, want) in [("z", "я"), ("Z", "Я"), ("d", "в"), ("s", "і"), ("p", "з")] {
        assert_eq!(
            e.check_word(&keys(typed, Lang::En))
                .map(|f| f.replacement)
                .as_deref(),
            Some(want),
            "{typed}"
        );
    }
    e.set_layout(Lang::Ru);
    e.set_layout(Lang::En);
    assert_eq!(e.check_word(&keys("b", Lang::En)).unwrap().replacement, "и");
    // … other letters and English one-letter words are not.
    for w in ["a", "I", "x", "q"] {
        assert_eq!(e.check_word(&keys(w, Lang::En)), None, "{w}");
    }
    e.set_layout(Lang::Uk);
    assert_eq!(e.check_word(&keys("ш", Lang::Uk)).unwrap().replacement, "i");
    assert_eq!(e.check_word(&keys("я", Lang::Uk)), None);
}

#[test]
fn correction_reports_what_to_erase_and_type() {
    let e = Engine::new();
    let fix = e.check_word(&keys("Ghbdsn", Lang::En)).unwrap();
    assert_eq!(fix.original, "Ghbdsn");
    assert_eq!(fix.replacement, "Привіт");
    assert_eq!(fix.erase, 6);
    assert_eq!((fix.from, fix.to), (Lang::En, Lang::Uk));
}

#[test]
fn ukrainian_letters_and_apostrophe() {
    let e = Engine::new();
    // п'ять, їжак, ґанок, єдиний: apostrophe on `, ї on ], ґ on \, є on '
    for word in ["п'ять", "їжак", "ґанок", "єдиний", "сім'я"] {
        let typed = convert(word, Lang::Uk, Lang::En);
        let fix = e.check_word(&keys(&typed, Lang::En));
        assert_eq!(
            fix.map(|f| f.replacement).as_deref(),
            Some(word),
            "typed as {typed}"
        );
    }
}

#[test]
fn conversion_round_trips() {
    for (text, lang) in [
        ("Привіт, світе! Ґудзик, їжак, є'", Lang::Uk),
        ("Съешь же ещё этих мягких булок", Lang::Ru),
    ] {
        let en = convert(text, lang, Lang::En);
        assert_eq!(convert(&en, Lang::En, lang), text);
    }
    assert_eq!(convert("ghbdsn", Lang::En, Lang::Uk), "привіт");
    assert_eq!(convert("руддщ", Lang::Ru, Lang::En), "hello");
}

#[test]
fn word_buffer_tracks_backspace() {
    let mut b = WordBuffer::new();
    for k in keys("ghbdsnn", Lang::En) {
        b.push(k);
    }
    b.backspace();
    let e = Engine::new();
    assert_eq!(e.check_word(&b.take()).unwrap().replacement, "привіт");
    assert!(b.is_empty());
}
