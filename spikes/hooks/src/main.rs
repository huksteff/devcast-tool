//! Spike 1.9: low-level mouse/keyboard hooks on their own message-loop thread, QPC timestamps,
//! privacy filter (only modifier combos and special keys are kept), cursor images.
//! Synthetic input goes only to our own test window on the LEFT monitor.
//!
//! Usage: spike-hooks <out_dir>

mod cursor;

use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Performance::QueryPerformanceCounter;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

#[derive(Debug)]
enum Event {
    Mouse { msg: u32, x: i32, y: i32, qpc: i64 },
    Combo { keys: String, qpc: i64 },
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static MODS: Mutex<[bool; 4]> = Mutex::new([false; 4]); // ctrl, shift, alt, win
static PLAIN_DROPPED: Mutex<u32> = Mutex::new(0);

fn qpc() -> i64 {
    let mut v = 0;
    // SAFETY: writes the counter into `v`.
    let _ = unsafe { QueryPerformanceCounter(&mut v) };
    v
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_MOUSE_LL, lparam points to an MSLLHOOKSTRUCT owned by the system.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if let Ok(mut events) = EVENTS.lock() {
            events.push(Event::Mouse { msg: wparam.0 as u32, x: info.pt.x, y: info.pt.y, qpc: qpc() });
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn key_name(vk: u16) -> Option<String> {
    let name = match VIRTUAL_KEY(vk) {
        VK_ESCAPE => "Esc".to_owned(),
        VK_RETURN => "Enter".to_owned(),
        VK_TAB => "Tab".to_owned(),
        VK_BACK => "Backspace".to_owned(),
        VK_DELETE => "Delete".to_owned(),
        VK_LEFT => "←".to_owned(),
        VK_RIGHT => "→".to_owned(),
        VK_UP => "↑".to_owned(),
        VK_DOWN => "↓".to_owned(),
        k if (VK_F1.0..=VK_F24.0).contains(&k.0) => format!("F{}", k.0 - VK_F1.0 + 1),
        k if (0x30..=0x5A).contains(&k.0) => char::from(k.0 as u8).to_string(),
        _ => return None,
    };
    Some(name)
}

/// Keeps the event only if it is a shortcut: a modifier other than Shift is held, or the key
/// itself is special (Esc, F-keys, arrows…). Plain typing (letters, digits, Shift+letter) is dropped.
fn on_key_down(vk: u16, qpc: i64) {
    let Ok(mods) = MODS.lock() else { return };
    let [ctrl, shift, alt, win] = *mods;
    let Some(name) = key_name(vk) else { return };
    let special = name.chars().count() > 1;
    if !(ctrl || alt || win || special) {
        if let Ok(mut n) = PLAIN_DROPPED.lock() {
            *n += 1;
        }
        return;
    }
    let mut keys: Vec<&str> = Vec::new();
    for (on, label) in [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift"), (win, "Win")] {
        if on {
            keys.push(label);
        }
    }
    let combo = format!("{}{}{}", keys.join("+"), if keys.is_empty() { "" } else { "+" }, name);
    if let Ok(mut events) = EVENTS.lock() {
        events.push(Event::Combo { keys: combo, qpc });
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT owned by the system.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let vk = VIRTUAL_KEY(info.vkCode as u16);
        let slot = match vk {
            VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Some(0),
            VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Some(1),
            VK_MENU | VK_LMENU | VK_RMENU => Some(2),
            VK_LWIN | VK_RWIN => Some(3),
            _ => None,
        };
        match slot {
            Some(i) => {
                if let Ok(mut mods) = MODS.lock() {
                    mods[i] = down;
                }
            }
            None if down => on_key_down(vk.0, qpc()),
            None => {}
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: flags, ..Default::default() } } }
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: flags, ..Default::default() } } }
}

