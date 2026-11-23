//! Native boundaries shared by the Web UI and the local CLI.
use lazybuddy_module_contract::Kind;
use lazybuddy_module_manager::{
    Manager as Modules,
    package::{Catalog, Source, download_catalog},
};
use lazybuddy_service_client::Session;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager};

const APP_UPDATES: bool = !cfg!(debug_assertions);
fn require_app_updates() -> Result<(), String> {
    if APP_UPDATES {
        Ok(())
    } else {
        Err("开发版请通过 start.bat 重新构建启动".into())
    }
}

pub struct State {
    pub data: PathBuf,
    logging: Option<lazybuddy_host_control::logs::Session>,
    pub news: Mutex<Option<(bool, Session)>>,
    catalog: Mutex<Option<Catalog>>,
    operation: Arc<tokio::sync::Mutex<()>>,
    exiting: Arc<AtomicBool>,
    preferences: Mutex<crate::preferences::Preferences>,
    preferences_read_only: bool,
    writer: crate::preferences::Writer,
    prepared: Mutex<Option<lazybuddy_app_updater::Prepared>>,
    checking: std::sync::atomic::AtomicBool,
    news_slots: std::sync::Arc<tokio::sync::Semaphore>,
}
impl State {
    pub fn new() -> Result<Self, String> {
        let data = lazybuddy_host_control::data_dir().map_err(|e| e.to_string())?;
        if !lazybuddy_host_control::isolated()
            && [
                "LAZYBUDDY_BROWSER_FIXTURE_FILE",
                "LAZYBUDDY_PROTOTYPE_PROBE",
                "LAZYBUDDY_PROTOTYPE_DATA_DIR",
            ]
            .iter()
            .any(|name| std::env::var_os(name).is_some())
        {
            return Err("测试配置要求独立实例".into());
        }
        let loaded = crate::preferences::Preferences::load(&data.join("web-preferences.json"));
        let preferences_read_only = loaded.is_err();
        let preferences = loaded.unwrap_or_else(|_| crate::preferences::Preferences {
            auto_update: false,
            ..Default::default()
        });
        let logging = lazybuddy_host_control::logs::begin().ok();
        lazybuddy_host_control::logs::event(
            "host",
            "runtime",
            json!({"version":env!("CARGO_PKG_VERSION"),"debug":cfg!(debug_assertions),"pid":std::process::id()}),
        );
        Ok(Self {
            logging,
            writer: crate::preferences::Writer::new(data.join("web-preferences.json")),
            data,
            news: Mutex::new(None),
            catalog: Mutex::new(None),
            operation: Default::default(),
            exiting: Default::default(),
            preferences: Mutex::new(preferences),
            preferences_read_only,
            prepared: Mutex::new(None),
            checking: Default::default(),
            news_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
    pub fn finish_logs(&self) {
        if let Some(session) = &self.logging {
            let _ = session.finish();
        }
    }
    pub fn modules(&self) -> Modules {
        Modules::new(self.data.join("modules"))
    }
    pub fn stop_news(&self) {
        if let Some((_, service)) = self.news.lock().unwrap().take() {
            service.stop();
        }
    }
    pub fn preferences(&self) -> crate::preferences::Preferences {
        self.preferences.lock().unwrap().clone()
    }
    pub fn set_exiting(&self, exiting: bool) {
        self.exiting.store(exiting, Ordering::Release);
    }
    pub async fn exit_barrier(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.operation.lock().await
    }
    pub async fn flush(&self) -> Result<(), String> {
        if self.preferences_read_only {
            return Ok(());
        }
        self.writer.save(self.preferences()).await
    }
    pub fn remember_window(&self, window: &tauri::WebviewWindow) {
        if self.preferences_read_only
            || crate::dock::is_docked(window)
            || !window.is_visible().unwrap_or(false)
            || window.is_minimized().unwrap_or(false)
        {
            return;
        }
        let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
            return;
        };
        let bounds = [
            position.x,
            position.y,
            size.width as i32,
            size.height as i32,
        ];
        let app = window.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<State>();
            let _guard = state.operation.lock().await;
            if state.exiting.load(Ordering::Acquire) {
                return;
            }
            let mut value = state.preferences();
            value.window = Some(bounds);
            match state.writer.save(value.clone()).await {
                Ok(()) => *state.preferences.lock().unwrap() = value,
                Err(error) => {
                    let _ = app.emit("shell-error", error);
                }
            }
        });
    }
}
fn failure(error: impl ToString) -> String {
    error.to_string()
}
pub fn features(
    installed: &[lazybuddy_module_contract::Manifest],
    catalog: Option<&Catalog>,
) -> Vec<Value> {
    let mut entries = std::collections::BTreeMap::new();
    for m in installed.iter().filter(|m| m.kind == Kind::Feature) {
        entries.insert(
            m.id.clone(),
            json!({"id":m.id,"name":m.name,"version":m.version,"installed":true,"update":false,
                "compatible":crate::Root::accepts_manifest(m)}),
        );
    }
    for p in catalog
        .into_iter()
        .flat_map(|c| &c.packages)
        .filter(|p| p.kind == Kind::Feature && crate::Root::from_module(&p.id).is_some())
    {
        let Some(available) = lazybuddy_module_contract::parse_version(&p.version) else {
            continue;
        };
        if let Some(entry) = entries.get_mut(&p.id) {
            entry["available_version"] = json!(p.version);
            entry["update"] = json!(
                entry["version"]
                    .as_str()
                    .and_then(lazybuddy_module_contract::parse_version)
                    .is_some_and(|installed| available > installed)
            );
        } else {
            entries.insert(p.id.clone(),json!({"id":p.id,"name":if p.name.is_empty(){&p.id}else{&p.name},"version":p.version,"installed":false,"update":false}));
        }
    }
    entries.into_values().collect()
}
pub async fn modules(
    app: tauri::AppHandle,
    action: String,
    id: Option<String>,
) -> Result<Value, String> {
    modules_with_progress(app, action, id, None).await
}
fn load_catalog() -> Result<Catalog, String> {
    if std::env::var_os("LAZYBUDDY_PROTOTYPE_PROBE").is_some() {
        return Err("隔离验证未启用在线目录".into());
    }
    let exe = std::env::current_exe().map_err(failure)?;
    let source = Source::load(&exe.with_file_name("module-catalog.json")).map_err(failure)?;
    download_catalog(&source.url, &source.public_key).map_err(failure)
}
async fn modules_with_progress(
    app: tauri::AppHandle,
    action: String,
    id: Option<String>,
    on_progress: Option<tauri::ipc::Channel<Value>>,
) -> Result<Value, String> {
    let state = app.state::<State>();
    if state.exiting.load(Ordering::Acquire) {
        return Err("应用正在退出".into());
    }
    if action == "install" && id.as_deref().and_then(crate::Root::from_module).is_none() {
        return Err("当前宿主未支持此功能".into());
    }
    if action != "list" && action != "catalog" {
        let model = app.state::<Mutex<crate::Model>>();
        if model.lock().unwrap().view.section != crate::Section::Modules {
            return Err("请先打开功能管理".into());
        }
        let news = state.news.lock().unwrap().take();
        if let Some((_, service)) = news {
            service.shutdown().await;
        }
        // Drain account/media transactions before immutable package directories can be reclaimed.
        app.state::<crate::playback::State>().disconnect().await;
        app.state::<crate::reader::State>().disconnect(&app).await?;
        app.state::<crate::bilibili::State>()
            .disconnect(&app)
            .await?;
    }
    let root = state.data.join("modules");
    let catalog = state.catalog.lock().unwrap().clone();
    let operation = state.operation.clone();
    let exiting = state.exiting.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let run = move || -> Result<_, String> {
        let cancelled = || exiting.load(Ordering::Acquire);
        let begin_commit = |manifests: &[lazybuddy_module_contract::Manifest]| {
            if manifests
                .iter()
                .any(|m| m.kind == Kind::Feature && !crate::Root::accepts_manifest(m))
            {
                return Err(std::io::Error::other("模块页面与当前宿主不兼容"));
            }
            let guard = operation.clone().blocking_lock_owned();
            if cancelled() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "application is exiting",
                ));
            }
            Ok(guard)
        };
        let manager = Modules::new(root);
        let mut loaded = None;
        match action.as_str() {
            "list" => {}
            "catalog" => {
                loaded = Some(load_catalog()?);
            }
            "install" => {
                if id.as_deref() == Some("hongguo")
                    && !catalog
                        .as_ref()
                        .and_then(|c| c.packages.iter().find(|p| p.id == "hongguo"))
                        .is_some_and(|p| crate::playback::scoped_package(&p.version))
                {
                    return Err(
                        "目录中的红果包仍为旧原生版本，Web 宿主需要 0.2.1 以上浏览器包".into(),
                    );
                }
                manager
                    .install_from_catalog_controlled(
                        catalog.as_ref().ok_or("目录未就绪")?,
                        id.as_deref().ok_or("缺少功能 ID")?,
                        |stage, downloaded, total| {
                            if let Some(channel) = &on_progress {
                                let _ = channel.send(
                                    json!({"stage":stage,"downloaded":downloaded,"total":total}),
                                );
                            }
                        },
                        cancelled,
                        begin_commit,
                    )
                    .map_err(failure)?;
            }
            "uninstall" => {
                let _guard = begin_commit(&[]).map_err(failure)?;
                manager
                    .uninstall(id.as_deref().ok_or("缺少功能 ID")?)
                    .map_err(failure)?;
            }
            _ => return Err("未知功能操作".into()),
        }
        let mut installed = manager.list_active().map_err(failure)?;
        if let Some(path) = crate::playback::development_packages() {
            let manifest: lazybuddy_module_contract::Manifest = serde_json::from_slice(
                &std::fs::read(path.join("hongguo/module.json")).map_err(failure)?,
            )
            .map_err(failure)?;
            manifest.validate().map_err(failure)?;
            installed.retain(|m| m.id != "hongguo");
            installed.push(manifest);
        }
        Ok((installed, loaded))
    };
    // A blocking network read must not keep the app runtime alive on exit.
    std::thread::spawn(move || {
        let _ = send.send(run());
    });
    let reply = receive.await.map_err(failure)??;
    if let Some(catalog) = reply.1 {
        *state.catalog.lock().unwrap() = Some(catalog);
    }
    let catalog = state.catalog.lock().unwrap();
    Ok(
        json!({"entries":features(&reply.0,catalog.as_ref()),"installed":reply.0.into_iter().filter(crate::Root::accepts_manifest).collect::<Vec<_>>(),"catalog_ready":catalog.is_some()}),
    )
}
#[tauri::command]
pub async fn module_call(
    action: String,
    id: Option<String>,
    app: tauri::AppHandle,
    webview: tauri::Webview,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<Value, String> {
    modules_with_progress(
        app,
        action,
        id,
        on_progress.map(|id| id.channel_on(webview)),
    )
    .await
}

#[tauri::command]
pub async fn news_call(
    method: String,
    args: Value,
    generation: u64,
    direct: Option<bool>,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    if !matches!(
        method.as_str(),
        "sources" | "state" | "preferences" | "configure" | "fetch"
    ) || !args.is_object()
    {
        return Err("无效新闻请求".into());
    }
    let state = app.state::<State>();
    let _permit = if method == "fetch" {
        let count = args["sources"]
            .as_array()
            .map(Vec::len)
            .filter(|n| (1..=4).contains(n))
            .ok_or("expected_one_to_four_sources")?;
        Some(
            state
                .news_slots
                .clone()
                .acquire_many_owned(count as u32)
                .await
                .map_err(failure)?,
        )
    } else {
        None
    };
    let direct = direct.unwrap_or(true);
    let (service, active_generation) = {
        let model = app.state::<Mutex<crate::Model>>();
        let mut model = model.lock().unwrap();
        if !news_allowed(&model, &method, generation) {
            return Err("inactive_news_page".into());
        }
        model.feed_requests += 1;
        let mut slot = state.news.lock().unwrap();
        if slot.as_ref().is_some_and(|(mode, _)| *mode != direct) {
            if let Some((_, previous)) = slot.take() {
                previous.stop();
            }
        }
        if slot.is_none() {
            let modules = state.modules();
            let mut launch = modules.launch("news").map_err(failure)?;
            if !lazybuddy_module_contract::parse_version(&launch.manifest.version)
                .is_some_and(|v| v >= [0, 1, 1])
            {
                return Err("请在功能管理中更新新闻模块至 0.1.1 以上，以使用连接设置".into());
            }
            launch.environment.push((
                "LAZYBUDDY_NEWS_DATA_DIR".into(),
                state
                    .data
                    .join("services/news")
                    .to_string_lossy()
                    .into_owned(),
            ));
            launch.environment.push((
                "LAZYBUDDY_NEWS_DIRECT".into(),
                if direct { "1" } else { "0" }.into(),
            ));
            *slot = Some((
                direct,
                Session::with_lease(
                    launch.executable,
                    launch.arguments,
                    launch.environment,
                    Some(launch.lease),
                ),
            ));
        }
        (
            slot.as_ref().unwrap().1.clone(),
            model.active_generation.clone(),
        )
    };
    let request_id = lazybuddy_host_control::logs::next_id();
    let began = std::time::Instant::now();
    let reply = service
        .request_scoped(
            json!({"version":1,"id":"web-news","method":method,"args":args}),
            active_generation,
            generation,
        )
        .await
        .map_err(failure)?;
    let model = app.state::<Mutex<crate::Model>>();
    if !news_allowed(&model.lock().unwrap(), &method, generation)
        || !state
            .news
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(mode, current)| *mode == direct && current.same_connection(&service))
    {
        return Err("stale_news_reply".into());
    }
    if reply["ok"] != true {
        return Err(reply["error"]["code"]
            .as_str()
            .unwrap_or("news_failed")
            .into());
    }
    if method == "fetch" {
        for row in reply["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| row["ok"] == false)
        {
            // Retain only diagnostic tokens, never arbitrary service messages or URLs.
            let detail = row["error"]["detail"].as_str().unwrap_or("");
            let fields: serde_json::Map<String, Value> = detail
                .split_whitespace()
                .filter_map(|part| {
                    let (key, value) = part.split_once('=')?;
                    (matches!(key, "stage" | "kind" | "status" | "phase")
                        && value.len() <= 64
                        && value
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'_'))
                    .then(|| (key.into(), json!(value)))
                })
                .collect();
            lazybuddy_host_control::logs::record(
                "news",
                "source_failed",
                json!({"source":row["id"],"diagnostic":fields,"request_id":request_id,"generation":generation,"direct":direct,"elapsed_ms":began.elapsed().as_millis()}),
            );
        }
    }
    Ok(reply["data"].clone())
}
fn news_allowed(model: &crate::Model, method: &str, generation: u64) -> bool {
    model.view.visible
        && model.view.root == crate::Root::News
        && model.view.generation == generation
        && (model.view.section == crate::Section::Content
            || model.view.section == crate::Section::Settings
                && model.account_page
                && method != "fetch")
}

