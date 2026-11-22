//! One service actor per active feature. Idle actors block without polling.
use crate::Client;
use futures::channel::oneshot;
use serde_json::Value;
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
};

enum Command {
    Call(
        Value,
        Option<(Arc<AtomicU64>, u64)>,
        oneshot::Sender<io::Result<Value>>,
    ),
    Stop,
    Finish(bool, oneshot::Sender<()>),
}

struct Inner {
    commands: mpsc::Sender<Command>,
    queued: Arc<std::sync::atomic::AtomicUsize>,
    cancelled: Arc<AtomicBool>,
    graceful: AtomicBool,
    #[cfg(windows)]
    stopper: Arc<std::sync::Mutex<Option<Arc<crate::job::Job>>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.graceful.load(Ordering::Acquire) {
            return;
        }
        self.cancelled.store(true, Ordering::Release);
        #[cfg(windows)]
        if let Some(job) = self.stopper.lock().unwrap().as_ref() {
            job.stop();
        }
        let _ = self.commands.send(Command::Stop);
    }
}

#[derive(Clone)]
pub struct Session(Arc<Inner>);

impl Session {
    pub fn start(executable: PathBuf, arguments: Vec<String>) -> Self {
        Self::with_environment(executable, arguments, vec![])
    }

    pub fn with_environment(
        executable: PathBuf,
        arguments: Vec<String>,
        environment: Vec<(String, String)>,
    ) -> Self {
        Self::with_lease(executable, arguments, environment, None)
    }

    pub fn with_lease(
        executable: PathBuf,
        arguments: Vec<String>,
        environment: Vec<(String, String)>,
        lease: Option<std::fs::File>,
    ) -> Self {
        let (commands, receiver) = mpsc::channel();
        let queued = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker_queued = queued.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        #[cfg(windows)]
        let stopper = Arc::new(std::sync::Mutex::new(None));
        #[cfg(windows)]
        let worker_stopper = stopper.clone();
        std::thread::spawn(move || {
            let mut lease = lease;
            if worker_cancelled.load(Ordering::Acquire) {
                return;
            }
            let mut client = Client::start_with_environment(&executable, &arguments, &environment);
            #[cfg(windows)]
            if let Ok(client) = &client {
                *worker_stopper.lock().unwrap() = Some(client.job.clone());
            }
            while let Ok(command) = receiver.recv() {
                if worker_cancelled.load(Ordering::Acquire) {
                    break;
                }
                match command {
                    Command::Stop => break,
                    Command::Finish(cleanup, reply) => {
                        if cleanup && let Ok(client) = &mut client {
                            let _=client.request(&serde_json::json!({"version":1,"id":"shutdown","method":"lifecycle.suspend","args":{}}));
                        }
                        drop(client);
                        drop(lease.take());
                        let _ = reply.send(());
                        return;
                    }
                    Command::Call(request, scope, reply) => {
                        worker_queued.fetch_sub(1, Ordering::AcqRel);
                        if scope.is_some_and(|(current, generation)| {
                            current.load(Ordering::Acquire) != generation
                        }) {
                            let _ = reply.send(Err(io::Error::new(
                                io::ErrorKind::Interrupted,
                                "stale service request",
                            )));
                            continue;
                        }
                        if reply.is_canceled() && abandoned_read(&request) {
                            continue;
                        }
                        let result = match &mut client {
                            Ok(client) => client.request_cancellable(&request, &worker_cancelled),
                            Err(_) => Err(io::Error::other("service startup failed")),
                        };
                        let disconnected = result.is_err();
                        let _ = reply.send(result);
                        if disconnected {
                            break;
                        }
                    }
                }
            }
            // Client's Drop tears down its Windows Job and joins its pipe reader.
        });
        Self(Arc::new(Inner {
            commands,
            queued,
            cancelled,
            graceful: AtomicBool::new(false),
            #[cfg(windows)]
            stopper,
        }))
    }

