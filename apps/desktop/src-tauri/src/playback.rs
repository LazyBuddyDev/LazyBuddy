//! Private media session control. Compressed resources go straight to WebView.
use lazybuddy_service_client::Session;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tauri::Manager;

struct Connection {
    service: Session,
    direct: bool,
    lease: Option<u64>,
}
#[derive(Default)]
pub struct State {
    connection: Mutex<Option<Connection>>,
    connecting: tokio::sync::Mutex<()>,
    epoch: AtomicU64,
    closing: AtomicBool,
    view: Mutex<(bool, bool, bool)>,
    favorite_reads: AtomicU64,
}
pub fn development_packages() -> Option<PathBuf> {
    lazybuddy_host_control::isolated()
        .then(|| std::env::var_os("LAZYBUDDY_BROWSER_PACKAGES"))
        .flatten()
        .map(PathBuf::from)
}

pub fn enabled() -> bool {
    development_packages().is_some()
        || lazybuddy_host_control::data_dir().is_ok_and(|data| {
            lazybuddy_module_manager::Manager::new(data.join("modules"))
                .manifest("hongguo")
                .is_ok_and(|m| scoped_package(&m.version))
        })
}
pub(crate) fn scoped_package(version: &str) -> bool {
    lazybuddy_module_contract::parse_version(version).is_some_and(|parts| parts >= [0, 2, 1])
}
fn direct_environment() -> Vec<(String, String)> {
    [
        ("HTTP_PROXY", ""),
        ("HTTPS_PROXY", ""),
        ("ALL_PROXY", ""),
        ("NO_PROXY", "*"),
    ]
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()))
    .collect()
}
pub fn data_directory() -> Result<PathBuf, String> {
    if lazybuddy_host_control::isolated() {
        return lazybuddy_host_control::data_dir()
            .map(|path| path.join("services/hongguo"))
            .map_err(|e| e.to_string());
    }
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("local_data_unavailable")?)
        .join("LazyBuddyShellPreview");
    Ok(base.join("hongguo"))
}
fn connect(direct: bool) -> Result<Session, String> {
    // Explicit development package directory; never load sibling service source.
    let Some(root) = development_packages() else {
        let manager = lazybuddy_module_manager::Manager::new(
            lazybuddy_host_control::data_dir()
                .map_err(|e| e.to_string())?
                .join("modules"),
        );
        let mut launch = manager.launch("hongguo").map_err(|e| e.to_string())?;
        if !scoped_package(&launch.manifest.version) {
            return Err("service_package_upgrade_required".into());
        }
        launch.environment.extend(environment(direct)?);
        return Ok(Session::with_lease(
            launch.executable,
            launch.arguments,
            launch.environment,
            Some(launch.lease),
        ));
    };
    let read = |id: &str| -> Result<lazybuddy_module_contract::Manifest, String> {
        let manifest: lazybuddy_module_contract::Manifest = serde_json::from_slice(
            &std::fs::read(root.join(id).join("module.json"))
                .map_err(|_| "service_package_unavailable")?,
        )
        .map_err(|_| "invalid_service_package")?;
        manifest.validate().map_err(|_| "invalid_service_package")?;
        if manifest.id != id {
            return Err("invalid_service_package".into());
        }
        Ok(manifest)
    };
    let feature = read("hongguo")?;
    if !scoped_package(&feature.version) {
        return Err("service_package_upgrade_required".into());
    }
    let python = read("python-runtime")?;
    if python.kind != lazybuddy_module_contract::Kind::Runtime
        || feature.runtime.as_deref() != Some(python.id.as_str())
        || feature.dependencies.len() != 1
        || !feature
            .dependencies
            .iter()
            .any(|dependency| dependency.id == python.id && dependency.version == python.version)
    {
        return Err("invalid_service_package".into());
    }
    let mut arguments = vec![
        "-I".into(),
        "-B".into(),
        root.join("hongguo")
            .join(feature.entrypoint)
            .to_string_lossy()
            .into_owned(),
    ];
    arguments.extend(feature.arguments);
    Ok(Session::with_environment(
        root.join("python-runtime").join(python.entrypoint),
        arguments,
        environment(direct)?,
    ))
}
fn environment(direct: bool) -> Result<Vec<(String, String)>, String> {
    let isolated =
        lazybuddy_host_control::isolated_credential_scope().map_err(|e| e.to_string())?;
    let progress_real = if isolated.is_none() { "1" } else { "0" };
    let mut environment = vec![
        (
            "LAZYBUDDY_INSTANCE".into(),
            isolated.clone().unwrap_or_else(|| "default".into()),
        ),
        (
            "LAZYBUDDY_CREDENTIAL_SCOPE".into(),
            isolated.unwrap_or_else(|| "shell-preview".into()),
        ),
        (
            "LAZYBUDDY_DATA_DIR".into(),
            data_directory()?.to_string_lossy().into_owned(),
        ),
        ("LAZYBUDDY_PLAYER_REAL".into(), "1".into()),
        ("LAZYBUDDY_SEARCH_REAL".into(), "1".into()),
        ("LAZYBUDDY_FAVORITES_REAL".into(), "1".into()),
        ("LAZYBUDDY_PROGRESS_REAL".into(), progress_real.into()),
    ];
    if direct {
        environment.extend(direct_environment());
    }
    Ok(environment)
}
impl State {
    pub fn favorite_reads(&self) -> u64 {
        self.favorite_reads.load(Ordering::Relaxed)
    }
    pub async fn disconnect(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            connection.service.shutdown().await;
        }
    }
    pub fn context(&self, page: bool, account: bool, visible: bool) {
        let previous = {
            let mut view = self.view.lock().unwrap();
            let previous = *view;
            *view = (page, account, visible);
            previous
        };
        if previous == (page, account, visible) {
            return;
        }
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let mut slot = self.connection.lock().unwrap();
        if let Some(connection) = slot.as_ref()
            && (page || account)
            && (visible || connection.lease.is_some())
        {
            let pending = connection.service.lifecycle(
                json!({"version":1,"id":"cancel-playback-work","method":"work.cancel","args":{}}),
            );
            tauri::async_runtime::spawn(async move {
                let _ = pending.await;
            });
        }
        if (page || account) && visible {
            if previous.0 != page || previous.1 != account {
                if slot
                    .as_ref()
                    .is_some_and(|connection| connection.lease.is_none())
                {
                    if let Some(connection) = slot.take() {
                        tauri::async_runtime::spawn(connection.service.shutdown());
                    }
                }
            }
            if !page {
                if let Some(connection) = slot.as_mut() {
                    if let Some(lease) = connection.lease.take() {
                        self.epoch.fetch_add(1, Ordering::AcqRel);
                        let future=connection.service.lifecycle(json!({"version":1,"id":"leave-player","method":"playback.close","args":{"session":lease}}));
                        tauri::async_runtime::spawn(async move {
                            let _ = future.await;
                        });
                    }
                }
            }
            return;
        }
        if !page && !account {
            if let Some(connection) = slot.take() {
                tauri::async_runtime::spawn(connection.service.shutdown());
            }
        } else if let Some(connection) = slot.as_ref() {
            if let Some(lease) = connection.lease {
                let service = connection.service.clone();
                let pending = service.lifecycle(json!({"version":1,"id":"visibility","method":"playback.suspend","args":{"session":lease}}));
                tauri::async_runtime::spawn(async move {
                    if !pending.await.is_ok_and(|reply| reply["ok"] == true) {
                        service.shutdown().await;
                    }
                });
            } else if let Some(connection) = slot.take() {
                tauri::async_runtime::spawn(connection.service.shutdown());
            }
        }
    }
    pub fn shutdown(&self) -> Option<impl std::future::Future<Output = ()> + Send + 'static> {
        if self.closing.swap(true, Ordering::AcqRel) {
            return None;
        }
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let service = self.connection.lock().unwrap().take().map(|c| c.service);
        Some(async move {
            if let Some(service) = service {
                service.shutdown().await;
            }
        })
    }
}
fn allowed(method: &str) -> bool {
    matches!(
        method,
        "playback.open"
            | "playback.prefetch"
            | "playback.prefetch_cancel"
            | "playback.report_error"
            | "playback.status"
            | "playback.close"
            | "playback.suspend"
            | "playback.resume"
            | "playback.checkpoint"
            | "danmaku.read"
            | "danmaku.cancel"
            | "account.restore"
            | "account.status"
            | "account.login"
            | "account.cancel"
            | "account.logout"
            | "search"
            | "favorites.read"
            | "favorites.set"
            | "progress.read"
            | "progress.sync"
            | "cover.read"
    )
}
#[tauri::command]
pub async fn playback_call(
    method: String,
    args: Value,
    direct: Option<bool>,
    generation: u64,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, State>,
) -> Result<Value, String> {
    if !allowed(&method) || !args.is_object() {
        return Err("invalid_playback_request".into());
    }
    if state.closing.load(Ordering::Acquire) {
        return Err("application_closing".into());
    }
    let checkpoint = method == "playback.checkpoint";
    let account_method = method.starts_with("account.");
    let model = window.state::<Mutex<crate::Model>>();
    let admissible = |model: &crate::Model| {
        let content = model.view.section == crate::Section::Content;
        let account =
            model.view.section == crate::Section::Settings && model.account_page && account_method;
        let same_activation = model.view.generation == generation
            || checkpoint
                && !model.view.visible
                && model.view.generation.checked_sub(1) == Some(generation);
        same_activation
            && model.view.root == crate::Root::Hongguo
            && (content || account || checkpoint)
            && (model.view.visible || checkpoint)
            && !state.closing.load(Ordering::Acquire)
    };
    let direct = direct.unwrap_or(true);
    let connecting = state.connecting.lock().await;
    let (epoch, previous) = {
        let model = model.lock().unwrap();
        if !admissible(&model) {
            return Err("inactive_media_page".into());
        }
        let mut slot = state.connection.lock().unwrap();
        if slot
            .as_ref()
            .is_some_and(|connection| connection.direct != direct)
        {
            if checkpoint {
                return Err("stale_playback_request".into());
            }
            (state.epoch.fetch_add(1, Ordering::AcqRel) + 1, slot.take())
        } else {
            (state.epoch.load(Ordering::Acquire), None)
        }
    };
    if let Some(connection) = previous {
        connection.service.shutdown().await;
    }
    let deferred = matches!(
        method.as_str(),
        "search" | "cover.read" | "account.restore" | "account.cancel"
    );
    let (service, pending, scope, scope_generation) = {
        let model = model.lock().unwrap();
        let mut slot = state.connection.lock().unwrap();
        if !admissible(&model) || epoch != state.epoch.load(Ordering::Acquire) {
            return Err("stale_playback_request".into());
        }
        if slot.is_none() {
            if checkpoint {
                return Err("stale_playback_request".into());
            }
            *slot = Some(Connection {
                service: connect(direct)?,
                direct,
                lease: None,
            });
        }
        let service = slot.as_ref().unwrap().service.clone();
        let scope = model.active_generation.clone();
        let scope_generation = model.view.generation;
        let request = if deferred {
            json!({"version":1,"id":"web-playback","method":"work.begin","args":{"method":method,"args":args}})
        } else {
            json!({"version":1,"id":"web-playback","method":method,"args":args})
        };
        let pending = service.request_scoped(request, scope.clone(), scope_generation);
        (service, pending, scope, scope_generation)
    };
    drop(connecting);
    if method == "favorites.read" {
        state.favorite_reads.fetch_add(1, Ordering::Relaxed);
    }
    let request_id = lazybuddy_host_control::logs::next_id();
    let began = std::time::Instant::now();
    let reply = pending.await;
    let reply = if deferred {
        finish_work(&service, scope, scope_generation, &method, args, reply).await
    } else {
        reply
    };
    let model = model.lock().unwrap();
    let mut slot = state.connection.lock().unwrap();
    if !admissible(&model)
        || epoch != state.epoch.load(Ordering::Acquire)
        || !slot
            .as_ref()
            .is_some_and(|c| c.service.same_connection(&service))
    {
        return Err("stale_playback_request".into());
    }
    let reply = match reply {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
            return Err("stale_playback_request".into());
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            return Err("request_busy".into());
        }
        Err(_) => {
            if let Some(connection) = slot.take() {
                connection.service.stop();
            }
            lazybuddy_host_control::logs::record(
                "hongguo",
                "api_failed",
                json!({"接口":method,"错误码":"playback_service_unavailable","request_id":request_id,"epoch":epoch,"direct":direct,"elapsed_ms":began.elapsed().as_millis()}),
            );
            return Err("playback_service_unavailable".into());
        }
    };
    let data = &reply["data"];
    let error = if reply["ok"] != true {
        reply["error"]["code"].as_str()
    } else {
        data["error"]
            .as_str()
            .or_else(|| data["progress_error"].as_str())
            .or_else(|| data["favorites_error"].as_str())
            .or_else(|| {
                data["diagnostic"]["event"]
                    .as_str()
                    .filter(|s| *s == "browser_media_error")
            })
    };
    if let Some(code) =
        error.filter(|s| s.len() < 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
    {
        let diagnostic = if reply["ok"] == true {
            &data["diagnostic"]
        } else {
            &reply["error"]["diagnostic"]
        };
        lazybuddy_host_control::logs::api_error(
            "hongguo",
            &method,
            code,
            diagnostic,
            json!({"id":request_id,"epoch":epoch,"direct":direct,"elapsed_ms":began.elapsed().as_millis() as u64}),
        );
    }
    if reply["ok"] != true {
        return Err(reply["error"]["code"]
            .as_str()
            .filter(|s| s.len() < 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
            .unwrap_or("playback_failed")
            .to_owned());
    }
    if let Some(connection) = slot.as_mut() {
        if method.starts_with("playback.") {
            connection.lease = resource_session(&reply["data"]);
        }
    }
    Ok(reply["data"].clone())
}

async fn finish_work(
    service: &Session,
    scope: std::sync::Arc<AtomicU64>,
    generation: u64,
    method: &str,
    args: Value,
    reply: std::io::Result<Value>,
) -> std::io::Result<Value> {
    let reply = reply?;
    if reply["error"]["code"] == "unknown_method" {
        return service
            .request_scoped(
                json!({"version":1,"id":"web-playback","method":method,"args":args}),
                scope,
                generation,
            )
            .await;
    }
    if reply["ok"] != true {
        return Ok(reply);
    }
    let job = reply["data"]["job"]
        .as_u64()
        .ok_or_else(|| std::io::Error::other("invalid media work receipt"))?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(65);
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let mut result = service
            .request_scoped(
                json!({"version":1,"id":"web-playback","method":"work.status","args":{"job":job}}),
                scope.clone(),
                generation,
            )
            .await?;
        if result["ok"] != true {
            return Ok(result);
        }
        if result["data"]["pending"] == true {
            if tokio::time::Instant::now() >= deadline {
                return Ok(json!({"ok":false,"error":{"code":"work_cancelled"}}));
            }
            continue;
        }
        return result["data"]
            .get_mut("reply")
            .map(Value::take)
            .ok_or_else(|| std::io::Error::other("missing media work result"));
    }
}
fn resource_session(data: &Value) -> Option<u64> {
    if data["loading"] == true || data["prepared"] == true {
        data["session"].as_u64()
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_upload_is_enabled_only_for_the_normal_instance() {
        if std::env::var_os("LAZYBUDDY_PROGRESS_ENV_TEST").is_some() {
            let values = environment(false).unwrap();
            assert!(values.contains(&("LAZYBUDDY_PROGRESS_REAL".into(), "0".into())));
            assert!(values.contains(&(
                "LAZYBUDDY_CREDENTIAL_SCOPE".into(),
                "isolated-progress-env-test".into()
            )));
            return;
        }
        let values = environment(false).unwrap();
        assert!(values.contains(&("LAZYBUDDY_PROGRESS_REAL".into(), "1".into())));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "playback::tests::progress_upload_is_enabled_only_for_the_normal_instance",
            ])
            .env("LAZYBUDDY_PROGRESS_ENV_TEST", "1")
            .env("LAZYBUDDY_INSTANCE", "progress-env-test")
            .env(
                "LAZYBUDDY_DATA_DIR",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .ancestors()
                    .nth(3)
                    .unwrap()
                    .join("tmp/progress-env-test"),
            )
            .status()
            .unwrap();
        assert!(status.success());
    }
    #[test]
    fn direct_mode_overrides_only_child_proxy_environment() {
        let environment = direct_environment();
        assert_eq!(environment.len(), 4);
        assert!(environment.contains(&("NO_PROXY".into(), "*".into())));
        for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
            assert!(environment.contains(&(key.into(), String::new())));
        }
    }
    #[test]
    fn closed_resource_does_not_keep_a_decoder_lease() {
        assert_eq!(
            resource_session(&json!({"session":42,"prepared":false,"loading":false})),
            None
        );
        assert_eq!(
            resource_session(&json!({"session":42,"loading":true})),
            Some(42)
        );
    }
    #[test]
    fn browser_bridge_only_exposes_media_session_operations() {
        assert!(allowed("playback.open"));
        assert!(allowed("playback.prefetch"));
        assert!(allowed("playback.prefetch_cancel"));
        assert!(allowed("account.login"));
        assert!(!allowed("media.open"));
        assert!(allowed("progress.sync"));
    }
    #[test]
    fn old_packages_cannot_silently_use_production_credentials() {
        assert!(!scoped_package("0.2.0"));
        assert!(scoped_package("0.2.1"));
        assert!(scoped_package("0.10.0"));
        assert!(!scoped_package("bad"));
    }
}
