// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//
// This file wires up the two `TODO(rust)` commands referenced from the Svelte
// overlay:
//   - set_keyboard_lock(locked)  -> swallow keystrokes system-wide except the
//                                    app's own escape hotkey
//   - set_touchpad_lock(locked)  -> swallow pointer input system-wide
//
// Both are implemented with Windows low-level hooks (WH_KEYBOARD_LL /
// WH_MOUSE_LL) since that's the only reliable, no-driver way to intercept
// input globally on Windows. On other platforms the commands compile and
// return a clear "unsupported" error instead of silently doing nothing, so
// the frontend can decide how to surface that.
//
// Cargo.toml needs, in addition to what `tauri` already pulls in:
//
// [target.'cfg(windows)'.dependencies]
// windows = { version = "0.58", features = [
//     "Win32_Foundation",
//     "Win32_UI_WindowsAndMessaging",
//     "Win32_System_LibraryLoader",
//     "Win32_System_Threading",
// ] }

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(InputLockState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            set_keyboard_lock,
            set_touchpad_lock
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================================
// Windows-only hook implementation
// ============================================================================
#[cfg(target_os = "windows")]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;
    use std::thread::JoinHandle;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    };

    // Virtual-key codes for the escape hotkey: Ctrl + Alt + Shift + U.
    const VK_CONTROL: u32 = 0x11;
    const VK_MENU: u32 = 0x12; // Alt
    const VK_SHIFT: u32 = 0x10;
    const VK_U: u32 = 0x55;

    // Hook procs run on the thread that installed them and can't easily
    // capture state, so the AppHandle (used only to emit an unlock event to
    // the frontend) is stashed in a process-wide static.
    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    static KEY_DOWN_MASK: AtomicIsize = AtomicIsize::new(0);

    const CTRL_BIT: isize = 1 << 0;
    const ALT_BIT: isize = 1 << 1;
    const SHIFT_BIT: isize = 1 << 2;

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

            let hook: HHOOK = match SetWindowsHookExW(hook_id, Some(proc), module, 0) {
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
                    VK_CONTROL => mask |= CTRL_BIT,
                    VK_MENU => mask |= ALT_BIT,
                    VK_SHIFT => mask |= SHIFT_BIT,
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
                    VK_CONTROL => mask &= !CTRL_BIT,
                    VK_MENU => mask &= !ALT_BIT,
                    VK_SHIFT => mask &= !SHIFT_BIT,
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
}