    pub fn request(
        &self,
        request: Value,
    ) -> impl std::future::Future<Output = io::Result<Value>> + Send + 'static {
        self.enqueue(request, None, true)
    }

    pub fn request_scoped(
        &self,
        request: Value,
        current: Arc<AtomicU64>,
        generation: u64,
    ) -> impl std::future::Future<Output = io::Result<Value>> + Send + 'static {
        self.enqueue(request, Some((current, generation)), true)
    }

    /// Lifecycle controls are queued synchronously in view order, ahead of later page work.
    pub fn lifecycle(
        &self,
        request: Value,
    ) -> impl std::future::Future<Output = io::Result<Value>> + Send + 'static {
        self.enqueue(request, None, false)
    }

    fn enqueue(
        &self,
        request: Value,
        scope: Option<(Arc<AtomicU64>, u64)>,
        counted: bool,
    ) -> impl std::future::Future<Output = io::Result<Value>> + Send + 'static {
        let (sender, receiver) = oneshot::channel();
        let limit = if counted { 32 } else { 40 };
        let full = self
            .0
            .queued
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < limit).then_some(n + 1)
            })
            .is_err();
        let sent = if full {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "service request queue full",
            ))
        } else if self
            .0
            .commands
            .send(Command::Call(request, scope, sender))
            .is_err()
        {
            self.0.queued.fetch_sub(1, Ordering::AcqRel);
            Err(io::Error::other("service session stopped"))
        } else {
            Ok(())
        };
        async move {
            sent?;
            receiver
                .await
                .map_err(|_| io::Error::other("service request cancelled"))?
        }
    }

    pub fn same_connection(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn stop(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        #[cfg(windows)]
        if let Some(job) = self.0.stopper.lock().unwrap().as_ref() {
            job.stop();
        }
        let _ = self.0.commands.send(Command::Stop);
    }

    /// Finish a final persistence transaction, then tear down the owned process tree.
    pub fn shutdown(self) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.finish(true)
    }

    pub fn close_after_cleanup(self) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.finish(false)
    }

    fn finish(self, cleanup: bool) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.0.graceful.store(true, Ordering::Release);
        let (sender, receiver) = oneshot::channel();
        let _ = self.0.commands.send(Command::Finish(cleanup, sender));
        async move {
            let _ = receiver.await;
        }
    }
}

