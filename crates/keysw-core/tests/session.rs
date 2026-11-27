//! The typing session as the apps drive it, including undo.

use keysw_core::{keys_from_en, Action, Lang, Session};

fn type_word(s: &mut Session, typed_en_keys: &str) {
    for k in keys_from_en(typed_en_keys) {
        s.on_char(k);
    }
}

fn act(erase: usize, text: &str, lang: Lang, boundary: &str) -> Vec<Action> {
    vec![
        Action::Erase(erase),
        Action::Type(text.into()),
        Action::SwitchLayout(lang),
        Action::Type(boundary.into()),
    ]
}

#[test]
fn correction_on_space() {
    let mut s = Session::default();
    s.layout_changed(Lang::En);
    type_word(&mut s, "ghbdsn");
    let r = s.on_boundary(' ');
    assert!(r.swallow);
    assert_eq!(r.actions, act(6, "привіт", Lang::Uk, " "));
    assert_eq!(s.engine.current_layout(), Lang::Uk);
}

#[test]
fn double_shift_undoes_last_correction() {
    let mut s = Session::default();
    s.layout_changed(Lang::En);
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    assert!(s.can_undo());

    assert!(s.on_shift_tap(1000).actions.is_empty());
    let r = s.on_shift_tap(1250);
    assert!(r.swallow);
    // erase "привіт " (7 chars), put back "ghbdsn ", back to English
    assert_eq!(r.actions, act(7, "ghbdsn", Lang::En, " "));
    assert_eq!(s.engine.current_layout(), Lang::En);
    assert!(!s.can_undo());
}

#[test]
fn undone_word_is_not_corrected_again() {
    let mut s = Session::default();
    s.layout_changed(Lang::En);
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    s.undo();
    type_word(&mut s, "ghbdsn");
    assert!(s.on_boundary(' ').actions.is_empty());
    // other words are still corrected
    type_word(&mut s, "ckfdf");
    assert_eq!(s.on_boundary(' ').actions, act(5, "слава", Lang::Uk, " "));
}

#[test]
fn slow_double_shift_does_nothing() {
    let mut s = Session::default();
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    s.on_shift_tap(1000);
    assert!(s.on_shift_tap(1600).actions.is_empty());
    assert!(s.can_undo());
}

#[test]
fn undo_only_right_after_the_correction() {
    let mut s = Session::default();
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    type_word(&mut s, "z"); // started the next word
    assert!(!s.can_undo());
    assert!(s.undo().actions.is_empty());

    let mut s = Session::default();
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    s.on_reset(); // clicked somewhere
    assert!(!s.can_undo());
}

#[test]
fn shift_taps_around_typing_do_not_undo() {
    let mut s = Session::default();
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    s.on_shift_tap(1000);
    s.on_backspace();
    assert!(s.on_shift_tap(1100).actions.is_empty());
}

#[test]
fn optional_backspace_undo() {
    let mut s = Session::default();
    s.undo_on_backspace = true;
    type_word(&mut s, "ghbdsn");
    s.on_boundary(' ');
    assert_eq!(s.on_backspace().actions, act(7, "ghbdsn", Lang::En, " "));
}

#[test]
fn backspace_inside_a_word_edits_it() {
    let mut s = Session::default();
    type_word(&mut s, "ghbdsnx");
    s.on_backspace();
    assert_eq!(s.on_boundary(' ').actions, act(6, "привіт", Lang::Uk, " "));
}
