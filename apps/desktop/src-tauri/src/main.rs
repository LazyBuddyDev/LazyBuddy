#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
mod dock;
mod host;
mod platform;
mod playback;
mod preferences;
mod shortcut;
mod startup;
mod tray;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Root {
    #[default]
    News,
    Hongguo,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Section {
    #[default]
    Content,
    Modules,
    Settings,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct View {
    root: Root,
    section: Section,
    visible: bool,
    generation: u64,
}
#[derive(Default)]
struct Model {
    view: View,
    feed_requests: u64,
    account_page: bool,
}
impl Model {
    fn transition(&mut self, root: Root, section: Section, visible: bool) -> View {
        if self.view.root != root || self.view.section != section || self.view.visible != visible {
            self.view.generation += 1;
            self.view.root = root;
            self.view.section = section;
            self.view.visible = visible;
        }
        self.view.clone()
    }
    #[cfg(test)]
    fn accepts(&self, generation: u64) -> bool {
        self.view.visible
            && self.view.section == Section::Content
            && self.view.root == Root::News
            && self.view.generation == generation
    }
}
#[tauri::command]
fn transition(
    root: Root,
    section: Section,
    visible: bool,
    account_page: Option<bool>,
    state: tauri::State<Mutex<Model>>,
    window: tauri::WebviewWindow,
) -> View {
    let visible = visible && !dock::is_docked(&window);
    let page = root == Root::Hongguo && section == Section::Content;
    let account = section == Section::Settings && account_page.unwrap_or(false);
    let view = {
        let mut state = state.lock().unwrap();
        if state.account_page != account {
            state.account_page = account;
            state.view.generation += 1;
        }
        state.transition(root, section, visible)
    };
    if !visible || root != Root::News || section != Section::Content {
        window.state::<host::State>().stop_news();
    }
    window
        .state::<playback::State>()
        .context(page, account && view.root == Root::Hongguo, visible);
    view
}
#[tauri::command]
fn snapshot(state: tauri::State<Mutex<Model>>, window: tauri::WebviewWindow) -> serde_json::Value {
    let state = state.lock().unwrap();
    serde_json::json!({"view":state.view,"feed_requests":state.feed_requests,
        "media_route":if playback::enabled(){"browser_service"}else{"unavailable"},"real_service":playback::enabled()&&std::env::var_os("LAZYBUDDY_BROWSER_FIXTURE_FILE").is_none(), "native_texture_integrated":false,"tray":window.app_handle().tray_by_id("shell").is_some(),"desktop":dock::snapshot(&window)})
}
fn visibility(window: &tauri::WebviewWindow, visible: bool) {
    let visible = visible && !dock::is_docked(window);
    let state = window.state::<Mutex<Model>>();
    let mut state = state.lock().unwrap();
    let view = state.view.clone();
    let changed = view.visible != visible;
    state.transition(view.root, view.section, visible);
    let page = state.view.root == Root::Hongguo && state.view.section == Section::Content;
    let account = state.account_page && state.view.root == Root::Hongguo;
    drop(state);
    window
        .state::<playback::State>()
        .context(page, account, visible);
    if !visible {
        window.state::<host::State>().stop_news();
    }
    if changed {
        let _ = window.emit("shell-visibility", visible);
    }
}
fn show_without_focus(window: &tauri::WebviewWindow) -> Result<(), String> {
    dock::expand(window).map_err(|_| "expand failed")?;
    let hwnd = window.hwnd().map_err(|_| "window unavailable")?;
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd.0 as _, 4);
    }
    visibility(window, true);
    Ok(())
}
#[tauri::command]
fn window_action(action: &str, window: tauri::WebviewWindow) -> Result<(), String> {
    match action {
        "hide" | "close" | "minimize" => {
            visibility(&window, false);
            let hwnd = window.hwnd().map_err(|_| "window unavailable")?;
            // Pair with native no-focus show; Tao still considers the initial window hidden.
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    SW_HIDE, SW_MINIMIZE, ShowWindow,
                };
                ShowWindow(
                    hwnd.0 as _,
                    if action == "minimize" {
                        SW_MINIMIZE
                    } else {
                        SW_HIDE
                    },
                );
            }
        }
        "show" => show_without_focus(&window)?,
        "expand" => dock::expand(&window).map_err(|_| "expand failed")?,
        "quit" => {
            let Some(completed) = window.state::<playback::State>().shutdown() else {
                return Ok(());
            };
            let app = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                completed.await;
                let state = app.state::<host::State>();
                state.stop_news();
                let _guard = state.exit_barrier().await;
                let _ = state.flush().await;
                app.exit(0);
            });
        }
        _ => return Err("unknown window action".into()),
    }
    Ok(())
}