#[tauri::command]
pub async fn settings(
    value: Option<crate::preferences::Preferences>,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    let state = app.state::<State>();
    if let Some(mut value) = value {
        if state.preferences_read_only {
            return Err("设置文件无法读取，原文件已保留。修复文件并重启后可保存。".into());
        }
        let _guard = state.operation.lock().await;
        let previous = state.preferences();
        value.window = previous.window;
        if !value.shortcut.supported()
            || !value.valid_attention()
            || !(0.0..=1.0).contains(&value.background_opacity)
            || !(0.0..=1.0).contains(&value.card_opacity)
        {
            return Err("无效设置".into());
        }
        let shortcut_changed = previous.shortcut != value.shortcut;
        let saved = async {
            if shortcut_changed {
                set_shortcut(&app, value.shortcut).await?;
            }
            state.writer.save(value.clone()).await
        }
        .await;
        if let Err(error) = saved {
            if shortcut_changed && set_shortcut(&app, previous.shortcut).await.is_err() {
                return Err(format!("{error} 原快捷键恢复失败。"));
            }
            return Err(error);
        }
        *state.preferences.lock().unwrap() = value;
    }
    Ok(
        json!({"preferences":state.preferences(),"startup":crate::startup::enabled(),"isolated":lazybuddy_host_control::isolated(),"shortcut_capturing":crate::platform::capturing(),"app_updates":APP_UPDATES,"preferences_read_only":state.preferences_read_only}),
    )
}
async fn set_shortcut(
    app: &tauri::AppHandle,
    shortcut: crate::shortcut::Shortcut,
) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("窗口未就绪")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::platform::resume(&window, shortcut));
    })
    .map_err(failure)?;
    rx.await.map_err(failure)?
}

