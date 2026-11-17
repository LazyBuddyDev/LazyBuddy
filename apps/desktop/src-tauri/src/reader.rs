//! Trusted local-file selection and a private, lifecycle-scoped reading service.
use lazybuddy_service_client::Session;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};
use tauri::Manager;

#[derive(Default)]
pub struct State {
    closing: std::sync::atomic::AtomicBool,
    requests: std::sync::atomic::AtomicU64,
    operation: tokio::sync::Mutex<()>,
    session: Mutex<Option<Session>>,
    pending: Mutex<HashMap<String, Value>>,
    selected: Mutex<HashSet<String>>,
}
pub fn allowed(model: &crate::Model, generation: u64, method: &str) -> bool {
    model.view.root == crate::Root::Reader
        && model.view.visible
        && model.view.generation == generation
        && (model.view.section == crate::Section::Content
            || model.view.section == crate::Section::Settings
                && model.account_page
                && matches!(method, "preferences.get" | "preferences.set"))
}
impl State {
    pub fn begin_exit(&self) -> bool {
        !self.closing.swap(true, std::sync::atomic::Ordering::AcqRel)
    }
    pub fn cancel_exit(&self) {
        self.closing
            .store(false, std::sync::atomic::Ordering::Release);
    }
    pub fn requests(&self) -> u64 {
        self.requests.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn service(&self, app: &tauri::AppHandle) -> Result<Session, String> {
        let mut slot = self.session.lock().unwrap();
        if slot.is_none() {
            let host = app.state::<crate::host::State>();
            let mut launch = host
                .modules()
                .launch("reader")
                .map_err(|_| "请先安装小说模块")?;
            launch.environment.push((
                "LAZYBUDDY_DATA_DIR".into(),
                host.data
                    .join("services/reader")
                    .to_string_lossy()
                    .into_owned(),
            ));
            if lazybuddy_host_control::isolated() {
                launch.environment.push((
                    "LAZYBUDDY_CREDENTIAL_SCOPE".into(),
                    lazybuddy_host_control::instance().map_err(|_| "invalid_credential_scope")?,
                ));
            }
            *slot = Some(Session::with_environment(
                launch.executable,
                launch.arguments,
                launch.environment,
            ));
        }
        Ok(slot.as_ref().unwrap().clone())
    }
    async fn request(
        &self,
        app: &tauri::AppHandle,
        method: &str,
        args: Value,
    ) -> Result<Value, String> {
        let result = self
            .service(app)?
            .request(json!({"version":1,"id":"web-reader","method":method,"args":args}))
            .await;
        let reply = result.map_err(|_| "reader_disconnected".to_owned())?;
        if reply["ok"] != true {
            let code = reply["error"]["code"]
                .as_str()
                .filter(|s| s.len() < 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .unwrap_or("reader_failed");
            lazybuddy_host_control::logs::record(
                "reader",
                "api_failed",
                json!({"method":method,"code":code}),
            );
            return Err(code.into());
        }
        Ok(reply["data"].clone())
    }
    async fn flush(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let pending = self.pending.lock().unwrap().clone();
        for (book, position) in pending {
            let current = self
                .request(app, "book.open", json!({"book_id":book}))
                .await?;
            let revision = current["revision"].as_u64().ok_or("invalid_reader_reply")? + 1;
            self.request(
                app,
                "progress.save",
                json!({"position":position,"revision":revision}),
            )
            .await?;
            let mut slots = self.pending.lock().unwrap();
            if slots.get(&book) == Some(&position) {
                slots.remove(&book);
            }
        }
        Ok(())
    }
    pub async fn suspend(&self, app: &tauri::AppHandle) -> Result<(), String> {
        self.selected.lock().unwrap().clear();
        let _guard = self.operation.lock().await;
        if self.session.lock().unwrap().is_none() {
            return Ok(());
        }
        self.request(app, "lifecycle.suspend", json!({})).await?;
        self.flush(app).await
    }
    pub async fn disconnect(&self, app: &tauri::AppHandle) -> Result<(), String> {
        self.suspend(app).await?;
        let _guard = self.operation.lock().await;
        let session = self.session.lock().unwrap().take();
        if let Some(session) = session {
            session.shutdown().await;
        }
        Ok(())
    }
}
#[tauri::command]
pub async fn reader_pick(generation: u64, app: tauri::AppHandle) -> Result<Vec<String>, String> {
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        "library.import",
    ) {
        return Err("inactive_reader_page".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = handle
            .get_webview_window("main")
            .ok_or("窗口未就绪".into())
            .and_then(|w| crate::platform::pick_books(&w));
        let _ = tx.send(result);
    })
    .map_err(|_| "文件选择失败")?;
    let paths = rx.await.map_err(|_| "文件选择失败")??;
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        "library.import",
    ) {
        return Err("inactive_reader_page".into());
    }
    *app.state::<State>().selected.lock().unwrap() = paths.iter().cloned().collect();
    Ok(paths)
}
#[tauri::command]
pub fn reader_checkpoint(
    position: Value,
    generation: u64,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if app
        .state::<State>()
        .closing
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("reader_closing".into());
    }
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        "progress.save",
    ) {
        return Err("inactive_reader_page".into());
    }
    if serde_json::to_vec(&position)
        .map_err(|_| "invalid_position")?
        .len()
        > 2048
        || !position.is_object()
    {
        return Err("invalid_position".into());
    }
    let book = position["book_id"]
        .as_str()
        .filter(|s| {
            s.len() == 29 && s.starts_with("book-") && s[5..].bytes().all(|b| b.is_ascii_hexdigit())
        })
        .ok_or("invalid_position")?
        .to_owned();
    let state = app.state::<State>();
    let mut pending = state.pending.lock().unwrap();
    if pending.len() >= 8 && !pending.contains_key(&book) {
        return Err("pending_progress_full".into());
    }
    pending.insert(book, position);
    Ok(())
}
#[tauri::command]
pub async fn reader_suspend(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<State>().suspend(&app).await
}
#[tauri::command]
pub async fn reader_call(
    method: String,
    args: Value,
    generation: u64,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    if !matches!(
        method.as_str(),
        "health"
            | "online.search"
            | "online.detail"
            | "online.chapter"
            | "online.open"
            | "online.status"
            | "online.cancel"
            | "library.list"
            | "library.import"
            | "library.remove"
            | "library.add"
            | "library.favorite"
            | "import.status"
            | "import.cancel"
            | "book.open"
            | "chapter.read"
            | "chapter.find"
            | "resource.read"
            | "preferences.get"
            | "preferences.set"
            | "progress.flush"
    ) || !args.is_object()
    {
        return Err("invalid_reader_method".into());
    }
    let state = app.state::<State>();
    let _guard = state.operation.lock().await;
    if state.closing.load(std::sync::atomic::Ordering::Acquire) {
        return Err("reader_closing".into());
    }
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        &method,
    ) {
        return Err("inactive_reader_page".into());
    }
    state
        .requests
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if method == "library.import"
        && !args["path"]
            .as_str()
            .is_some_and(|p| state.selected.lock().unwrap().contains(p))
    {
        return Err("file_not_selected".into());
    }
    if method == "progress.flush" {
        state.flush(&app).await?;
        return Ok(json!({}));
    }
    if method == "library.remove" {
        state.flush(&app).await?;
    }
    let reply = state.request(&app, &method, args).await?;
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        &method,
    ) {
        return Err("stale_reader_reply".into());
    }
    Ok(reply)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reading_and_settings_are_generation_scoped() {
        let mut m = crate::Model::default();
        m.transition(crate::Root::Reader, crate::Section::Content, true);
        let generation = m.view.generation;
        assert!(allowed(&m, generation, "chapter.read"));
        assert!(allowed(&m, generation, "online.search"));
        assert!(allowed(&m, generation, "online.detail"));
        assert!(allowed(&m, generation, "online.chapter"));
        assert!(allowed(&m, generation, "online.open"));
        assert!(allowed(&m, generation, "library.favorite"));
        assert!(allowed(&m, generation, "library.add"));
        m.transition(crate::Root::News, crate::Section::Content, true);
        m.transition(crate::Root::Reader, crate::Section::Content, true);
        assert!(!allowed(&m, generation, "chapter.read"));
        assert!(!allowed(&m, generation, "online.status"));
        assert!(!allowed(&m, generation, "online.chapter"));
        assert!(!allowed(&m, generation, "online.open"));
        assert!(!allowed(&m, generation, "library.favorite"));
        assert!(!allowed(&m, generation, "library.add"));
        m.transition(crate::Root::Reader, crate::Section::Settings, true);
        m.account_page = true;
        assert!(allowed(&m, m.view.generation, "preferences.set"));
        assert!(!allowed(&m, m.view.generation, "chapter.read"));
        assert!(!allowed(&m, m.view.generation, "online.search"));
        assert!(!allowed(&m, m.view.generation, "online.detail"));
        assert!(!allowed(&m, m.view.generation, "online.chapter"));
        assert!(!allowed(&m, m.view.generation, "online.open"));
        assert!(!allowed(&m, m.view.generation, "library.favorite"));
        assert!(!allowed(&m, m.view.generation, "library.add"));
        m.transition(crate::Root::Reader, crate::Section::Content, false);
        assert!(!allowed(&m, m.view.generation, "chapter.read"));
        assert!(!allowed(&m, m.view.generation, "library.add"));
    }
}
