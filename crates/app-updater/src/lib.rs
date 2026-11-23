//! Signed portable-host updates. Feature packages and user data are never replaced.
#[cfg(test)]
mod tests;
pub mod uninstall;
mod windows;
use lazybuddy_module_contract::{package_path, version};
use lazybuddy_module_manager::package::{Source, download_archive, download_signed_payload};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const SOURCE: &str = "app-update-source.json";
const INFO: &str = "build-info.json";
const RECOVERY: &str = ".lazybuddy-update-transaction";
const JOURNAL: &str = "journal.json";
const INSTALL_LOCK: &str = ".lazybuddy-update.lock";
const UPDATE_TRIAL: &str = "LAZYBUDDY_UPDATE_TRIAL";
const REQUIRED: [&str; 6] = [
    "lazybuddy.exe",
    "lazybuddy-cli.exe",
    "lazybuddy-updater.exe",
    "uninstall.exe",
    SOURCE,
    "module-catalog.json",
];
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub target: String,
    pub version: String,
    pub build: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    pub host_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInfo {
    pub schema: u32,
    pub version: String,
    pub build: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub files: BTreeMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct Prepared {
    pub directory: PathBuf,
    pub version: String,
    pub build: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    install: PathBuf,
    parent_pid: u32,
}

fn hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn file_digest(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    if fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(invalid("update metadata too large"));
    }
    serde_json::from_slice(&fs::read(path)?).map_err(|_| invalid("invalid update metadata"))
}
pub fn verify_manifest(payload: &[u8], signature: &[u8], key: &[u8]) -> io::Result<Manifest> {
    if payload.len() > 1024 * 1024 {
        return Err(invalid("update manifest too large"));
    }
    UnparsedPublicKey::new(&ED25519, key)
        .verify(payload, signature)
        .map_err(|_| invalid("invalid update signature"))?;
    let manifest: Manifest =
        serde_json::from_slice(payload).map_err(|_| invalid("invalid update manifest"))?;
    if manifest.schema != 1
        || manifest.target != "windows-x86_64"
        || !version(&manifest.version)
        || !hex(&manifest.build)
        || !hex(&manifest.sha256)
        || !hex(&manifest.host_sha256)
        || !manifest.url.starts_with("https://")
        || manifest.sequence == Some(0)
        || manifest.bytes == 0
        || manifest.bytes > 256 * 1024 * 1024
    {
        return Err(invalid("unsupported update manifest"));
    }
    Ok(manifest)
}
fn owned(name: &str) -> bool {
    package_path(name)
        && (REQUIRED.contains(&name)
            || matches!(name, "README.txt" | "LICENSE" | INFO)
            || name.starts_with("licenses/"))
}
fn plain(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("linked update path"));
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(invalid("linked update path"));
    }
    Ok(())
}
fn destination(root: &Path, name: &str) -> io::Result<PathBuf> {
    if !owned(name) {
        return Err(invalid("unexpected application file"));
    }
    plain(root)?;
    let mut path = root.to_owned();
    for part in Path::new(name).components() {
        path.push(part);
        match plain(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(path)
}
fn read_build_info(path: &Path) -> io::Result<BuildInfo> {
    let info: BuildInfo = read_json(path)?;
    if info.schema != 1
        || !version(&info.version)
        || info.sequence == Some(0)
        || info.files.len() > 10000
        || digest(&serde_json::to_vec(&info.files).map_err(io::Error::other)?) != info.build
        || REQUIRED.iter().any(|name| !info.files.contains_key(*name))
    {
        return Err(invalid("application build mismatch"));
    }
    Ok(info)
}

fn build_info(stage: &Path) -> io::Result<BuildInfo> {
    let info = read_build_info(&stage.join(INFO))?;
    for (name, expected) in &info.files {
        if !hex(expected) || file_digest(&destination(stage, name)?)? != *expected {
            return Err(invalid("application file digest mismatch"));
        }
    }
    Ok(info)
}
pub fn verify_stage(stage: &Path, manifest: &Manifest) -> io::Result<BuildInfo> {
    let info = build_info(stage)?;
    match_manifest(&info, manifest)?;
    Ok(info)
}

fn match_manifest(info: &BuildInfo, manifest: &Manifest) -> io::Result<()> {
    if info.version != manifest.version
        || info.sequence != manifest.sequence
        || info.build != manifest.build
        || info.files.get("lazybuddy.exe") != Some(&manifest.host_sha256)
    {
        return Err(invalid("application build mismatch"));
    }
    Ok(())
}
fn current(install: &Path, manifest: &Manifest) -> bool {
    read_json::<BuildInfo>(&install.join(INFO)).is_ok_and(|info| {
        info.version == manifest.version
            && info.build == manifest.build
            && info.sequence == manifest.sequence
    }) && file_digest(&install.join("lazybuddy.exe")).is_ok_and(|hash| hash == manifest.host_sha256)
}
fn update_allowed(install: &Path, manifest: &Manifest) -> io::Result<bool> {
    let installed: BuildInfo = read_json(&install.join(INFO))?;
    if installed.schema != 1 || !version(&installed.version) || installed.sequence == Some(0) {
        return Err(invalid("invalid installed build marker"));
    }
    let numeric = |version: &str| {
        version
            .split('.')
            .map(|part| part.parse::<u32>().unwrap())
            .collect::<Vec<_>>()
    };
    if !version(&manifest.version) || numeric(&manifest.version) < numeric(&installed.version) {
        return Ok(false);
    }
    if installed.version == manifest.version
        && installed.build == manifest.build
        && installed.sequence == manifest.sequence
    {
        return Ok(true); // The signed build can repair its own files.
    }
    Ok(match (installed.sequence, manifest.sequence) {
        (Some(current), Some(incoming)) => incoming > current,
        (None, Some(_)) => true,
        (None, None) => numeric(&manifest.version) > numeric(&installed.version),
        (Some(_), None) => false,
    })
}

fn unpack(archive: &Path, stage: &Path, manifest: &Manifest) -> io::Result<()> {
    if fs::metadata(archive)?.len() != manifest.bytes || file_digest(archive)? != manifest.sha256 {
        return Err(invalid("application archive digest mismatch"));
    }
    fs::create_dir(stage)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(archive)?)
        .map_err(|_| invalid("invalid application ZIP"))?;
    if archive.len() > 10000 {
        return Err(invalid("too many application files"));
    }
    let mut budget = 512 * 1024 * 1024u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| invalid("invalid application ZIP entry"))?;
        let name = entry
            .name()
            .strip_prefix("LazyBuddy/")
            .ok_or_else(|| invalid("unexpected application ZIP root"))?;
        if !owned(name)
            || entry.is_dir()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(invalid("unsafe application ZIP path"));
        }
        budget = budget
            .checked_sub(entry.size())
            .ok_or_else(|| invalid("application ZIP too large"))?;
        let path = destination(stage, name)?;
        fs::create_dir_all(path.parent().unwrap())?;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)?;
        let expected = entry.size();
        if io::copy(&mut entry.take(expected + 1), &mut output)? != expected {
            return Err(invalid("incomplete application ZIP entry"));
        }
    }
    verify_stage(stage, manifest)?;
    Ok(())
}
fn clear_directory(path: &Path) -> io::Result<()> {
    plain(path)?;
    // This function is used only on updater-owned cache directories, never installs.
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        plain(&entry.path())?;
        if entry.file_type()?.is_dir() {
            clear_directory(&entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    fs::remove_dir(path)
}
fn prune(cache: &Path, keep: Option<&str>) {
    if let Ok(entries) = fs::read_dir(cache) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if hex(&name) && Some(name.as_ref()) != keep {
                let lock = entry.path().join("apply.lock");
                if lock.exists() && windows::lock(&lock).is_err() {
                    continue;
                }
                let _ = clear_directory(&entry.path());
            }
        }
    }
}
pub fn prepare(install: &Path, cache: &Path) -> io::Result<Option<Prepared>> {
    let recovery = install.join(RECOVERY);
    if !install.join(INFO).is_file()
        || recovery.join(JOURNAL).exists()
        || uninstall::removal_pending(install)?
    {
        return Ok(None);
    }
    if recovery.is_dir() {
        if let Ok(_lock) = windows::lock(&install.join(INSTALL_LOCK)) {
            if !recovery.join(JOURNAL).exists() {
                let _ = clear_directory(&recovery);
            }
        }
    }
    let source = Source::load(&install.join(SOURCE))?;
    let (payload, signature) = download_signed_payload(&source.url, &source.public_key)?;
    let manifest = verify_manifest(&payload, &signature, &source.public_key)?;
    if current(install, &manifest) || !update_allowed(install, &manifest)? {
        prune(cache, None);
        return Ok(None);
    }
    fs::create_dir_all(cache)?;
    plain(cache)?;
    let directory = cache.join(&manifest.build);
    let stage = directory.join("stage");
    if verify_stage(&stage, &manifest).is_err() {
        if directory.exists() {
            clear_directory(&directory)?;
        }
        fs::create_dir(&directory)?;
        let archive = directory.join("package.zip");
        let result = (|| {
            download_archive(&manifest.url, &archive, manifest.bytes)?;
            unpack(&archive, &stage, &manifest)
        })();
        if let Err(error) = result {
            let _ = clear_directory(&directory);
            return Err(error);
        }
    }
    fs::write(directory.join("manifest.json"), payload)?;
    fs::write(directory.join("manifest.json.sig"), signature)?;
    prune(cache, Some(&manifest.build));
    Ok(Some(Prepared {
        directory,
        version: manifest.version,
        build: manifest.build,
    }))
}
pub fn start_helper(prepared: &Prepared, install: &Path) -> io::Result<()> {
    if uninstall::removal_pending(install)? {
        return Err(invalid("uninstallation is pending"));
    }
    if install.join(RECOVERY).join(JOURNAL).exists() {
        return Err(invalid("update recovery is pending"));
    }
    let directory = &prepared.directory;
    plain(directory)?;
    let source = Source::load(&install.join(SOURCE))?;
    let manifest = verify_manifest(
        &fs::read(directory.join("manifest.json"))?,
        &fs::read(directory.join("manifest.json.sig"))?,
        &source.public_key,
    )?;
    if !update_allowed(install, &manifest)? {
        return Err(invalid("update is older than the installed release"));
    }
    verify_stage(&directory.join("stage"), &manifest)?;
    let ready = directory.join("ready");
    if ready.exists() {
        fs::remove_file(&ready)?;
    }
    fs::write(
        directory.join("job.json"),
        serde_json::to_vec(&Job {
            install: install.canonicalize()?,
            parent_pid: std::process::id(),
        })
        .map_err(io::Error::other)?,
    )?;
    fs::copy(
        directory.join("stage/lazybuddy-updater.exe"),
        directory.join("apply.exe"),
    )?;
    let child = windows::command(&directory.join("apply.exe"))
        .arg("--apply")
        .arg(directory.join("job.json"))
        .spawn()?;
    wait_ready(child, &ready)
}

