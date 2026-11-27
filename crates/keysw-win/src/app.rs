//! The Windows side: a low-level keyboard hook feeds key presses to the
//! core [`Session`]; its actions are replayed with `SendInput`; a tray icon
//! turns the app on and off.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use keysw_core::{key_char, Action, Key, Lang, Response, Session};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, GetKeyboardLayoutList, SendInput, HKL, INPUT,
    INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RETURN,
    VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::keymap::{lang_to_primary_langid, langid_to_lang, scan_to_base};

const WM_APP_RUN: u32 = WM_APP + 1;
const WM_APP_TRAY: u32 = WM_APP + 2;
const ID_TOGGLE: usize = 1;
const ID_EXIT: usize = 2;
/// After we switch the layout ourselves, the foreground window may report
/// the old one for a moment; trust our switch for this long.
const SWITCH_GRACE: Duration = Duration::from_millis(600);

struct State {
    session: Session,
    enabled: bool,
    hwnd: HWND,
    /// Shift is down and no other key was pressed since.
    shift_clean: bool,
    /// The active layout is one we don't handle (e.g. German): stay out of the way.
    foreign_layout: bool,
    switch_guard: Option<(Lang, Instant)>,
    pending: VecDeque<Action>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn run() {
    unsafe {
        // One copy at a time.
        let _mutex = CreateMutexW(None, true, w!("KeySW.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let class = w!("KeySWWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("KeySW"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        )
        .expect("create window");

        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                session: Session::default(),
                enabled: true,
                hwnd,
                shift_clean: false,
                foreign_layout: false,
                switch_guard: None,
                pending: VecDeque::new(),
            })
        });
        with_state(sync_layout);

        tray(hwnd, NIM_ADD, true);

        let kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(hinstance), 0)
            .expect("keyboard hook");
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(hinstance), 0)
            .expect("mouse hook");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(kb_hook);
        let _ = UnhookWindowsHookEx(mouse_hook);
        tray(hwnd, NIM_DELETE, false);
    }
}

// ---------------------------------------------------------------- hooks ---

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // Our own SendInput events come back through the hook: ignore them.
        if kb.flags.0 & LLKHF_INJECTED.0 == 0 {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            if with_state(|st| on_key(st, kb, down)).unwrap_or(false) {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
        )
    {
        // A click moves the cursor: the word being typed is gone.
        with_state(|st| st.session.on_reset());
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16) & 0x8000 != 0 }
}

/// Returns true to swallow the key.
fn on_key(st: &mut State, kb: &KBDLLHOOKSTRUCT, down: bool) -> bool {
    let vk = VIRTUAL_KEY(kb.vkCode as u16);
    let is_shift = matches!(vk, VK_SHIFT | VK_LSHIFT | VK_RSHIFT);

    if !down {
        if is_shift && std::mem::take(&mut st.shift_clean) && st.enabled {
            let r = st.session.on_shift_tap(kb.time as u64);
            queue(st, r);
        }
        return false;
    }
    if is_shift {
        st.shift_clean = true;
        return false;
    }
    st.shift_clean = false;
    if !st.enabled {
        return false;
    }
    // Ctrl / Alt / Win shortcuts are commands, not typing.
    if is_down(VK_CONTROL) || is_down(VK_MENU) || is_down(VK_LWIN) || is_down(VK_RWIN) {
        st.session.on_reset();
        return false;
    }
    sync_layout(st);
    if st.foreign_layout {
        st.session.on_reset();
        return false;
    }

    let r = match vk {
        VK_SPACE => st.session.on_boundary(' '),
        VK_RETURN => st.session.on_boundary('\n'),
        VK_TAB => st.session.on_boundary('\t'),
        VK_BACK => st.session.on_backspace(),
        _ => {
            let extended = kb.flags.0 & LLKHF_EXTENDED.0 != 0;
            match scan_to_base(kb.scanCode).filter(|_| !extended) {
                Some(base) => {
                    let mut shift = is_down(VK_SHIFT);
                    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0;
                    let lang = st.session.engine.current_layout();
                    let is_letter =
                        key_char(Key::new(base, false), lang).is_some_and(char::is_alphabetic);
                    if caps && is_letter {
                        shift = !shift;
                    }
                    st.session.on_char(Key::new(base, shift))
                }
                // Arrows, Home, Delete, F-keys, numpad …
                None => {
                    st.session.on_reset();
                    Response::default()
                }
            }
        }
    };
    queue(st, r)
}

/// Hand actions to the window procedure: sending input from inside the hook
/// would delay every key press system-wide.
fn queue(st: &mut State, r: Response) -> bool {
    if !r.actions.is_empty() {
        // Start trusting the new layout right away: the user's next key can
        // arrive before the switch below has even been requested.
        for a in &r.actions {
            if let Action::SwitchLayout(lang) = a {
                st.switch_guard = Some((*lang, Instant::now()));
            }
        }
        st.pending.extend(r.actions);
        unsafe {
            let _ = PostMessageW(Some(st.hwnd), WM_APP_RUN, WPARAM(0), LPARAM(0));
        }
    }
    r.swallow
}

