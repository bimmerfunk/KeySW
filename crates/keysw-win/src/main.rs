//! KeySW for Windows: a tray app that watches typing and fixes words typed
//! in the wrong layout. See `app.rs` for the WinAPI side.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg_attr(not(windows), allow(dead_code))]
mod keymap;

fn main() {
    #[cfg(windows)]
    app::run();
    #[cfg(not(windows))]
    eprintln!("keysw-win runs on Windows only; on this OS try `keysw` (the CLI).");
}