/// Runs on a separate thread: clicks into our window, then types plain text and shortcuts.
fn inject(hook_thread: u32) {
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: plain Win32 input calls; the cursor is put over our own topmost test window.
    unsafe {
        let _ = SetCursorPos(-1300, 650);
        let mut clicks = Vec::new();
        for _ in 0..50 {
            clicks.push(mouse_input(MOUSEEVENTF_LEFTDOWN));
            clicks.push(mouse_input(MOUSEEVENTF_LEFTUP));
        }
        let sent = if std::env::var("SPIKE_BATCH").is_ok() {
            SendInput(&clicks, size_of::<INPUT>() as i32)
        } else {
            let mut n = 0;
            for pair in clicks.chunks(2) {
                n += SendInput(&pair[..1], size_of::<INPUT>() as i32);
                std::thread::sleep(Duration::from_millis(10));
                n += SendInput(&pair[1..], size_of::<INPUT>() as i32);
                std::thread::sleep(Duration::from_millis(20));
            }
            n
        };
        println!("injected {sent} mouse events");
        std::thread::sleep(Duration::from_millis(200));
        let mut keys = Vec::new();
        for vk in [VIRTUAL_KEY(0x41), VIRTUAL_KEY(0x42), VIRTUAL_KEY(0x43)] {
            keys.push(key_input(vk, false));
            keys.push(key_input(vk, true));
        }
        keys.push(key_input(VK_SHIFT, false));
        keys.push(key_input(VIRTUAL_KEY(0x44), false));
        keys.push(key_input(VIRTUAL_KEY(0x44), true));
        keys.push(key_input(VK_SHIFT, true));
        for (a, b, k) in [(VK_CONTROL, VK_SHIFT, VIRTUAL_KEY(0x50))] {
            keys.extend([key_input(a, false), key_input(b, false), key_input(k, false)]);
            keys.extend([key_input(k, true), key_input(b, true), key_input(a, true)]);
        }
        keys.extend([key_input(VK_F5, false), key_input(VK_F5, true), key_input(VK_ESCAPE, false), key_input(VK_ESCAPE, true)]);
        let sent = SendInput(&keys, size_of::<INPUT>() as i32);
        println!("injected {sent} keyboard events (abc, Shift+D, Ctrl+Shift+P, F5, Esc)");
        std::thread::sleep(Duration::from_millis(300));
        let _ = PostThreadMessageW(hook_thread, WM_QUIT, WPARAM(0), LPARAM(0));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    // SAFETY: standard window class registration, window creation, hooks and message loop
    // on this thread; all handles are owned by this process and released at the end.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: w!("spike-hooks"),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd: HWND = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("spike-hooks"),
            w!("spike-hooks test target"),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            -1500,
            500,
            400,
            300,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(instance.into()), 0)?;
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(instance.into()), 0)?;
        let thread = GetCurrentThreadId();
        let injector = std::thread::spawn(move || inject(thread));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = injector.join();
        UnhookWindowsHookEx(mouse)?;
        UnhookWindowsHookEx(keyboard)?;
        DestroyWindow(hwnd)?;
    }
    report();
    cursor::save_cursor_images(&out)?;
    Ok(())
}

fn report() {
    let Ok(events) = EVENTS.lock() else { return };
    let downs = events.iter().filter(|e| matches!(e, Event::Mouse { msg: WM_LBUTTONDOWN, .. })).count();
    let ups = events.iter().filter(|e| matches!(e, Event::Mouse { msg: WM_LBUTTONUP, .. })).count();
    let first_click = events.iter().find_map(|e| match e {
        Event::Mouse { msg: WM_LBUTTONDOWN, x, y, qpc } => Some((*x, *y, *qpc)),
        _ => None,
    });
    println!("mouse: {downs} left downs, {ups} left ups; first click at {first_click:?}");
    let mut kinds: Vec<(u32, usize)> = Vec::new();
    for e in events.iter() {
        if let Event::Mouse { msg, .. } = e {
            match kinds.iter_mut().find(|(m, _)| m == msg) {
                Some((_, n)) => *n += 1,
                None => kinds.push((*msg, 1)),
            }
        }
    }
    println!("mouse message counts (msg id, count): {kinds:x?}");
    let seq: String = events.iter().filter_map(|e| match e {
        Event::Mouse { msg: WM_LBUTTONDOWN, .. } => Some('D'),
        Event::Mouse { msg: WM_LBUTTONUP, .. } => Some('U'),
        _ => None,
    }).collect();
    println!("sequence: {seq}");
    let combos: Vec<&str> = events.iter().filter_map(|e| match e {
        Event::Combo { keys, .. } => Some(keys.as_str()),
        Event::Mouse { .. } => None,
    }).collect();
    let dropped = PLAIN_DROPPED.lock().map(|n| *n).unwrap_or(0);
    println!("keys kept: {combos:?}; plain keys dropped (never stored): {dropped}");
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: default handling for our test window.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
