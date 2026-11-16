//! Host-owned, readable per-run log. Services only return structured diagnostics.
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
static WRITER: Mutex<()> = Mutex::new(());
static EVENTS: OnceLock<Option<EventWriter>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub fn next_id() -> u64 {
    SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1
}
enum Event {
    Line(String),
    Flush(mpsc::Sender<()>),
}
struct EventWriter {
    sender: mpsc::SyncSender<Event>,
    dropped: Arc<AtomicU64>,
}
impl EventWriter {
    fn start(directory: PathBuf) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(128);
        let dropped = Arc::new(AtomicU64::new(0));
        let lost = dropped.clone();
        std::thread::Builder::new()
            .name("host-events".into())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    let count = lost.swap(0, Ordering::Relaxed);
                    if count > 0 {
                        let _ = write(
                            &directory,
                            &format!(
                                "[{}] [WARN] [宿主] 日志队列已满 dropped_events={}\n",
                                now().0,
                                count
                            ),
                            false,
                        );
                    }
                    match event {
                        Event::Line(text) => {
                            let _ = write(&directory, &text, false);
                        }
                        Event::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })?;
        Ok(Self { sender, dropped })
    }
    fn line(&self, text: String) -> bool {
        if self.sender.try_send(Event::Line(text)).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    fn flush(&self) -> io::Result<()> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Event::Flush(sender))
            .map_err(|_| io::Error::other("event writer closed"))?;
        receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| io::Error::other("event writer flush timed out"))
    }
}
fn now() -> (String, String) {
    let mut t = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
    (
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        ),
        format!(
            "{:04}{:02}{:02}{:02}{:02}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute
        ),
    )
}
pub fn root() -> io::Result<PathBuf> {
    Ok(crate::data_dir()?.join("logs"))
}
pub struct Session {
    directory: PathBuf,
    _lease: File,
}
impl Session {
    pub fn finish(&self) -> io::Result<()> {
        if let Some(Some(writer)) = EVENTS.get() {
            let _ = writer.flush();
        }
        let (_, stamp) = now();
        write(
            &self.directory,
            &format!("[{}] [INFO] [宿主] 应用退出 closed_at={}\n", now().0, stamp),
            true,
        )
    }
}
pub fn begin() -> io::Result<Session> {
    let directory = root()?;
    let session = begin_in(&directory)?;
    EVENTS.get_or_init(|| EventWriter::start(directory).ok());
    Ok(session)
}
fn begin_in(directory: &Path) -> io::Result<Session> {
    fs::create_dir_all(directory)?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(directory.join("LB.session"))?;
    let _guard = WRITER
        .lock()
        .map_err(|_| io::Error::other("log writer unavailable"))?;
    let path = directory.join("LB.log");
    if path.exists() {
        let mut file = File::open(&path)?;
        let size = file.metadata()?.len();
        file.seek(SeekFrom::Start(size.saturating_sub(16384)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let tail = String::from_utf8_lossy(&bytes);
        drop(file);
        let stamp = tail
            .lines()
            .rev()
            .find_map(|line| {
                line.split_once("closed_at=")
                    .map(|(_, s)| s.trim())
                    .filter(|s| s.len() == 12 && s.bytes().all(|c| c.is_ascii_digit()))
            })
            .map(str::to_owned)
            .unwrap_or_else(|| format!("unclean_{}", now().1));
        let mut backup = directory.join(format!("LB_backup_{stamp}.log"));
        let mut n = 1;
        while backup.exists() {
            backup = directory.join(format!("LB_backup_{stamp}_{n}.log"));
            n += 1;
        }
        fs::rename(&path, backup)?;
    }
    File::create(&path)?;
    append_unlocked(
        directory,
        &format!("[{}] [INFO] [宿主] 应用启动\n", now().0),
        false,
    )?;
    Ok(Session {
        directory: directory.to_owned(),
        _lease: lease,
    })
}
// Project only the public diagnostic schema. Never log arbitrary API replies/resources.
fn diagnostic(value: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for key in ["event", "stage", "mode", "quality", "exception_type"] {
        if let Some(text) = value[key]
            .as_str()
            .filter(|s| s.len() <= 80 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'))
        {
            result.insert(key.into(), Value::from(text));
        }
    }
    for key in ["source_host", "target_host"] {
        if let Some(text) = value[key].as_str().filter(|s| {
            s.len() <= 253
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        }) {
            result.insert(key.into(), Value::from(text));
        }
    }
    if let Some(series) = value["series"]
        .as_str()
        .filter(|s| s.len() <= 32 && s.bytes().all(|c| c.is_ascii_digit()))
    {
        result.insert("series".into(), Value::from(series));
    }
    for (key, max) in [
        ("episode", 9999),
        ("http_status", 599),
        ("body_bytes", 8 * 1024 * 1024 + 1),
        ("browser_code", 4),
        ("hop", 6),
    ] {
        if let Some(n) = value[key].as_u64().filter(|n| *n <= max) {
            result.insert(key.into(), Value::from(n));
        }
    }
    if let Some(b) = value["body_read_ok"].as_bool() {
        result.insert("body_read_ok".into(), Value::from(b));
    }
    let mut headers = serde_json::Map::new();
    for (key, allowed) in [
        (
            "http_version",
            &[
                "http/1.0", "http/1.1", "http/2.0", "http/3.0", "absent", "other",
            ][..],
        ),
        (
            "content_type",
            &[
                "application/json",
                "application/x-protobuf",
                "text/html",
                "application/octet-stream",
                "absent",
                "other",
            ][..],
        ),
        (
            "content_encoding",
            &["gzip", "br", "deflate", "identity", "absent", "other"][..],
        ),
        (
            "transfer_encoding",
            &["chunked", "identity", "absent", "other"][..],
        ),
    ] {
        if let Some(text) = value["response_meta"][key]
            .as_str()
            .filter(|s| allowed.contains(s))
        {
            headers.insert(key.into(), Value::from(text));
        }
    }
    for key in ["uncompressed", "request_id_present", "location_present"] {
        if let Some(b) = value["response_meta"][key].as_bool() {
            headers.insert(key.into(), Value::from(b));
        }
    }
    for key in ["declared_length", "date_delta_seconds"] {
        if let Some(n) = value["response_meta"][key].as_i64() {
            headers.insert(key.into(), Value::from(n));
        }
    }
    if !headers.is_empty() {
        result.insert("response_meta".into(), Value::Object(headers));
    }
    let stack: Vec<Value> = value["stack"]
        .as_array()
        .into_iter()
        .flatten()
        .take(6)
        .filter_map(|row| {
            let file = row["file"].as_str().filter(|s| {
                s.len() <= 128
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            })?;
            let function = row["function"].as_str().filter(|s| {
                s.len() <= 128
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_<>".contains(&c))
            })?;
            let line = row["line"].as_u64()?;
            Some(serde_json::json!({"file":file,"function":function,"line":line}))
        })
        .collect();
    if !stack.is_empty() {
        result.insert("stack".into(), Value::Array(stack));
    }
    Value::Object(result)
}
#[track_caller]
pub fn api_error(module: &str, method: &str, code: &str, details: &Value, context: Value) {
    record(
        module,
        "api_failed",
        serde_json::json!({"接口":method,"错误码":code,"诊断":diagnostic(details),"请求":request_context(&context)}),
    );
}
fn request_context(value: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for key in ["id", "elapsed_ms", "epoch", "generation"] {
        if let Some(n) = value[key].as_u64() {
            result.insert(key.into(), Value::from(n));
        }
    }
    if let Some(direct) = value["direct"].as_bool() {
        result.insert("direct".into(), Value::from(direct));
    }
    Value::Object(result)
}
fn clean(value: &str) -> String {
    value
        .chars()
        .take(512)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
fn fields(value: &Value, prefix: &str, output: &mut String, depth: usize) {
    if depth > 6 {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{}", clean(key))
                };
                let key = key
                    .replace("body_read_ok", "读取完成")
                    .replace("body_bytes", "响应体字节")
                    .replace("http_status", "HTTP状态")
                    .replace("response_meta", "响应头")
                    .replace("stage", "阶段")
                    .replace("quality", "清晰度")
                    .replace("episode", "集数");
                fields(value, &key, output, depth + 1)
            }
        }
        Value::Array(rows) if prefix.ends_with("stack") => {
            for row in rows.iter().take(6) {
                if let (Some(file), Some(line), Some(function)) = (
                    row["file"].as_str(),
                    row["line"].as_u64(),
                    row["function"].as_str(),
                ) {
                    output.push_str(&format!(
                        "  服务位置：{}:{}（{}）\n",
                        clean(file),
                        line,
                        clean(function)
                    ));
                }
            }
        }
        Value::Array(rows) => {
            for (i, row) in rows.iter().take(6).enumerate() {
                fields(row, &format!("{prefix}[{i}]"), output, depth + 1)
            }
        }
        Value::Null => {}
        _ => {
            let text = match value {
                Value::String(s) => clean(s),
                Value::Bool(b) => {
                    if *b {
                        "是".into()
                    } else {
                        "否".into()
                    }
                }
                _ => value.to_string(),
            };
            output.push_str(&format!("  {}：{}\n", clean(prefix), text));
        }
    }
}
#[track_caller]
pub fn record(module: &str, event: &str, value: Value) {
    let caller = std::panic::Location::caller();
    let text = format_record(module, event, &value, "ERROR", caller);
    if text.len() <= 128 * 1024 {
        let _ = root().and_then(|root| write(&root, &text, false));
    }
}
/// Low-volume lifecycle events; native callbacks never wait for disk I/O.
#[track_caller]
pub fn event(module: &str, event: &str, value: Value) {
    if let Some(Some(writer)) = EVENTS.get() {
        let text = format_record(
            module,
            event,
            &value,
            "INFO",
            std::panic::Location::caller(),
        );
        if text.len() <= 16 * 1024 {
            writer.line(text);
        }
    }
}
fn format_record(
    module: &str,
    event: &str,
    value: &Value,
    level: &str,
    caller: &std::panic::Location<'_>,
) -> String {
    let name = match module {
        "news" => "新闻",
        "hongguo" => "红果",
        "host" => "宿主",
        other => other,
    };
    let description = match event {
        "source_failed" => "新闻取数失败",
        "api_failed" => "接口调用失败",
        "update_check_failed" => "更新检查失败",
        other => other,
    };
    let mut text = format!(
        "[{}] [{}] [{}] #{} {}:{} {}\n",
        now().0,
        level,
        clean(name),
        next_id(),
        caller.file(),
        caller.line(),
        clean(description)
    );
    fields(value, "", &mut text, 0);
    text
}
fn write(directory: &Path, text: &str, sync: bool) -> io::Result<()> {
    let _guard = WRITER
        .lock()
        .map_err(|_| io::Error::other("log writer unavailable"))?;
    append_unlocked(directory, text, sync)
}
fn append_unlocked(directory: &Path, text: &str, sync: bool) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("LB.log"))?;
    file.write_all(text.as_bytes())?;
    if sync {
        file.sync_data()?
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn events_are_ordered_and_flushed_before_exit() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp")
            .join(format!("qa-event-log-{}", std::process::id()));
        let writer = EventWriter::start(directory.clone()).unwrap();
        assert!(writer.line("first\n".into()));
        assert!(writer.line("second\n".into()));
        writer.flush().unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("LB.log")).unwrap(),
            "first\nsecond\n"
        );
        drop(writer);
        fs::remove_file(directory.join("LB.log")).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn full_event_queue_does_not_block_and_counts_lost_events() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let writer = EventWriter {
            sender,
            dropped: Arc::new(AtomicU64::new(0)),
        };
        assert!(writer.line("first".into()));
        assert!(!writer.line("overflow".into()));
        assert_eq!(writer.dropped.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn request_context_does_not_log_credentials_or_addresses() {
        let context = request_context(
            &json!({"id":7,"elapsed_ms":120,"epoch":3,"direct":true,"cookie":"SECRET","url":"SECRET","generation":"SECRET"}),
        );
        assert_eq!(
            context,
            json!({"id":7,"elapsed_ms":120,"epoch":3,"direct":true})
        );
    }
    #[test]
    fn restart_archives_at_closed_time_and_never_overwrites() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp")
            .join(format!("qa-run-log-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let first = begin_in(&directory).unwrap();
        write(&directory, &"长日志".repeat(20000), false).unwrap();
        assert!(begin_in(&directory).is_err());
        write(&directory, "[INFO] 应用退出 closed_at=202611151024\n", true).unwrap();
        drop(first);
        let second = begin_in(&directory).unwrap();
        assert!(directory.join("LB_backup_202611151024.log").exists());
        write(&directory, "[INFO] 应用退出 closed_at=202611151024\n", true).unwrap();
        drop(second);
        let third = begin_in(&directory).unwrap();
        assert!(directory.join("LB_backup_202611151024_1.log").exists());
        third.finish().unwrap();
        drop(third);
        for entry in fs::read_dir(&directory).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn api_fields_drop_unknown_secrets_and_preserve_source_location() {
        let value = diagnostic(
            &json!({"http_status":200,"body_read_ok":true,"cookie":"SECRET","response_meta":{"content_type":"application/json","location":"SECRET"},"stack":[{"file":"app_input.py","line":37,"function":"resolve","locals":"SECRET"}]}),
        );
        let mut text = String::new();
        fields(&value, "诊断", &mut text, 0);
        assert!(!text.contains("SECRET"));
        assert!(text.contains("app_input.py:37（resolve）"));
        assert!(text.contains("HTTP状态：200"));
    }
    #[test]
    fn text_fields_are_readable_and_cannot_forge_lines() {
        let mut text = String::new();
        fields(
            &json!({"接口":"playback.status","错误":"bad\n[FAKE]"}),
            "",
            &mut text,
            0,
        );
        assert!(text.contains("接口：playback.status"));
        assert!(!text.contains("\n[FAKE]"));
    }
}
