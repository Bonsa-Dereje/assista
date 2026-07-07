// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//
// This file wires up the commands referenced from the Svelte overlay:
//   - set_keyboard_lock(locked)         -> swallow keystrokes system-wide
//                                          except the app's own escape hotkey
//   - set_touchpad_lock(locked)         -> swallow pointer input system-wide
//   - send_key(key, ctrl, alt, shift)   -> inject a synthetic keystroke into
//                                          whatever window currently has OS
//                                          focus (i.e. NOT this app)
//   - send_modified_key(key, shift)     -> send key with shift modifier
//   - send_alt_tab()                    -> send Alt+Tab to switch windows
//   - mouse_click_down()                -> hold left mouse button
//   - mouse_click_up()                  -> release left mouse button
//   - undo_shortcut()                   -> send Ctrl+Z
//   - redo_shortcut()                   -> send Ctrl+Y
//
// All are implemented with Windows low-level hooks / SendInput since that's
// the only reliable, no-driver way to intercept or inject input globally on
// Windows. On other platforms the commands compile and return a clear
// "unsupported" error instead of silently doing nothing.
//
// ----------------------------------------------------------------------
// Why send_key needs the window-focus fix below:
// ----------------------------------------------------------------------
// SendInput() delivers keystrokes to whichever HWND currently has OS
// keyboard focus. Clicking a button in the on-screen keyboard's webview
// normally activates that window first (standard Windows click behavior),
// which STEALS focus away from the app the user was typing into — so the
// injected key would just loop back into our own keyboard window.
//
// The fix is to make the "keyboard" window a non-activating window:
//   1. WS_EX_NOACTIVATE extended style, set once at startup.
//   2. Handle WM_MOUSEACTIVATE and return MA_NOACTIVATE, because
//      WS_EX_NOACTIVATE alone doesn't stop activation from a mouse click.
//   3. Show it with ShowWindow(hwnd, SW_SHOWNA) instead of Tauri's normal
//      window.show(), since SW_SHOW can still activate a window on first
//      display even with the style bit set.
// With all three in place, the target app keeps OS focus the entire time
// the on-screen keyboard is being clicked, and send_key's SendInput calls
// land where the user was actually typing.
//
// Cargo.toml needs, in addition to what `tauri` already pulls in:
//
// [target.'cfg(windows)'.dependencies]
// windows = { version = "0.58", features = [
//     "Win32_Foundation",
//     "Win32_UI_WindowsAndMessaging",
//     "Win32_System_LibraryLoader",
//     "Win32_System_Threading",
//     "Win32_UI_Input_KeyboardAndMouse",
// ] }

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Manager, State};

/// Shared app state tracking lock status + the handle needed to tear a hook
/// down again. Kept behind a Mutex because the hook lives on its own thread.
pub struct InputLockState {
    keyboard_locked: AtomicBool,
    touchpad_locked: AtomicBool,
    #[cfg(target_os = "windows")]
    keyboard_hook: Mutex<Option<win::HookHandle>>,
    #[cfg(target_os = "windows")]
    mouse_hook: Mutex<Option<win::HookHandle>>,
}

