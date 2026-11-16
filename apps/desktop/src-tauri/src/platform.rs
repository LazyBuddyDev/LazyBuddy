//! Global shortcuts stay active when all content is silent.
use tauri::WebviewWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
static CURRENT: std::sync::Mutex<Option<crate::shortcut::Shortcut>> = std::sync::Mutex::new(None);
static CAPTURING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn pick_books(window: &WebviewWindow) -> Result<Vec<String>, String> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut buffer = vec![0u16; 32768];
    let filter: Vec<u16> = "小说文件 (*.txt;*.epub)\0*.txt;*.epub\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "导入小说\0".encode_utf16().collect();
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = window.hwnd().map_err(|_| "窗口未就绪")?.0 as _;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = buffer.len() as u32;
    dialog.Flags = OFN_EXPLORER
        | OFN_ALLOWMULTISELECT
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT;
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(vec![])
        } else {
            Err("文件选择失败，请减少所选文件后重试".into())
        };
    }
    let parts: Vec<String> = buffer
        .split(|n| *n == 0)
        .take_while(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect();
    if parts.len() == 1 {
        return Ok(parts);
    }
    if parts.len() > 101 {
        return Err("每次最多导入100本书".into());
    }
    Ok(parts[1..]
        .iter()
        .map(|p| {
            std::path::Path::new(&parts[0])
                .join(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect())
}
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
