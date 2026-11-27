//! OS-independent helpers: physical scan codes and Windows language ids.
//! Kept separate from the WinAPI code so they are unit-tested on every OS.

use keysw_core::Lang;

/// Scan code (set 1, as in `KBDLLHOOKSTRUCT::scanCode`) -> the key's
/// unshifted character on the US layout. Scan codes are physical, so this
/// works whatever layout is active.
pub fn scan_to_base(scan: u32) -> Option<char> {
    let c = match scan {
        0x29 => '`',
        0x02..=0x0A => (b'1' + (scan - 0x02) as u8) as char,
        0x0B => '0',
        0x0C => '-',
        0x0D => '=',
        0x10..=0x19 => b"qwertyuiop"[(scan - 0x10) as usize] as char,
        0x1A => '[',
        0x1B => ']',
        0x2B => '\\',
        0x1E..=0x26 => b"asdfghjkl"[(scan - 0x1E) as usize] as char,
        0x27 => ';',
        0x28 => '\'',
        0x2C..=0x32 => b"zxcvbnm"[(scan - 0x2C) as usize] as char,
        0x33 => ',',
        0x34 => '.',
        0x35 => '/',
        _ => return None,
    };
    Some(c)
}

/// Primary language id (low 10 bits of a LANGID) -> our language.
pub fn langid_to_lang(langid: u16) -> Option<Lang> {
    match langid & 0x3ff {
        0x09 => Some(Lang::En),
        0x22 => Some(Lang::Uk),
        0x19 => Some(Lang::Ru),
        _ => None,
    }
}

pub fn lang_to_primary_langid(lang: Lang) -> u16 {
    match lang {
        Lang::En => 0x09,
        Lang::Uk => 0x22,
        Lang::Ru => 0x19,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_codes() {
        assert_eq!(scan_to_base(0x10), Some('q'));
        assert_eq!(scan_to_base(0x19), Some('p'));
        assert_eq!(scan_to_base(0x1E), Some('a'));
        assert_eq!(scan_to_base(0x26), Some('l'));
        assert_eq!(scan_to_base(0x2C), Some('z'));
        assert_eq!(scan_to_base(0x32), Some('m'));
        assert_eq!(scan_to_base(0x02), Some('1'));
        assert_eq!(scan_to_base(0x0A), Some('9'));
        assert_eq!(scan_to_base(0x0B), Some('0'));
        assert_eq!(scan_to_base(0x39), None, "space is a boundary, not a char");
        // every key the core knows is reachable
        let all: String = (0..0x60).filter_map(scan_to_base).collect();
        for c in "`1234567890-=qwertyuiop[]\\asdfghjkl;'zxcvbnm,./".chars() {
            assert!(all.contains(c), "{c}");
        }
    }

    #[test]
    fn language_ids() {
        assert_eq!(langid_to_lang(0x0409), Some(Lang::En)); // en-US
        assert_eq!(langid_to_lang(0x0809), Some(Lang::En)); // en-GB
        assert_eq!(langid_to_lang(0x0422), Some(Lang::Uk));
        assert_eq!(langid_to_lang(0x0419), Some(Lang::Ru));
        assert_eq!(langid_to_lang(0x0407), None); // German
        for l in Lang::ALL {
            assert_eq!(langid_to_lang(lang_to_primary_langid(l)), Some(l));
        }
    }
}