impl Default for InputLockState {
    fn default() -> Self {
        Self {
            keyboard_locked: AtomicBool::new(false),
            touchpad_locked: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            keyboard_hook: Mutex::new(None),
            #[cfg(target_os = "windows")]
            mouse_hook: Mutex::new(None),
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Locks/unlocks the keyboard. While locked, all keystrokes are swallowed
/// system-wide EXCEPT the built-in escape hotkey (Ctrl+Alt+Shift+U), which
/// always force-unlocks and notifies the frontend so the UI never gets
/// stuck with no way back in.
#[tauri::command]
fn set_keyboard_lock(
    locked: bool,
    app: AppHandle,
    state: State<'_, InputLockState>,
) -> Result<(), String> {
    state.keyboard_locked.store(locked, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    {
        let mut guard = state.keyboard_hook.lock().map_err(|e| e.to_string())?;
        if locked {
            if guard.is_none() {
                *guard = Some(win::install_keyboard_hook(app.clone())?);
            }
        } else if let Some(handle) = guard.take() {
            win::uninstall_hook(handle)?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Keyboard locking is only implemented on Windows right now.".into())
    }
}

/// Locks/unlocks pointer input (mouse + precision touchpad). Windows doesn't
/// expose a clean "this event came from the touchpad" flag through
/// WH_MOUSE_LL, so this swallows all pointer input while locked rather than
/// trying to selectively disable just the touchpad HID device — the
/// on-screen toolbar/keyboard stays reachable via the keyboard escape
/// hotkey, and unlocking always restores normal mouse behavior.
#[tauri::command]
fn set_touchpad_lock(
    locked: bool,
    app: AppHandle,
    state: State<'_, InputLockState>,
) -> Result<(), String> {
    state.touchpad_locked.store(locked, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    {
        let mut guard = state.mouse_hook.lock().map_err(|e| e.to_string())?;
        if locked {
            if guard.is_none() {
                *guard = Some(win::install_mouse_hook(app.clone())?);
            }
        } else if let Some(handle) = guard.take() {
            win::uninstall_hook(handle)?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Touchpad locking is only implemented on Windows right now.".into())
    }
}

/// Injects a synthetic keystroke into whatever window currently has OS
/// keyboard focus. `ctrl`/`alt`/`shift` are held down around the key so the
/// on-screen keyboard's sticky-modifier buttons work as chords (e.g. toggle
/// Shift, then tap 'a' -> types 'A').
#[tauri::command]
fn send_key(key: String, ctrl: bool, alt: bool, shift: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key(&key, ctrl, alt, shift)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (key, ctrl, alt, shift);
        Err("Key injection is only implemented on Windows right now.".into())
    }
}

/// Sends a key with shift modifier (for selection via Shift+Arrow keys)
#[tauri::command]
fn send_modified_key(key: String, shift: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key(&key, false, false, shift)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (key, shift);
        Err("Key injection is only implemented on Windows right now.".into())
    }
}

/// Sends the OS "copy" shortcut (Ctrl+C) into whatever window currently has
/// OS keyboard focus, via the same non-activating SendInput path as
/// `send_key` — so clicking the toolbar's Copy button never steals focus
/// away from the app the user was actually working in.
#[tauri::command]
fn copy_shortcut() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key("c", true, false, false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Copy is only implemented on Windows right now.".into())
    }
}

/// Sends the OS "paste" shortcut (Ctrl+V) into whatever window currently has
/// OS keyboard focus. Same rationale as `copy_shortcut`.
#[tauri::command]
fn paste_shortcut() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key("v", true, false, false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Paste is only implemented on Windows right now.".into())
    }
}

/// Sends the OS "undo" shortcut (Ctrl+Z)
#[tauri::command]
fn undo_shortcut() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key("z", true, false, false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Undo is only implemented on Windows right now.".into())
    }
}

/// Sends the OS "redo" shortcut (Ctrl+Y)
#[tauri::command]
fn redo_shortcut() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_key("y", true, false, false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Redo is only implemented on Windows right now.".into())
    }
}

/// Sends Alt+Tab to switch windows
#[tauri::command]
fn send_alt_tab() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::send_alt_tab()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Alt+Tab is only implemented on Windows right now.".into())
    }
}

/// Holds down the left mouse button (for drag operations)
#[tauri::command]
fn mouse_click_down() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::mouse_click_down()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Mouse click is only implemented on Windows right now.".into())
    }
}

/// Releases the left mouse button
#[tauri::command]
fn mouse_click_up() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::mouse_click_up()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Mouse click is only implemented on Windows right now.".into())
    }
}