#[tauri::command]
fn toggle_pin(window: tauri::WebviewWindow) -> Result<bool, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let hwnd = window.hwnd().map_err(|_| "window unavailable")?.0 as _;
    let pinned = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST == 0;
    if unsafe {
        SetWindowPos(
            hwnd,
            if pinned { HWND_TOPMOST } else { HWND_NOTOPMOST },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err("pin failed".into());
    }
    Ok(pinned)
}
#[tauri::command]
fn open_logs() -> Result<(), String> {
    let path = playback::data_directory()?.join("logs");
    std::fs::create_dir_all(&path).map_err(|_| "无法创建日志目录")?;
    std::process::Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|_| "无法打开日志目录")?;
    Ok(())
}
#[tauri::command]
fn resize(width: u32, height: u32, window: tauri::WebviewWindow) -> Result<(), String> {
    if dock::is_docked(&window) {
        return Err("expand before resizing".into());
    }
    if !(440..=1920).contains(&width) || !(440..=1200).contains(&height) {
        return Err("invalid window size".into());
    }
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|_| "resize failed".into())
}

fn main() {
    let host = host::State::new().expect("host settings unavailable");
    tauri::Builder::default()
        .manage(host)
        .manage(Mutex::new(Model::default()))
        .manage(Mutex::new(dock::Dock::default()))
        .manage(playback::State::default())
        .invoke_handler(tauri::generate_handler![
            transition,
            snapshot,
            host::module_call,
            host::news_call,
            host::settings,
            host::set_startup,
            host::open_link,
            host::check_update,
            host::apply_update,
            window_action,
            resize,
            toggle_pin,
            open_logs,
            playback::playback_call
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            dock::attach(&window)?;
            tray::install(app)?;
            host::start(app.handle().clone()).map_err(std::io::Error::other)?;
            let preferences = app.state::<host::State>().preferences();
            if let Some([x, y, w, h]) = preferences.window {
                window.set_size(tauri::PhysicalSize::new(w as u32, h as u32))?;
                window.set_position(tauri::PhysicalPosition::new(x, y))?;
            }
            if preferences.topmost {
                toggle_pin(window.clone()).map_err(std::io::Error::other)?;
            }
            if let Err(error) = platform::register(&window, preferences.shortcut) {
                let _ = window.emit("shell-error", error);
            }
            unsafe {
                // Keep the product's rectangular-window convention.
                let hwnd = window.hwnd()?;
                let corner: u32 = 1;
                windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd.0 as _,
                    33,
                    (&corner as *const u32).cast(),
                    4,
                );
            }
            if std::env::var_os("LAZYBUDDY_PROTOTYPE_PROBE").is_none()
                && !std::env::args().any(|arg| arg == "--background")
            {
                show_without_focus(&window).map_err(std::io::Error::other)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    let _ = window_action("close", webview);
                }
                return;
            }
            if let tauri::WindowEvent::Resized(_) = event {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    if dock::is_docked(&webview) {
                        let _ = dock::shape(&webview);
                    }
                    visibility(
                        &webview,
                        !window.is_minimized().unwrap_or(false)
                            && window.is_visible().unwrap_or(false),
                    );
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("LazyBuddy launch failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_entry_does_not_renew_but_a_b_a_invalidates_old_results() {
        let mut model = Model::default();
        let first = model.transition(Root::News, Section::Content, true);
        assert_eq!(
            model
                .transition(Root::News, Section::Content, true)
                .generation,
            first.generation
        );
        model.transition(Root::Hongguo, Section::Content, true);
        model.transition(Root::News, Section::Content, true);
        assert!(!model.accepts(first.generation));
        assert!(model.accepts(model.view.generation));
    }
    #[test]
    fn hidden_and_settings_reject_content_work() {
        let mut model = Model::default();
        for (section, visible) in [
            (Section::Settings, true),
            (Section::Modules, true),
            (Section::Content, false),
        ] {
            let view = model.transition(Root::News, section, visible);
            assert!(!model.accepts(view.generation));
        }
    }
}