fn abandoned_read(request: &Value) -> bool {
    matches!(
        request["method"].as_str(),
        Some(
            "health"
                | "sources"
                | "state"
                | "preferences"
                | "fetch"
                | "search"
                | "recommend"
                | "following"
                | "task.status"
                | "playback.status"
                | "progress.read"
                | "cover.read"
                | "library.list"
                | "history.list"
                | "chapter.read"
                | "chapter.find"
                | "resource.read"
                | "online.search"
                | "online.detail"
                | "online.status"
                | "preferences.get"
        )
    )
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServiceSession")
            .field("stopped", &self.0.cancelled.load(Ordering::Acquire))
            .finish()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cancelling_a_blocked_pipe_write_does_not_wait_for_the_service() {
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 30".into(),
            ],
        );
        let pending = session.request(
            json!({"version":1,"id":"blocked","method":"fixture","pad":"x".repeat(60000)}),
        );
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(futures::executor::block_on(pending));
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        session.stop();
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn lifecycle_timeout_covers_a_blocked_pipe_write() {
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 30".into(),
            ],
        );
        let (send, received) = std::sync::mpsc::channel();
        let request = session.request(json!({"version":1,"id":"blocked","method":"lifecycle.suspend","pad":"x".repeat(60000)}));
        std::thread::spawn(move || {
            let _ = send.send(futures::executor::block_on(request));
        });
        assert!(
            received
                .recv_timeout(std::time::Duration::from_secs(12))
                .unwrap()
                .is_err()
        );
        assert!(
            futures::executor::block_on(
                session.request(json!({"version":1,"id":"next","method":"health"}))
            )
            .is_err()
        );
    }

    #[test]
    fn stale_queued_work_is_discarded_without_stopping_the_connection() {
        let script = r#"while($line=[Console]::ReadLine()){$r=$line|ConvertFrom-Json;[Console]::WriteLine((@{version=1;id=$r.id;ok=$true;data=@{method=$r.method}}|ConvertTo-Json -Compress))}"#;
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec!["-NoProfile".into(), "-Command".into(), script.into()],
        );
        let current = Arc::new(AtomicU64::new(2));
        let old = session.request_scoped(
            json!({"version":1,"id":"old","method":"search"}),
            current.clone(),
            1,
        );
        assert_eq!(
            futures::executor::block_on(old).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        let fresh = session.request_scoped(
            json!({"version":1,"id":"new","method":"health"}),
            current,
            2,
        );
        assert_eq!(
            futures::executor::block_on(fresh).unwrap()["data"]["method"],
            "health"
        );
    }

    #[test]
    fn ordinary_queue_is_bounded_and_lifecycle_has_reserved_capacity() {
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 30".into(),
            ],
        );
        let request = || json!({"version":1,"id":"normal","method":"health"});
        let mut queued = (0..34)
            .map(|_| session.request(request()))
            .collect::<Vec<_>>();
        let newest = queued.pop().unwrap();
        assert_eq!(
            futures::executor::block_on(newest).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let cleanup =
            session.lifecycle(json!({"version":1,"id":"cleanup","method":"lifecycle.suspend"}));
        session.stop();
        assert_ne!(
            futures::executor::block_on(cleanup).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn graceful_shutdown_finishes_the_cleanup_rpc_before_stopping() {
        let script = r#"while($line=[Console]::ReadLine()){$r=$line|ConvertFrom-Json;if($r.method -eq 'lifecycle.suspend'){Start-Sleep -Milliseconds 100};[Console]::WriteLine((@{version=1;id=$r.id;ok=$true;data=@{pid=$PID}}|ConvertTo-Json -Compress))}"#;
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                script.into(),
            ],
        );
        futures::executor::block_on(
            session.request(json!({"version":1,"id":"health","method":"health"})),
        )
        .unwrap();
        let started = std::time::Instant::now();
        futures::executor::block_on(session.shutdown());
        assert!(started.elapsed() >= std::time::Duration::from_millis(80));
    }

    #[test]
    fn session_reuses_process_and_cancellation_stops_an_inflight_request() {
        let script = r#"$n=0;while($line=[Console]::ReadLine()){$r=$line|ConvertFrom-Json;if($r.method -eq 'block'){Start-Sleep -Seconds 30};$n++;[Console]::WriteLine((@{version=1;id=$r.id;ok=$true;data=@{pid=$PID;count=$n}}|ConvertTo-Json -Compress))}"#;
        let session = Session::start(
            PathBuf::from("pwsh.exe"),
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                script.into(),
            ],
        );
        let request = |method: &str| json!({"version":1,"id":"test","method":method});
        let first = futures::executor::block_on(session.request(request("echo"))).unwrap();
        let second = futures::executor::block_on(session.request(request("echo"))).unwrap();
        assert_eq!(first["data"]["pid"], second["data"]["pid"]);
        assert_eq!(second["data"]["count"], 2);
        let pid = first["data"]["pid"].as_u64().unwrap() as u32;
        let process =
            unsafe { windows_sys::Win32::System::Threading::OpenProcess(0x00100000, 0, pid) };
        assert!(!process.is_null());
        let pending = session.request(request("block"));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let started = std::time::Instant::now();
        session.stop();
        assert!(futures::executor::block_on(pending).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(futures::executor::block_on(session.request(request("echo"))).is_err());
        unsafe {
            assert_eq!(
                windows_sys::Win32::System::Threading::WaitForSingleObject(process, 2000),
                0
            );
            windows_sys::Win32::Foundation::CloseHandle(process);
        }
    }
}
