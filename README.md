# KeySW

Автоматичне перемикання розкладки для Windows і macOS, відкритий аналог Punto Switcher.
Набрали `ghbdsn` в англійській розкладці? KeySW виправить на `привіт` і перемкне мову.

*Automatic keyboard layout switcher for Windows and macOS (English ↔ Ukrainian / Russian), an open-source Punto Switcher alternative. English below.*

## Як це працює

- Англійська → українська або російська: обирається та кирилична розкладка, якою ви користувались **останньою**.
- Українська / російська → англійська.
- Рішення приймається для кожного слова після пробілу: спершу словник (30 000 найчастіших слів кожної мови), потім статистика сполучень літер (триграми), щоб розпізнавати й слова, яких немає в словнику.
- Не чіпає слова з цифрами, змішаний регістр (`iPhone`), одиночні літери та поширені скорочення (`db`, `ui`, `ok` …).
- Усе працює локально, набраний текст нікуди не надсилається.

## Стан

| Частина | Стан |
|---|---|
| Ядро розпізнавання (`crates/keysw-core`) | ✅ готово, з тестами |
| Програма для Windows | ⏳ наступний крок |
| Програма для macOS | ⏳ |
| Збірки в Releases | ⏳ |

## Спробувати ядро

Потрібен [Rust](https://rustup.rs).

```sh
cargo run --release -p keysw-cli -- ghbdsn ckfdf            # → привіт слава
cargo run --release -p keysw-cli -- --last ru ghbdtn         # → привет
cargo run --release -p keysw-cli -- --layout uk руддщ цщкдв  # → hello world
echo "Z gbie yjde ghjuhfve" | cargo run --release -p keysw-cli  # → Я пишу нову програму
```

`--explain` показує, на основі чого прийнято рішення.

## Розробка

```sh
cargo test --release
# якість на словах, яких немає в словнику:
cargo run --release -p keysw-core --example eval -- en words_en.txt uk words_uk.txt ru words_ru.txt
```

Словники у `crates/keysw-core/data/` згенеровані `tools/build_dict.py` зі списків
[hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) (OpenSubtitles 2018, CC-BY-SA-4.0).
З українського списку відфільтровано російські слова, які потрапили туди із субтитрів.

---

## English

KeySW watches what you type and, when a word was clearly typed in the wrong layout,
replaces it and switches the layout. English-layout mistakes go to whichever Cyrillic
layout (Ukrainian or Russian) you used last; Cyrillic-layout mistakes go to English.

The core engine (`crates/keysw-core`) is pure Rust with no OS dependencies: a
word-frequency dictionary plus a character trigram model per language. The Windows
and macOS apps (in progress) feed it key presses and apply the corrections it returns.

Try it: `cargo run --release -p keysw-cli -- ghbdsn` prints `привіт`.

Code is MIT-licensed. The word lists are derived from
[FrequencyWords](https://github.com/hermitdave/FrequencyWords) (OpenSubtitles, CC-BY-SA-4.0).