#[tauri::command]
pub async fn set_startup(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::startup::set(enabled).map_err(failure))
        .await
        .map_err(failure)?
}
#[tauri::command]
pub async fn open_link(url: String) -> Result<(), String> {
    if url.len() > 8192
        || !(url.starts_with("https://") || url.starts_with("http://"))
        || url.chars().any(char::is_control)
    {
        return Err("无效新闻链接".into());
    }
    tauri::async_runtime::spawn_blocking(move || open::that_detached(url).map_err(failure))
        .await
        .map_err(failure)?
}
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<bool, String> {
    require_app_updates()?;
    use std::sync::atomic::Ordering;
    let state = app.state::<State>();
    if state.checking.swap(true, Ordering::AcqRel) {
        return Err("正在检查更新".into());
    }
    let cache = state.data.join("updates");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let exe = std::env::current_exe().map_err(failure)?;
        lazybuddy_app_updater::prepare(exe.parent().unwrap(), &cache).map_err(failure)
    })
    .await
    .map_err(failure)
    .and_then(|r| r);
    state.checking.store(false, Ordering::Release);
    if result.is_err() {
        lazybuddy_host_control::logs::record("host", "update_check_failed", json!({}));
    }
    let prepared = result?;
    let ready = prepared.is_some();
    *state.prepared.lock().unwrap() = prepared;
    let _ = app.emit("shell-update", ready);
    Ok(ready)
}
#[tauri::command]
pub async fn apply_update(app: tauri::AppHandle) -> Result<(), String> {
    require_app_updates()?;
    let prepared = app
        .state::<State>()
        .prepared
        .lock()
        .unwrap()
        .clone()
        .ok_or("没有已准备更新")?;
    tauri::async_runtime::spawn_blocking(move || {
        let exe = std::env::current_exe().map_err(failure)?;
        lazybuddy_app_updater::start_helper(&prepared, exe.parent().unwrap()).map_err(failure)
    })
    .await
    .map_err(failure)??;
    let window = app.get_webview_window("main").ok_or("窗口未就绪")?;
    crate::window_action("quit", window)
}
#[tauri::command]
pub fn update_status(app: tauri::AppHandle) -> Value {
    let state = app.state::<State>();
    let prepared = state.prepared.lock().unwrap();
    json!({"ready":prepared.is_some(),"build":prepared.as_ref().map(|p|format!("{}:{}",p.version,p.build))})
}
pub fn start(app: tauri::AppHandle) -> Result<(), String> {
    let dispatcher = app.clone();
    lazybuddy_host_control::serve(move|request,reply|{
        let app=dispatcher.clone();
        tauri::async_runtime::spawn(async move{
            let result=match(request.version,request.command.as_str()){
                (1,"status")=>{
                    let view=app.state::<Mutex<crate::Model>>().lock().unwrap().view.clone();
                    let state=app.state::<State>();
                    Ok(json!({"pid":std::process::id(),"exe":std::env::current_exe().ok(),"backend":"web","bilibili":app.state::<crate::bilibili::State>().receipt(),"hidden":!view.visible,"view":view,
                        "window":app.get_webview_window("main").map(|w|crate::dock::snapshot(&w)["bounds"].clone()),
                        "app_updates":APP_UPDATES,"update_checking":state.checking.load(std::sync::atomic::Ordering::Acquire),"update_ready":state.prepared.lock().unwrap().is_some()}))
                },
                (1,"modules.list")=>modules(app.clone(),"list".into(),None).await,
                (1,"updates.check")=>require_app_updates().map(|()|{let handle=app.clone();tauri::async_runtime::spawn(async move{if let Err(error)=check_update(handle.clone()).await{let _=handle.emit("shell-error",error);}});json!({"accepted":true})}),
                (1,"updates.apply")=>apply_update(app.clone()).await.map(|_|json!({"accepted":true})),
                (1,"window.show"|"window.hide"|"app.quit")=>{
                    let action=match request.command.as_str(){"window.show"=>"show","window.hide"=>"hide",_=>"quit"};
                    let (tx,rx)=tokio::sync::oneshot::channel();let handle=app.clone();
                    match app.run_on_main_thread(move||{let result=handle.get_webview_window("main").ok_or("窗口未就绪".into()).and_then(|w|crate::window_action(action,w));let _=tx.send(result);}){
                        Ok(())=>rx.await.map_err(failure).and_then(|r|r).map(|_|json!({})),Err(e)=>Err(failure(e))
                    }
                },
                _=>Err("unsupported_command".into())
            };
            let _=reply.send(match result{Ok(data)=>json!({"ok":true,"data":data}),Err(error)=>lazybuddy_host_control::error("host_request_failed",error)});
        });
    }).map_err(failure)?;
    {
        tauri::async_runtime::spawn(async move {
            loop {
                if !lazybuddy_host_control::isolated()
                    && std::env::var_os("LAZYBUDDY_PROTOTYPE_PROBE").is_none()
                {
                    // Network preparation must not hold the settings/package/exit operation lock.
                    match tauri::async_runtime::spawn_blocking(load_catalog).await {
                        Ok(Ok(catalog)) => {
                            {
                                let state = app.state::<State>();
                                let _guard = state.operation.lock().await;
                                *state.catalog.lock().unwrap() = Some(catalog);
                            }
                            let _ = app.emit("shell-catalog", true);
                        }
                        _ => lazybuddy_host_control::logs::record(
                            "host",
                            "catalog_check_failed",
                            json!({}),
                        ),
                    }
                }
                if APP_UPDATES
                    && !lazybuddy_host_control::isolated()
                    && app.state::<State>().preferences().auto_update
                {
                    if let Err(error) = check_update(app.clone()).await {
                        let _ = app.emit("shell-error", format!("检查更新失败：{error}"));
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(900)).await;
            }
        });
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_and_other_settings_cannot_fetch_news() {
        let mut model = crate::Model::default();
        model.transition(crate::Root::News, crate::Section::Content, true);
        assert!(news_allowed(&model, "fetch", model.view.generation));
        assert!(news_allowed(&model, "configure", model.view.generation));
        let old = model.view.generation;
        model.transition(crate::Root::News, crate::Section::Settings, true);
        assert!(!news_allowed(&model, "fetch", model.view.generation));
        assert!(!news_allowed(&model, "sources", old));
        model.account_page = false;
        assert!(!news_allowed(&model, "configure", model.view.generation));
    }
    #[test]
    fn only_known_compatible_features_are_navigable() {
        let mut known: lazybuddy_module_contract::Manifest =
            serde_json::from_str(include_str!("../../../../contracts/fixtures/reader.json"))
                .unwrap();
        assert!(crate::Root::accepts_manifest(&known));
        known.id = "other".into();
        assert!(!crate::Root::accepts_manifest(&known));
        assert_eq!(features(&[known.clone()], None)[0]["compatible"], false);
        known.id = "news".into();
        assert!(!crate::Root::accepts_manifest(&known));
    }
    #[test]
    fn unavailable_catalog_keeps_installed_features() {
        let m = serde_json::from_str(include_str!("../../../../contracts/fixtures/feature.json"))
            .unwrap();
        let rows = features(&[m], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["installed"], true);
        assert_eq!(rows[0]["update"], false);
    }
}