fn wait_ready(mut child: std::process::Child, ready: &Path) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ready.is_file() {
            return Ok(());
        }
        if child.try_wait()?.is_some() {
            return Err(invalid("update helper could not prepare restart"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(invalid("update helper startup timed out"))
}

struct Transaction {
    install: PathBuf,
    backup: PathBuf,
    changed: Vec<(String, bool)>,
}
impl Transaction {
    fn new(install: &Path, backup: &Path) -> io::Result<Self> {
        if backup.join(JOURNAL).exists() {
            return Err(invalid("update recovery is pending"));
        }
        if backup.exists() {
            clear_directory(backup)?;
        }
        fs::create_dir(backup)?;
        plain(install)?;
        Ok(Self {
            install: install.to_owned(),
            backup: backup.to_owned(),
            changed: vec![],
        })
    }
    fn replace(&mut self, stage: &Path, info: &BuildInfo) -> io::Result<()> {
        // A first-time upgrade from a legacy host needs the new startup recovery hook.
        for name in std::iter::once("lazybuddy.exe")
            .chain(
                info.files
                    .keys()
                    .map(String::as_str)
                    .filter(|name| *name != "lazybuddy.exe"),
            )
            .chain(std::iter::once(INFO))
        {
            let target = destination(&self.install, name)?;
            let existed = target.exists();
            if existed {
                let backup = destination(&self.backup, name)?;
                fs::create_dir_all(backup.parent().unwrap())?;
                atomic_copy(&target, &backup)?;
            }
            self.changed.push((name.into(), existed));
            let recorded = atomic_write(&self.backup.join(JOURNAL), |file| {
                serde_json::to_writer(file, &self.changed).map_err(io::Error::other)
            });
            if let Err(error) = recorded {
                self.changed.pop();
                return Err(error);
            }
            atomic_copy(&destination(stage, name)?, &target)?;
        }
        Ok(())
    }
    fn rollback(&self) -> io::Result<()> {
        let mut failed = false;
        for (name, existed) in self.changed.iter().rev() {
            failed |= (|| -> io::Result<()> {
                let target = destination(&self.install, name)?;
                let temporary = copy_temporary(&target);
                if temporary.exists() {
                    plain(&temporary)?;
                    fs::remove_file(temporary)?;
                }
                if *existed {
                    let backup = destination(&self.backup, name)?;
                    if file_digest(&target)
                        .ok()
                        .zip(file_digest(&backup).ok())
                        .is_some_and(|(a, b)| a == b)
                    {
                        return Ok(());
                    }
                    atomic_copy(&backup, &target)
                } else if target.exists() {
                    fs::remove_file(target)
                } else {
                    Ok(())
                }
            })()
            .is_err();
        }
        if failed {
            return Err(invalid("rollback failed; update backup retained"));
        }
        self.finish()
    }

    fn finish(&self) -> io::Result<()> {
        match fs::remove_file(self.backup.join(JOURNAL)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn copy_temporary(target: &Path) -> PathBuf {
    target.with_file_name(format!(
        ".{}.lazybuddy-new",
        target.file_name().unwrap().to_string_lossy()
    ))
}

fn atomic_write(
    target: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(target.parent().unwrap())?;
    let temporary = copy_temporary(target);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        write(&mut file)?;
        file.flush()?;
        file.sync_all()
    })();
    drop(file);
    let result = result.and_then(|()| windows::replace(&temporary, target));
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn atomic_copy(source: &Path, target: &Path) -> io::Result<()> {
    atomic_write(target, |file| {
        io::copy(&mut fs::File::open(source)?, file)?;
        Ok(())
    })
}

/// Return true if startup must yield to an updater holding the install lock.
pub fn recover_on_start(install: &Path) -> io::Result<bool> {
    let backup = install.join(RECOVERY);
    if !backup.join(JOURNAL).exists() {
        return Ok(false);
    }
    let _lock = match windows::lock(&install.join(INSTALL_LOCK)) {
        Ok(lock) => lock,
        Err(error) if error.raw_os_error() == Some(32) => {
            return Ok(std::env::var_os(UPDATE_TRIAL).is_none());
        }
        Err(error) => return Err(error),
    };
    plain(&backup)?;
    let helper = backup.join("recover.exe");
    verify_recovery_helper(install, &backup, &helper)?;
    let ready = backup.join("recovery-ready");
    if ready.exists() {
        fs::remove_file(&ready)?;
    }
    // The helper owns the lock from the handshake through the restoration.
    drop(_lock);
    let child = windows::command(&helper)
        .arg("--recover")
        .arg(std::process::id().to_string())
        .spawn()?;
    wait_ready(child, &ready)?;
    Ok(true)
}

fn verify_recovery_helper(install: &Path, backup: &Path, helper: &Path) -> io::Result<()> {
    let source = Source::load(&install.join(SOURCE))?;
    let manifest = verify_manifest(
        &fs::read(backup.join("recovery-manifest.json"))?,
        &fs::read(backup.join("recovery-manifest.sig"))?,
        &source.public_key,
    )?;
    let info = read_build_info(&backup.join("recovery-build.json"))?;
    match_manifest(&info, &manifest)?;
    plain(helper)?;
    if file_digest(helper)? != info.files["lazybuddy-updater.exe"] {
        return Err(invalid("recovery helper digest mismatch"));
    }
    Ok(())
}

fn discard_unstarted_transaction(install: &Path, backup: &Path) -> io::Result<()> {
    plain(backup)?;
    let journal = backup.join(JOURNAL);
    plain(&journal)?;
    let changed: Vec<(String, bool)> = read_json(&journal)?;
    // Legacy hosts cannot run the startup recovery hook before their EXE is replaced.
    if changed.as_slice() != [("lazybuddy.exe".into(), true)]
        || file_digest(&destination(install, "lazybuddy.exe")?)?
            != file_digest(&destination(backup, "lazybuddy.exe")?)?
    {
        return Err(invalid("update recovery is pending"));
    }
    recover_transaction(install, backup)
}

fn recover_transaction(install: &Path, backup: &Path) -> io::Result<()> {
    plain(backup)?;
    let journal = backup.join(JOURNAL);
    plain(&journal)?;
    let changed: Vec<(String, bool)> = read_json(&journal)?;
    let mut names = std::collections::BTreeSet::new();
    if changed.is_empty()
        || changed.len() > 10_001
        || changed
            .iter()
            .any(|(name, _)| !owned(name) || !names.insert(name))
    {
        return Err(invalid("invalid update recovery journal"));
    }
    Transaction {
        install: install.to_owned(),
        backup: backup.to_owned(),
        changed,
    }
    .rollback()
}

pub fn recover_job(parent: u32) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let backup = executable
        .parent()
        .ok_or_else(|| invalid("missing recovery directory"))?;
    if executable.file_name().and_then(|name| name.to_str()) != Some("recover.exe")
        || backup.file_name().and_then(|name| name.to_str()) != Some(RECOVERY)
    {
        return Err(invalid("invalid recovery executable location"));
    }
    let install = backup
        .parent()
        .ok_or_else(|| invalid("missing installation"))?;
    let _lock = windows::lock(&install.join(INSTALL_LOCK))?;
    verify_recovery_helper(install, backup, &executable)?;
    windows::wait_parent(
        parent,
        &install.join("lazybuddy.exe"),
        &backup.join("recovery-ready"),
    )?;
    recover_transaction(install, backup)?;
    launch_and_confirm(install, false)
}
fn launch_and_confirm(install: &Path, trial: bool) -> io::Result<()> {
    let mut command = windows::command(&install.join("lazybuddy.exe"));
    command
        .arg("--background")
        .current_dir(install)
        .env_remove(UPDATE_TRIAL);
    if trial {
        command.env(UPDATE_TRIAL, "1");
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Err(invalid("updated application exited before readiness"));
        }
        let request = lazybuddy_host_control::Request {
            version: 1,
            command: "status".into(),
            args: serde_json::json!({}),
        };
        if lazybuddy_host_control::call_sync(&request).is_ok_and(|reply| {
            reply["ok"] == true
                && reply["data"]["pid"].as_u64() == Some(u64::from(child.id()))
                && reply["data"]["window"].is_array()
        }) {
            let _ = lazybuddy_host_control::call_sync(&lazybuddy_host_control::Request {
                version: 1,
                command: "window.show".into(),
                args: serde_json::json!({}),
            });
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(invalid("updated application did not become ready"))
}
pub fn apply_job(path: &Path) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| invalid("missing update directory"))?
        .canonicalize()?;
    let _lock = windows::lock(&directory.join("apply.lock"))?;
    let job: Job = read_json(path)?;
    let install = job.install.canonicalize()?;
    let source = Source::load(&install.join(SOURCE))?;
    let manifest = verify_manifest(
        &fs::read(directory.join("manifest.json"))?,
        &fs::read(directory.join("manifest.json.sig"))?,
        &source.public_key,
    )?;
    verify_stage(&directory.join("stage"), &manifest)?;
    let _install_lock = windows::lock(&install.join(INSTALL_LOCK))?;
    if uninstall::removal_pending(&install)? {
        return Err(invalid("uninstallation is pending"));
    }
    let recovery = install.join(RECOVERY);
    if recovery.join(JOURNAL).exists() {
        discard_unstarted_transaction(&install, &recovery)?;
    }
    if !update_allowed(&install, &manifest)? {
        return Err(invalid("update is older than the installed release"));
    }
    windows::wait_parent(
        job.parent_pid,
        &install.join("lazybuddy.exe"),
        &directory.join("ready"),
    )?;
    let initialized = verify_stage(&directory.join("stage"), &manifest).and_then(|info| {
        let transaction = Transaction::new(&install, &install.join(RECOVERY))?;
        for (source, target) in [
            ("stage/lazybuddy-updater.exe", "recover.exe"),
            ("manifest.json", "recovery-manifest.json"),
            ("manifest.json.sig", "recovery-manifest.sig"),
            ("stage/build-info.json", "recovery-build.json"),
        ] {
            atomic_copy(&directory.join(source), &transaction.backup.join(target))?;
        }
        Ok((info, transaction))
    });
    let (info, mut transaction) = match initialized {
        Ok(value) => value,
        Err(error) => {
            let _ = launch_and_confirm(&install, false);
            return Err(error);
        }
    };
    let result = transaction
        .replace(&directory.join("stage"), &info)
        .and_then(|_| launch_and_confirm(&install, true));
    if result.is_err() {
        transaction.rollback()?;
        let _ = launch_and_confirm(&install, false);
    } else {
        transaction.finish()?;
    }
    let status = serde_json::json!({"ok":result.is_ok(),"build":manifest.build});
    fs::write(
        directory.parent().unwrap().join("result.json"),
        serde_json::to_vec(&status).map_err(io::Error::other)?,
    )?;
    let _ = clear_directory(&transaction.backup);
    if result.is_ok() {
        for name in ["stage", "backup"] {
            let _ = clear_directory(&directory.join(name));
        }
        for name in [
            "package.zip",
            "manifest.json",
            "manifest.json.sig",
            "job.json",
            "ready",
        ] {
            let _ = fs::remove_file(directory.join(name));
        }
    }
    result
}
