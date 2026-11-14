//! Global shortcuts stay active when all content is silent.
use tauri::WebviewWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
static CURRENT: std::sync::Mutex<Option<crate::shortcut::Shortcut>> = std::sync::Mutex::new(None);
static CAPTURING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn unregister(window: &WebviewWindow) -> Result<(), String> {
    CAPTURING.store(true, std::sync::atomic::Ordering::Relaxed);
    if lazybuddy_host_control::isolated() {
        return Ok(());
    }
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
    unsafe {
        UnregisterHotKey(hwnd, 0x4c42);
    }
    *CURRENT.lock().unwrap() = None;
    Ok(())
}
pub fn capturing() -> bool {
    CAPTURING.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn register(window: &WebviewWindow, shortcut: crate::shortcut::Shortcut) -> Result<(), String> {
    if lazybuddy_host_control::isolated() || CAPTURING.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
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
pub fn resume(window: &WebviewWindow, shortcut: crate::shortcut::Shortcut) -> Result<(), String> {
    CAPTURING.store(false, std::sync::atomic::Ordering::Relaxed);
    register(window, shortcut)
}
