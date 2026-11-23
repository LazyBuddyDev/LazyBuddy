use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub attention_receipts: std::collections::BTreeMap<String, String>,
    #[serde(default = "enabled")]
    pub auto_update: bool,
    pub window: Option<[i32; 4]>,
    pub shortcut: super::shortcut::Shortcut,
    pub dark: bool,
    pub root: Option<String>,
    pub topmost: bool,
    #[serde(default = "default_opacity")]
    pub background_opacity: f32,
    #[serde(default = "default_opacity")]
    pub card_opacity: f32,
}
fn enabled() -> bool {
    true
}
fn default_opacity() -> f32 {
    0.94
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            attention_receipts: Default::default(),
            auto_update: true,
            window: None,
            shortcut: Default::default(),
            dark: false,
            root: None,
            topmost: false,
            background_opacity: default_opacity(),
            card_opacity: default_opacity(),
        }
    }
}

type Save = (
    Preferences,
    futures::channel::oneshot::Sender<io::Result<()>>,
);
pub struct Writer {
    sender: Option<std::sync::mpsc::Sender<Save>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Writer {
    pub fn new(path: std::path::PathBuf) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Save>();
        let worker = std::thread::spawn(move || {
            while let Ok((preferences, reply)) = receiver.recv() {
                let _ = reply.send(preferences.save(&path));
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
        }
    }
    pub fn save(
        &self,
        preferences: Preferences,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send + 'static {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let sent = self
            .sender
            .as_ref()
            .expect("settings writer running")
            .send((preferences, sender));
        async move {
            if sent.is_err() {
                return Err("设置保存进程不可用。".into());
            }
            receiver
                .await
                .map_err(|_| "设置保存失败。".to_owned())?
                .map_err(|_| "设置保存失败。".into())
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Preferences {
    pub fn valid_attention(&self) -> bool {
        self.attention_receipts.len() <= 2048
            && self.attention_receipts.iter().all(|(key, value)| {
                !key.is_empty()
                    && key.len() <= 240
                    && !value.is_empty()
                    && value.len() <= 160
                    && !key.chars().chain(value.chars()).any(char::is_control)
            })
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        if fs::metadata(path).is_ok_and(|metadata| metadata.len() > 1024 * 1024) {
            return Err(io::Error::other("preferences file too large"));
        }
        match fs::read(path) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => {
                let value: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                if value.shortcut.supported()
                    && value.valid_attention()
                    && value.window.is_none_or(|[x, y, w, h]| {
                        (-100_000..=100_000).contains(&x)
                            && (-100_000..=100_000).contains(&y)
                            && (400..=16_384).contains(&w)
                            && (240..=16_384).contains(&h)
                    })
                    && (0.0..=1.0).contains(&value.background_opacity)
                    && (0.0..=1.0).contains(&value.card_opacity)
                {
                    Ok(value)
                } else {
                    Err(io::Error::other("invalid preferences"))
                }
            }
            Ok(_) => Err(io::Error::other("preferences file too large")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path.parent().expect("preferences directory"))?;
        let temporary = path.with_extension("tmp");
        let mut file = fs::File::create(&temporary)?;
        serde_json::to_writer(&mut file, self)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_updates_default_on_for_existing_settings_and_keep_explicit_opt_out() {
        let old: Preferences = serde_json::from_str(r#"{"dark":true}"#).unwrap();
        assert!(old.auto_update && old.dark);
        let disabled: Preferences = serde_json::from_str(r#"{"auto_update":false}"#).unwrap();
        assert!(!disabled.auto_update);
        assert!(old.attention_receipts.is_empty());
    }
    #[test]
    fn attention_receipts_round_trip_and_reject_oversized_or_control_data() {
        let mut p = Preferences::default();
        p.attention_receipts
            .insert("modules/update:sample".into(), "1.2.3".into());
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(p.attention_receipts, restored.attention_receipts);
        assert!(restored.valid_attention());
        p.attention_receipts
            .insert("invalid\nkey".into(), "1".into());
        assert!(!p.valid_attention());
        p.attention_receipts.clear();
        p.attention_receipts.insert("key".into(), "x".repeat(161));
        assert!(!p.valid_attention());
    }
    #[test]
    fn malformed_settings_are_not_replaced_with_defaults() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tmp")
            .join(format!("qa-preferences-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("preferences.json");
        assert!(Preferences::load(&path).is_ok());
        let mut preferences = Preferences::default();
        preferences.save(&path).unwrap();
        preferences.dark = true;
        preferences.window = Some([-800, 40, 780, 560]);
        preferences.save(&path).unwrap();
        assert!(Preferences::load(&path).unwrap().dark);
        let restored = Preferences::load(&path).unwrap();
        assert_eq!(restored.window, Some([-800, 40, 780, 560]));
        fs::write(&path, b"broken").unwrap();
        assert!(Preferences::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        fs::remove_dir_all(directory).unwrap();
    }
}
