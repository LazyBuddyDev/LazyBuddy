//! Global shortcuts stay active when all content is silent.
use tauri::WebviewWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
pub fn register(window: &WebviewWindow, shortcut: crate::shortcut::Shortcut) -> Result<(), String> {
    if lazybuddy_host_control::isolated() {
        return Ok(());
    }
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
    static CURRENT: std::sync::Mutex<Option<crate::shortcut::Shortcut>> =
        std::sync::Mutex::new(None);
    let mut current = CURRENT.lock().unwrap();
    if *current == Some(shortcut) {
        return Ok(());
    }
    unsafe {
        UnregisterHotKey(hwnd, 0x4c42);
    }
    if !shortcut.register(hwnd, 0x4c42) {
        if let Some(old) = *current {
            old.register(hwnd, 0x4c42);
        }
        return Err("全局快捷键已被占用".into());
    }
    *current = Some(shortcut);
    Ok(())
}
