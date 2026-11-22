#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{Emitter, Manager};
mod bilibili;
mod dock;
mod host;
mod platform;
mod playback;
mod preferences;
mod reader;
mod shortcut;
mod startup;
mod tray;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Root {
    #[default]
    News,
    Hongguo,
    Reader,
    Bilibili,
}
impl Root {
    fn from_module(id: &str) -> Option<Self> {
        Self::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(id))
            .ok()
    }

    fn accepts_manifest(manifest: &lazybuddy_module_contract::Manifest) -> bool {
        use lazybuddy_module_contract::Page;
        matches!(
            (Self::from_module(&manifest.id), manifest.page.as_ref()),
            (Some(Self::News), Some(Page::NewsGrid { .. }))
                | (Some(Self::Reader), Some(Page::BookLibrary { .. }))
                | (
                    Some(Self::Hongguo | Self::Bilibili),
                    Some(Page::MediaLibrary { .. })
                )
        )
    }
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
    active_generation: Arc<AtomicU64>,
}
impl Model {
    fn transition(&mut self, root: Root, section: Section, visible: bool) -> View {
        if self.view.root != root || self.view.section != section || self.view.visible != visible {
            self.view.generation += 1;
            self.view.root = root;
            self.view.section = section;
            self.view.visible = visible;
        }
        self.active_generation
            .store(self.view.generation, Ordering::Release);
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
    let account = section == Section::Settings && account_page.unwrap_or(false);
    let (before, view) = {
        let mut state = state.lock().unwrap();
        let before = state.view.clone();
        let before_account = state.account_page;
        if before_account != account {
            state.account_page = account;
            state.view.generation += 1;
        }
        let view = state.transition(root, section, visible);
        if before.generation != view.generation {
            apply_view(&window, &before, before_account, &view, account);
        }
        (before, view)
    };
    if before.generation != view.generation {
        lazybuddy_host_control::logs::event(
            "host",
            "view_transition",
            serde_json::json!({"view":view}),
        );
    }
    view
}

fn feature_active(view: &View, account: bool, root: Root) -> bool {
    view.visible
        && view.root == root
        && (view.section == Section::Content || view.section == Section::Settings && account)
}

fn apply_view(
    window: &tauri::WebviewWindow,
    before: &View,
    before_account: bool,
    view: &View,
    account: bool,
) {
    if feature_active(before, before_account, Root::News) {
        window.state::<host::State>().stop_news();
    }
    window.state::<playback::State>().context(
        view.root == Root::Hongguo && view.section == Section::Content,
        view.root == Root::Hongguo && account,
        view.visible,
    );
    let app = window.app_handle();
    if feature_active(before, before_account, Root::Bilibili) {
        app.state::<bilibili::State>()
            .leave(view.root != Root::Bilibili || view.section != Section::Content);
    }
    if feature_active(before, before_account, Root::Reader) {
        app.state::<reader::State>().leave(app, view.generation);
    }
}

#[tauri::command]
fn snapshot(state: tauri::State<Mutex<Model>>, window: tauri::WebviewWindow) -> serde_json::Value {
    let state = state.lock().unwrap();
    serde_json::json!({"view":state.view,"feed_requests":state.feed_requests,"favorite_reads":window.state::<playback::State>().favorite_reads(),
        "bilibili":window.state::<bilibili::State>().receipt(),"reader_requests":window.state::<reader::State>().requests(),"media_route":if playback::enabled(){"browser_service"}else{"unavailable"},"real_service":playback::enabled()&&std::env::var_os("LAZYBUDDY_BROWSER_FIXTURE_FILE").is_none(), "native_texture_integrated":false,"tray":window.app_handle().tray_by_id("shell").is_some(),"desktop":dock::snapshot(&window)})
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DockRender {
    docked: bool,
    visible: bool,
    viewport: [u32; 2],
    button: [u32; 2],
    icon: [u32; 2],
    delay_ms: u32,
}
impl DockRender {
    fn valid(&self) -> bool {
        self.viewport
            .iter()
            .chain(&self.button)
            .chain(&self.icon)
            .all(|n| *n <= 16384)
            && self.delay_ms <= 60000
    }
}
#[tauri::command]
fn dock_rendered(report: DockRender, window: tauri::WebviewWindow) -> Result<(), String> {
    if !report.valid() {
        return Err("invalid_render_diagnostic".into());
    }
    dock::trace(
        &window,
        "dock_rendered",
        serde_json::json!({"render":report}),
    );
    Ok(())
}
fn visibility(window: &tauri::WebviewWindow, visible: bool) {
    let visible = visible && !dock::is_docked(window);
    let (before, view) = {
        let state = window.state::<Mutex<Model>>();
        let mut state = state.lock().unwrap();
        let before = state.view.clone();
        let view = state.transition(before.root, before.section, visible);
        if before.generation != view.generation {
            apply_view(
                window,
                &before,
                state.account_page,
                &view,
                state.account_page,
            );
        }
        (before, view)
    };
    if before.generation != view.generation {
        dock::trace(
            window,
            "visibility_changed",
            serde_json::json!({"visible":visible}),
        );
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
    if !matches!(
        action,
        "hide" | "close" | "minimize" | "show" | "activate" | "expand" | "quit"
    ) {
        return Err("unknown window action".into());
    }
    dock::trace(
        &window,
        "window_action",
        serde_json::json!({"action":action}),
    );
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
        "activate" => {
            show_without_focus(&window)?;
            let hwnd = window.hwnd().map_err(|_| "window unavailable")?;
            if unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd.0 as _)
            } == 0
            {
                return Err("activate failed".into());
            }
        }
        "expand" => dock::expand(&window).map_err(|_| "expand failed")?,
        "quit" => {
            if !window.state::<reader::State>().begin_exit() {
                return Ok(());
            }
            window.state::<host::State>().set_exiting(true);
            let app = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = app.state::<bilibili::State>().disconnect(&app).await {
                    let _ = app.emit("shell-error", error);
                    app.state::<reader::State>().cancel_exit();
                    app.state::<host::State>().set_exiting(false);
                    return;
                }
                if let Err(error) = app.state::<reader::State>().disconnect(&app).await {
                    let _ = app.emit("shell-error", format!("阅读进度保存失败，未退出：{error}"));
                    app.state::<reader::State>().cancel_exit();
                    app.state::<host::State>().set_exiting(false);
                    return;
                }
                let Some(completed) = app.state::<playback::State>().shutdown() else {
                    return;
                };
                completed.await;
                let state = app.state::<host::State>();
                state.stop_news();
                let _guard = state.exit_barrier().await;
                let _ = state.flush().await;
                state.finish_logs();
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
fn capture_shortcut(enabled: bool, window: tauri::WebviewWindow) -> Result<(), String> {
    if enabled {
        platform::unregister(&window)
    } else {
        platform::resume(
            &window,
            window.state::<host::State>().preferences().shortcut,
        )
    }
}
#[tauri::command]
fn open_logs() -> Result<(), String> {
    let path = lazybuddy_host_control::logs::root().map_err(|_| "无法确定日志目录")?;
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
    if let Ok(exe) = std::env::current_exe() {
        match lazybuddy_app_updater::recover_on_start(exe.parent().unwrap()) {
            Ok(false) => {}
            Ok(true) => return,
            Err(error) => {
                eprintln!("update recovery could not start: {error}");
                return;
            }
        }
    }
    let host = host::State::new().expect("host settings unavailable");
    tauri::Builder::default()
        .manage(host)
        .manage(Mutex::new(Model::default()))
        .manage(Mutex::new(dock::Dock::default()))
        .manage(playback::State::default())
        .manage(reader::State::default())
        .manage(bilibili::State::default())
        .invoke_handler(tauri::generate_handler![
            transition,
            snapshot,
            dock_rendered,
            host::module_call,
            host::news_call,
            reader::reader_call,
            bilibili::bilibili_call,
            bilibili::bilibili_suspend,
            reader::reader_pick,
            reader::reader_checkpoint,
            host::settings,
            host::set_startup,
            host::open_link,
            host::check_update,
            host::update_status,
            host::apply_update,
            window_action,
            resize,
            toggle_pin,
            capture_shortcut,
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
            if let tauri::WindowEvent::Focused(focused) = event {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    dock::trace(
                        &webview,
                        "focus_changed",
                        serde_json::json!({"focused":focused}),
                    );
                }
            }
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
    fn render_diagnostic_is_bounded_and_does_not_accept_extra_fields() {
        let valid = serde_json::json!({"docked":true,"visible":true,"viewport":[40,40],"button":[38,38],"icon":[28,28],"delay_ms":16});
        assert!(
            serde_json::from_value::<DockRender>(valid.clone())
                .unwrap()
                .valid()
        );
        let mut huge = valid.clone();
        huge["icon"] = serde_json::json!([20000, 20000]);
        assert!(!serde_json::from_value::<DockRender>(huge).unwrap().valid());
        let mut secret = valid;
        secret["cookie"] = serde_json::json!("must-not-be-logged");
        assert!(serde_json::from_value::<DockRender>(secret).is_err());
    }
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
