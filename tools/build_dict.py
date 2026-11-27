#!/usr/bin/env python3
"""Build the word-frequency dictionaries that keysw-core embeds.

Source: https://github.com/hermitdave/FrequencyWords (OpenSubtitles 2018).
The Ukrainian subtitle list contains many Russian words, so we drop words
that look Russian (letters ы э ъ ё, or a frequency profile that matches the
Russian list far better than the Ukrainian one).

Usage:
    python3 tools/build_dict.py <dir with en_50k.txt uk_50k.txt ru_50k.txt>
Writes crates/keysw-core/data/{en,uk,ru}.tsv  ("word<TAB>count" per line).
"""
import os
import re
import sys

TOP = 30000
ALPHABET = {
    "en": re.compile(r"^[a-z']+$"),
    "uk": re.compile(r"^[абвгґдеєжзиіїйклмнопрстуфхцчшщьюя']+$"),
    "ru": re.compile(r"^[абвгдеёжзийклмнопрстуфхцчшщъыьэюя]+$"),
}


def load(path):
    words, total = {}, 0
    with open(path, encoding="utf-8") as f:
        for line in f:
            parts = line.split()
            if len(parts) != 2:
                continue
            w, c = parts[0].lower().replace("’", "'"), int(parts[1])
            words[w] = words.get(w, 0) + c
            total += c
    return words, total


def main(src):
    lists = {l: load(os.path.join(src, f"{l}_50k.txt")) for l in ("en", "uk", "ru")}
    uk, uk_total = lists["uk"]
    ru, ru_total = lists["ru"]

    def is_russian_in_uk(w):
        if re.search("[ыэъё]", w):
            return True
        if re.search("[іїєґ']", w):
            return False
        if w not in ru:
            return False
        # Russian words leaked into the Ukrainian list show up at ~0.4x of
        # their Russian relative frequency; genuinely shared words are ~1x.
        return (uk[w] / uk_total) < 0.6 * (ru[w] / ru_total)

    out_dir = os.path.join(os.path.dirname(__file__), "..", "crates", "keysw-core", "data")
    for lang, (words, _total) in lists.items():
        rx = ALPHABET[lang]
        kept = []
        for w, c in sorted(words.items(), key=lambda kv: -kv[1]):
            if not rx.match(w) or w.strip("'") != w:
                continue
            if lang == "uk" and is_russian_in_uk(w):
                continue
            if lang == "ru" and re.search("[іїєґ]", w):
                continue
            kept.append((w, c))
            if len(kept) >= TOP:
                break
        with open(os.path.join(out_dir, f"{lang}.tsv"), "w", encoding="utf-8") as f:
            for w, c in kept:
                f.write(f"{w}\t{c}\n")
        print(lang, len(kept))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
