//! UI/lifecycle boundary only; account and media adapters live in the installed service.
use lazybuddy_service_client::Session;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Mutex};
use tauri::Manager;
#[derive(Default)]
pub struct State {
    operation: tokio::sync::Mutex<()>,
    session: Mutex<Option<Session>>,
    jobs: Mutex<HashMap<u64, (u64, String)>>,
    receipt: Mutex<Value>,
}
pub fn allowed(m: &crate::Model, generation: u64, method: &str) -> bool {
    m.view.root == crate::Root::Bilibili
        && m.view.visible
        && m.view.generation == generation
        && (m.view.section == crate::Section::Content
            || m.view.section == crate::Section::Settings
                && m.account_page
                && (method.starts_with("account.")
                    || matches!(method, "health" | "task.status" | "task.cancel")))
}
impl State {
    pub fn receipt(&self) -> Value {
        self.receipt.lock().unwrap().clone()
    }
    fn service(&self, app: &tauri::AppHandle) -> Result<Session, String> {
        let mut slot = self.session.lock().unwrap();
        if slot.is_none() {
            let host = app.state::<crate::host::State>();
            let mut launch = host
                .modules()
                .launch("bilibili")
                .map_err(|_| "请先安装B站模块")?;
            launch.environment.push((
                "LAZYBUDDY_DATA_DIR".into(),
                host.data
                    .join("services/bilibili")
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
        let reply = self
            .service(app)?
            .request(json!({"version":1,"id":"web-bilibili","method":method,"args":args}))
            .await
            .map_err(|_| "bilibili_disconnected")?;
        if reply["ok"] != true {
            let code = reply["error"]["code"]
                .as_str()
                .filter(|s| s.len() < 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .unwrap_or("bilibili_failed");
            lazybuddy_host_control::logs::record(
                "bilibili",
                "api_failed",
                json!({"method":method,"code":code}),
            );
            return Err(code.into());
        }
        Ok(reply["data"].clone())
    }
    pub async fn suspend(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        if self.session.lock().unwrap().is_some() {
            self.request(app, "lifecycle.suspend", json!({"release":true}))
                .await?;
        }
        self.jobs.lock().unwrap().clear();
        Ok(())
    }
    pub async fn suspend_if_inactive(&self, app: &tauri::AppHandle) {
        let _guard = self.operation.lock().await;
        let release;
        {
            let m = app.state::<Mutex<crate::Model>>();
            let m = m.lock().unwrap();
            release =
                m.view.root != crate::Root::Bilibili || m.view.section != crate::Section::Content;
            if m.view.root == crate::Root::Bilibili
                && m.view.visible
                && (m.view.section == crate::Section::Content
                    || m.view.section == crate::Section::Settings && m.account_page)
            {
                return;
            }
        }
        if self.session.lock().unwrap().is_some() {
            let _ = self
                .request(app, "lifecycle.suspend", json!({"release":release}))
                .await;
        }
        self.jobs.lock().unwrap().clear();
    }
    pub async fn disconnect(&self, app: &tauri::AppHandle) -> Result<(), String> {
        self.suspend(app).await?;
        let _guard = self.operation.lock().await;
        let session = self.session.lock().unwrap().take();
        if let Some(s) = session {
            s.shutdown().await;
        }
        Ok(())
    }
}
#[tauri::command]
pub async fn bilibili_call(
    method: String,
    args: Value,
    generation: u64,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    if !matches!(
        method.as_str(),
        "health"
            | "search"
            | "recommend"
            | "following"
            | "task.status"
            | "task.cancel"
            | "account.get"
            | "account.restore"
            | "account.login"
            | "account.poll"
            | "account.cancel"
            | "account.logout"
            | "playback.open"
            | "playback.suspend"
            | "playback.resume"
            | "playback.close"
    ) || !args.is_object()
        || args.to_string().len() > 4096
    {
        return Err("invalid_bilibili_method".into());
    }
    let s = app.state::<State>();
    let _guard = s.operation.lock().await;
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        &method,
    ) {
        return Err("inactive_bilibili_page".into());
    }
    if matches!(method.as_str(), "task.status" | "task.cancel")
        && args["job"]
            .as_u64()
            .is_none_or(|id| s.jobs.lock().unwrap().get(&id).map(|v| v.0) != Some(generation))
    {
        return Err("stale_bilibili_job".into());
    }
    let reply = s.request(&app, &method, args).await?;
    if let Some(id) = reply["job"].as_u64() {
        if reply["status"] == "pending" && method != "task.status" {
            let mut jobs = s.jobs.lock().unwrap();
            jobs.clear();
            jobs.insert(id, (generation, method.clone()));
        }
    }
    if !allowed(
        &app.state::<Mutex<crate::Model>>().lock().unwrap(),
        generation,
        &method,
    ) {
        return Err("inactive_bilibili_page".into());
    }
    if method == "task.status" && reply["status"] == "failed" {
        if let Some(id) = reply["job"].as_u64() {
            if let Some((_, operation)) = s.jobs.lock().unwrap().remove(&id) {
                lazybuddy_host_control::logs::api_error(
                    "bilibili",
                    &operation,
                    error_code(&reply["error"]),
                    &reply["diagnostic"],
                    json!({"generation":generation}),
                );
            }
        }
    }
    let account = if reply["result"]["state"].is_string() {
        &reply["result"]
    } else {
        &reply
    };
    if let Some(state) = account["state"].as_str() {
        let operation = if method == "task.status" {
            reply["job"]
                .as_u64()
                .and_then(|id| s.jobs.lock().unwrap().get(&id).map(|v| v.1.clone()))
                .unwrap_or_default()
        } else {
            method.clone()
        };
        *s.receipt.lock().unwrap() = json!({"state":state,"operation":operation});
    }
    Ok(reply)
}
fn error_code(value: &Value) -> &str {
    value
        .as_str()
        .filter(|s| s.len() < 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        .unwrap_or("bilibili_failed")
}
#[tauri::command]
pub async fn bilibili_suspend(
    generation: u64,
    release: Option<bool>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let s = app.state::<State>();
    let _guard = s.operation.lock().await;
    if app
        .state::<Mutex<crate::Model>>()
        .lock()
        .unwrap()
        .view
        .generation
        != generation
    {
        return Ok(());
    }
    if s.session.lock().unwrap().is_some() {
        s.request(
            &app,
            "lifecycle.suspend",
            json!({"release":release.unwrap_or(false)}),
        )
        .await?;
    }
    s.jobs.lock().unwrap().clear();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_error_codes_do_not_expose_arbitrary_service_text() {
        assert_eq!(error_code(&json!("source_tls_failed")), "source_tls_failed");
        assert_eq!(
            error_code(&json!("https://private?token=secret")),
            "bilibili_failed"
        );
    }
    #[test]
    fn lifecycle_and_account_scope() {
        let mut m = crate::Model::default();
        let v = m.transition(crate::Root::Bilibili, crate::Section::Content, true);
        assert!(allowed(&m, v.generation, "search"));
        m.transition(crate::Root::News, crate::Section::Content, true);
        let now = m.transition(crate::Root::Bilibili, crate::Section::Content, true);
        assert!(!allowed(&m, v.generation, "search"));
        assert!(allowed(&m, now.generation, "search"));
        let v = m.transition(crate::Root::Bilibili, crate::Section::Settings, true);
        m.account_page = true;
        assert!(allowed(&m, v.generation, "account.poll"));
        assert!(!allowed(&m, v.generation, "search"));
        m.transition(crate::Root::Bilibili, crate::Section::Settings, false);
        assert!(!allowed(&m, m.view.generation, "account.poll"));
    }
}