/// Shows the on-screen keyboard window WITHOUT giving it OS focus, so the
/// app the user was typing into stays focused. Must be used instead of the
/// frontend calling the window's own `.show()`, which can activate it on
/// first display even with WS_EX_NOACTIVATE set.
#[tauri::command]
fn show_keyboard_noactivate(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("keyboard")
        .ok_or_else(|| "keyboard window not found".to_string())?;

    #[cfg(target_os = "windows")]
    {
        win::show_noactivate(&window)
    }

    #[cfg(not(target_os = "windows"))]
    {
        window.show().map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(InputLockState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            set_keyboard_lock,
            set_touchpad_lock,
            send_key,
            send_modified_key,
            copy_shortcut,
            paste_shortcut,
            undo_shortcut,
            redo_shortcut,
            send_alt_tab,
            mouse_click_down,
            mouse_click_up,
            show_keyboard_noactivate
        ])
        .setup(|app| {
            // Make BOTH windows non-activating up front. The keyboard window
            // needed this so clicking a key doesn't steal focus; the main
            // toolbar window needs the exact same treatment, since clicking
            // its arrow/lock/etc buttons was activating it and stealing
            // focus away from whatever the user was actually directing
            // input at. Neither window ever needs to hold text-input focus
            // itself, so this is safe for both.
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    win::make_noactivate(&window)?;
                }
                if let Some(window) = app.get_webview_window("keyboard") {
                    win::make_noactivate(&window)?;
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================================
// Windows-only hook / injection / non-activating-window implementation
// ============================================================================
#[cfg(target_os = "windows")]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;
    use std::thread::JoinHandle;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, MOUSE_EVENT_FLAGS,
        VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DOWN, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT,
        VK_SHIFT, VK_SPACE, VK_UP, VK_TAB,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, CallWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
        PostThreadMessageW, SetWindowLongPtrW, SetWindowsHookExW, ShowWindow, TranslateMessage,
        UnhookWindowsHookEx, GWLP_WNDPROC, GWL_EXSTYLE, HHOOK, KBDLLHOOKSTRUCT, MSG,
        MSLLHOOKSTRUCT, SW_SHOWNA, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_MOUSEACTIVATE,
        WM_QUIT, WM_SYSKEYDOWN, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    // Virtual-key codes for the escape hotkey: Ctrl + Alt + Shift + U.
    const VK_CONTROL_RAW: u32 = 0x11;
    const VK_MENU_RAW: u32 = 0x12; // Alt
    const VK_SHIFT_RAW: u32 = 0x10;
    const VK_U: u32 = 0x55;

    const MA_NOACTIVATE: LRESULT = LRESULT(3);

    // Hook procs run on the thread that installed them and can't easily
    // capture state, so the AppHandle (used only to emit an unlock event to
    // the frontend) is stashed in a process-wide static.
    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    static KEY_DOWN_MASK: AtomicIsize = AtomicIsize::new(0);

    const CTRL_BIT: isize = 1 << 0;
    const ALT_BIT: isize = 1 << 1;
    const SHIFT_BIT: isize = 1 << 2;

    // The keyboard window's original WNDPROC, saved so our subclass proc can
    // forward everything except WM_MOUSEACTIVATE back to it unchanged.
    static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

    /// Handle to a running hook thread so it can be torn down cleanly later.
    pub struct HookHandle {
        thread_id: u32,
        join: JoinHandle<()>,
    }

    pub fn uninstall_hook(handle: HookHandle) -> Result<(), String> {
        // Posting WM_QUIT breaks the message loop, which unhooks and exits.
        unsafe {
            PostThreadMessageW(handle.thread_id, WM_QUIT, WPARAM(0), LPARAM(0))
                .map_err(|e| e.to_string())?;
        }
        handle.join.join().map_err(|_| "hook thread panicked".to_string())?;
        Ok(())
    }

    pub fn install_keyboard_hook(app: AppHandle) -> Result<HookHandle, String> {
        let _ = APP_HANDLE.set(app);
        spawn_hook_thread(WH_KEYBOARD_LL, keyboard_hook_proc)
    }

    pub fn install_mouse_hook(app: AppHandle) -> Result<HookHandle, String> {
        let _ = APP_HANDLE.set(app);
        spawn_hook_thread(WH_MOUSE_LL, mouse_hook_proc)
    }

    fn spawn_hook_thread(
        hook_id: windows::Win32::UI::WindowsAndMessaging::WINDOWS_HOOK_ID,
        proc: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
    ) -> Result<HookHandle, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<u32, String>>();

        let join = std::thread::spawn(move || unsafe {
            let module = match GetModuleHandleW(None) {
                Ok(h) => HINSTANCE(h.0),
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };

            let hook: HHOOK = match SetWindowsHookExW(hook_id, Some(proc), Some(module), 0) {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
            };

            let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
            let _ = tx.send(Ok(thread_id));

            // Pump messages until WM_QUIT (sent by uninstall_hook) arrives.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
        });

        match rx.recv() {
            Ok(Ok(thread_id)) => Ok(HookHandle { thread_id, join }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => {
                let _ = join.join();
                Err("hook thread exited before initializing".into())
            }
        }
    }

    /// Swallows all keystrokes while locked, except it always lets the
    /// Ctrl+Alt+Shift+U combo through and uses it as a force-unlock signal
    /// sent back to the frontend via the "input-lock://force-unlock" event.
    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            let data = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = data.vkCode;

            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let mut mask = KEY_DOWN_MASK.load(Ordering::SeqCst);
                match vk {
                    VK_CONTROL_RAW => mask |= CTRL_BIT,
                    VK_MENU_RAW => mask |= ALT_BIT,
                    VK_SHIFT_RAW => mask |= SHIFT_BIT,
                    _ => {}
                }
                KEY_DOWN_MASK.store(mask, Ordering::SeqCst);

                let escape_combo = mask & (CTRL_BIT | ALT_BIT | SHIFT_BIT)
                    == (CTRL_BIT | ALT_BIT | SHIFT_BIT)
                    && vk == VK_U;

                if escape_combo {
                    if let Some(app) = APP_HANDLE.get() {
                        let _ = app.emit("input-lock://force-unlock", ());
                    }
                    // Let this one through so the OS/app both see it end cleanly.
                    return CallNextHookEx(None, code, wparam, lparam);
                }
            } else {
                // Key-up: clear the corresponding modifier bit.
                let mut mask = KEY_DOWN_MASK.load(Ordering::SeqCst);
                match vk {
                    VK_CONTROL_RAW => mask &= !CTRL_BIT,
                    VK_MENU_RAW => mask &= !ALT_BIT,
                    VK_SHIFT_RAW => mask &= !SHIFT_BIT,
                    _ => {}
                }
                KEY_DOWN_MASK.store(mask, Ordering::SeqCst);
            }

            // Swallow everything else: return non-zero without calling
            // CallNextHookEx so the event never reaches other apps.
            return LRESULT(1);
        }

        CallNextHookEx(None, code, wparam, lparam)
    }

    /// Swallows all mouse/touchpad input while locked.
    unsafe extern "system" fn mouse_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let _data = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            return LRESULT(1);
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    // ------------------------------------------------------------------
    // Key injection (SendInput)
    // ------------------------------------------------------------------

    /// Maps an on-screen-keyboard label to a Windows virtual-key code.
    /// Falls back to VkKeyScanW for ordinary characters (letters, digits,
    /// punctuation), which also tells us whether that key normally needs
    /// Shift held (not used here since our labels are already base keys,
    /// but kept so behavior matches the real keyboard driver).
    fn resolve_vk(key: &str) -> Option<VIRTUAL_KEY> {
        match key {
            "⌫" => Some(VK_BACK),
            "↵" => Some(VK_RETURN),
            "␣" => Some(VK_SPACE),
            "←" => Some(VK_LEFT),
            "→" => Some(VK_RIGHT),
            "↑" => Some(VK_UP),
            "↓" => Some(VK_DOWN),
            "⇧" => Some(VK_SHIFT),
            "Ctrl" => Some(VK_CONTROL),
            "Alt" => Some(VK_MENU),
            _ => {
                let ch = key.chars().next()?;
                let scan = unsafe { VkKeyScanW(ch as u16) };
                if scan == -1 {
                    None
                } else {
                    Some(VIRTUAL_KEY((scan as u16) & 0xFF))
                }
            }
        }
    }

    fn key_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if key_up { KEYEVENTF_KEYUP } else { Default::default() },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Sends `key` as a synthetic keystroke, holding down whichever of
    /// ctrl/alt/shift are true around it (so the on-screen keyboard's
    /// sticky-modifier toggles act like a real chord, e.g. Shift+A).
    pub fn send_key(key: &str, ctrl: bool, alt: bool, shift: bool) -> Result<(), String> {
        let vk = resolve_vk(key).ok_or_else(|| format!("unmapped key: {key}"))?;

        let mut inputs = Vec::with_capacity(8);
        if ctrl {
            inputs.push(key_input(VK_CONTROL, false));
        }
        if alt {
            inputs.push(key_input(VK_MENU, false));
        }
        if shift {
            inputs.push(key_input(VK_SHIFT, false));
        }
        inputs.push(key_input(vk, false));
        inputs.push(key_input(vk, true));
        if shift {
            inputs.push(key_input(VK_SHIFT, true));
        }
        if alt {
            inputs.push(key_input(VK_MENU, true));
        }
        if ctrl {
            inputs.push(key_input(VK_CONTROL, true));
        }

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err("SendInput did not deliver all events".into());
        }
        Ok(())
    }

    /// Sends Alt+Tab to switch windows
    pub fn send_alt_tab() -> Result<(), String> {
        let mut inputs = Vec::with_capacity(4);
        inputs.push(key_input(VK_MENU, false));     // Alt down
        inputs.push(key_input(VK_TAB, false));      // Tab down
        inputs.push(key_input(VK_TAB, true));       // Tab up
        inputs.push(key_input(VK_MENU, true));      // Alt up

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err("SendInput did not deliver all events".into());
        }
        Ok(())
    }

    /// Holds down the left mouse button
    pub fn mouse_click_down() -> Result<(), String> {
        let input = mouse_input(MOUSEEVENTF_LEFTDOWN);
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err("SendInput did not deliver mouse down event".into());
        }
        Ok(())
    }

    /// Releases the left mouse button
    pub fn mouse_click_up() -> Result<(), String> {
        let input = mouse_input(MOUSEEVENTF_LEFTUP);
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err("SendInput did not deliver mouse up event".into());
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Non-activating window (so clicking the on-screen keyboard never
    // steals OS focus away from whatever the user was typing into)
    // ------------------------------------------------------------------

    /// Adds WS_EX_NOACTIVATE (+ WS_EX_TOOLWINDOW to also keep it out of
    /// Alt+Tab) and installs a WNDPROC subclass that answers
    /// WM_MOUSEACTIVATE with MA_NOACTIVATE. Call once, at startup, before
    /// the window is ever shown.
    pub fn make_noactivate(window: &tauri::WebviewWindow) -> tauri::Result<()> {
        let hwnd = window.hwnd()?;
        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex_style | (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize),
            );

            let old_proc = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc as isize);
            ORIGINAL_WNDPROC.store(old_proc, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Shows the window via ShowWindow(SW_SHOWNA) — "show, no activate" —
    /// instead of Tauri's normal show(), which can still hand it focus on
    /// first display even with WS_EX_NOACTIVATE set.
    pub fn show_noactivate(window: &tauri::WebviewWindow) -> Result<(), String> {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
        Ok(())
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_MOUSEACTIVATE {
            // Tell Windows: activate nothing, but still let the click
            // through to the control under the cursor.
            return MA_NOACTIVATE;
        }

        let old = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
        if old != 0 {
            let old_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                std::mem::transmute(old);
            CallWindowProcW(Some(old_proc), hwnd, msg, wparam, lparam)
        } else {
            LRESULT(0)
        }
    }
}