// --------------------------------------------------------------- layout ---

fn foreground_hkl() -> HKL {
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        GetKeyboardLayout(tid)
    }
}

fn hkl_lang(hkl: HKL) -> Option<Lang> {
    langid_to_lang((hkl.0 as usize & 0xffff) as u16)
}

fn sync_layout(st: &mut State) {
    let lang = hkl_lang(foreground_hkl());
    if let Some((target, since)) = st.switch_guard {
        if lang != Some(target) && since.elapsed() < SWITCH_GRACE {
            return;
        }
        st.switch_guard = None;
    }
    match lang {
        Some(l) => {
            st.foreign_layout = false;
            if l != st.session.engine.current_layout() {
                st.session.layout_changed(l);
            }
        }
        None => st.foreign_layout = true,
    }
}

fn installed_layout(lang: Lang) -> Option<HKL> {
    unsafe {
        let mut list = [HKL::default(); 32];
        let n = GetKeyboardLayoutList(Some(&mut list)).max(0) as usize;
        let want = lang_to_primary_langid(lang);
        list[..n.min(list.len())]
            .iter()
            .copied()
            .find(|h| (h.0 as usize & 0x3ff) as u16 == want)
    }
}

fn switch_layout(lang: Lang) {
    let Some(hkl) = installed_layout(lang) else {
        return;
    };
    unsafe {
        let fg = GetForegroundWindow();
        let _ = PostMessageW(
            Some(fg),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(hkl.0 as isize),
        );
    }
}

// ---------------------------------------------------------------- input ---

fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn press(vk: VIRTUAL_KEY, times: usize) -> Vec<INPUT> {
    (0..times)
        .flat_map(|_| {
            [
                key_input(vk, 0, KEYBD_EVENT_FLAGS(0)),
                key_input(vk, 0, KEYEVENTF_KEYUP),
            ]
        })
        .collect()
}

fn type_text(text: &str) -> Vec<INPUT> {
    match text {
        // Real keys, so apps that react to Enter/Tab/Space key presses work.
        " " => press(VK_SPACE, 1),
        "\n" => press(VK_RETURN, 1),
        "\t" => press(VK_TAB, 1),
        _ => text
            .encode_utf16()
            .flat_map(|u| {
                [
                    key_input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE),
                    key_input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                ]
            })
            .collect(),
    }
}

fn send(inputs: &[INPUT]) {
    if !inputs.is_empty() {
        unsafe {
            SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

fn run_pending() {
    // Take the actions out first: SendInput must not run while the state is
    // borrowed, or the hook could not see the user's next key.
    let actions: Vec<Action> = with_state(|st| st.pending.drain(..).collect()).unwrap_or_default();
    for a in actions {
        match a {
            Action::Erase(n) => send(&press(VK_BACK, n)),
            Action::Type(t) => send(&type_text(&t)),
            Action::SwitchLayout(lang) => {
                switch_layout(lang);
                with_state(|st| st.switch_guard = Some((lang, Instant::now())));
            }
        }
    }
}

// ----------------------------------------------------------------- tray ---

fn wide<const N: usize>(s: &str) -> [u16; N] {
    let mut out = [0u16; N];
    for (i, u) in s.encode_utf16().take(N - 1).enumerate() {
        out[i] = u;
    }
    out
}

fn tray(hwnd: HWND, op: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE, enabled: bool) {
    unsafe {
        let icon = LoadIconW(
            None,
            if enabled {
                IDI_APPLICATION
            } else {
                IDI_WARNING
            },
        )
        .unwrap_or_default();
        let tip = if enabled {
            "KeySW: увімкнено (двічі Shift — скасувати заміну)"
        } else {
            "KeySW: вимкнено"
        };
        let data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: icon,
            szTip: wide(tip),
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(op, &data);
    }
}

fn toggle(hwnd: HWND) {
    let enabled = with_state(|st| {
        st.enabled = !st.enabled;
        st.session.on_reset();
        st.enabled
    })
    .unwrap_or(true);
    tray(hwnd, NIM_MODIFY, enabled);
}

fn show_menu(hwnd: HWND) {
    let enabled = with_state(|st| st.enabled).unwrap_or(true);
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let checked = if enabled { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, MF_STRING | checked, ID_TOGGLE, w!("Автоперемикання"));
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_GRAYED,
            0,
            w!("Скасувати заміну: двічі Shift"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Вихід"));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu closes when clicking elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        match cmd.0 as usize {
            ID_TOGGLE => toggle(hwnd),
            ID_EXIT => PostQuitMessage(0),
            _ => {}
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_RUN => {
            run_pending();
            LRESULT(0)
        }
        WM_APP_TRAY => {
            match (lparam.0 as u32) & 0xffff {
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                WM_LBUTTONDBLCLK => toggle(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
