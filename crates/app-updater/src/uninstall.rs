//! Inventory-based uninstall. Shared settings and credentials are never removed.
use super::*;
use lazybuddy_host_control::{Request, call_sync};
use std::time::SystemTime;
use windows_sys::{Win32::UI::WindowsAndMessaging::*, core::w};

const PENDING: &str = ".lazybuddy-uninstall.json";

pub(super) fn removal_pending(install: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(install.join(PENDING)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn inventory(install: &Path) -> io::Result<BuildInfo> {
    plain(install)?;
    if !removal_pending(install)? {
        return build_info(install);
    }
    let journal = install.join(PENDING);
    plain(&journal)?;
    let info = read_build_info(&journal)?;
    for (name, expected) in &info.files {
        let file = destination(install, name)?;
        match file_digest(&file) {
            Ok(actual) if actual == *expected => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(invalid("remaining installation file changed")),
        }
    }
    let marker = destination(install, INFO)?;
    match fs::symlink_metadata(&marker) {
        Ok(_) if read_build_info(&marker)? != info => {
            return Err(invalid("installation inventory changed"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(info)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn remove_files(install: &Path, before_final: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    let info = inventory(install)?;
    let journal = install.join(PENDING);
    if !removal_pending(install)? {
        atomic_write(&journal, |file| {
            serde_json::to_writer(file, &info).map_err(io::Error::other)
        })?;
    }
    let mut directories = std::collections::BTreeSet::new();
    for name in info
        .files
        .keys()
        .filter(|name| name.as_str() != "uninstall.exe")
    {
        let file = destination(install, name)?;
        for parent in file.ancestors().skip(1).take_while(|path| *path != install) {
            directories.insert(parent.to_owned());
        }
        remove_if_present(&file)?;
    }
    for directory in directories.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error),
        }
    }
    remove_if_present(&destination(install, INFO)?)?;
    before_final()?;
    remove_if_present(&destination(install, "uninstall.exe")?)?;
    remove_if_present(&journal)
}

fn stop_host(install: &Path, stage: &Path) -> io::Result<()> {
    let request = |command: &str| Request {
        version: 1,
        command: command.into(),
        args: serde_json::json!({}),
    };
    let status = match call_sync(&request("status")) {
        Ok(status) => status,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let pid = status["data"]["pid"]
        .as_u64()
        .and_then(|pid| u32::try_from(pid).ok())
        .ok_or_else(|| invalid("invalid host status"))?;
    let expected = install.join("lazybuddy.exe");
    let ready = stage.join("host-ready");
    let check = ready.clone();
    let wait = std::thread::spawn(move || windows::wait_parent(pid, &expected, &ready));
    while !check.exists() && !wait.is_finished() {
        std::thread::sleep(Duration::from_millis(20));
    }
    if check.exists() {
        let reply = call_sync(&request("app.quit"))?;
        if reply["ok"] != true {
            return Err(invalid("application refused to quit"));
        }
    }
    wait.join().map_err(|_| invalid("host exit wait failed"))?
}

fn remove_modules(install: &Path) -> io::Result<()> {
    let call = |args: &[&str]| -> io::Result<Vec<lazybuddy_module_contract::Manifest>> {
        let reply = windows::command(&install.join("lazybuddy-cli.exe"))
            .args(args)
            .output()?;
        if !reply.status.success() {
            return Err(invalid(
                "module removal failed; application files preserved",
            ));
        }
        if args == ["modules", "list"] {
            serde_json::from_slice(&reply.stdout).map_err(io::Error::other)
        } else {
            Ok(vec![])
        }
    };
    for package in call(&["modules", "list"])? {
        if package.kind == lazybuddy_module_contract::Kind::Feature {
            call(&["modules", "uninstall", &package.id])?;
        }
    }
    for package in call(&["modules", "list"])? {
        call(&["modules", "uninstall", &package.id])?;
    }
    Ok(())
}

fn startup_matches(value: &str, executable: &Path) -> bool {
    value
        .trim_end_matches('\0')
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix("\" --background"))
        .and_then(|path| Path::new(path).canonicalize().ok())
        == executable.canonicalize().ok()
        && executable.exists()
}
fn clear_startup(install: &Path) -> io::Result<()> {
    use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};
    if lazybuddy_host_control::isolated() {
        return Ok(());
    }
    let mut value = vec![0u16; 32768];
    let mut bytes = (value.len() * 2) as u32;
    let run = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            run,
            w!("LazyBuddy"),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            value.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if result == ERROR_SUCCESS
        && startup_matches(
            &String::from_utf16_lossy(&value[..bytes as usize / 2]),
            &install.join("lazybuddy.exe"),
        )
    {
        let result = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, run, w!("LazyBuddy")) };
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
    }
    Ok(())
}

fn registered_install_matches(value: &str, install: &Path) -> bool {
    Path::new(value.trim_end_matches('\0')).canonicalize().ok() == install.canonicalize().ok()
        && install.is_dir()
}

fn clear_install_registration(install: &Path) -> io::Result<()> {
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::*,
    };
    let key: Vec<u16> = if lazybuddy_host_control::isolated() {
        format!(
            r"Software\LazyBuddy\Verification\{}",
            lazybuddy_host_control::instance()?
        )
    } else {
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\LazyBuddy".into()
    }
    .encode_utf16()
    .chain([0])
    .collect();
    let mut value = vec![0u16; 32768];
    let mut bytes = (value.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            w!("InstallLocation"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            value.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    if registered_install_matches(
        &String::from_utf16_lossy(&value[..bytes as usize / 2]),
        install,
    ) {
        // Non-recursive: an unexpected subkey must not be deleted.
        let result =
            unsafe { RegDeleteKeyExW(HKEY_CURRENT_USER, key.as_ptr(), KEY_WOW64_64KEY, 0) };
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
    }
    Ok(())
}

fn dialog(text: &str, flags: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let text: Vec<_> = text.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            w!("LazyBuddy 卸载"),
            flags,
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Removal {
    install: PathBuf,
    parent: u32,
    modules: bool,
    quiet: bool,
}

pub fn start(quiet: bool, mut modules: bool) -> io::Result<()> {
    let install = std::env::current_exe()?.parent().unwrap().canonicalize()?;
    let resuming = removal_pending(&install)?;
    inventory(&install)?;
    if !quiet {
        if dialog(
            "卸载 LazyBuddy？\n正在运行的本安装将正常退出。设置和账号凭据会保留。",
            MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION,
        ) != IDYES
        {
            return Ok(());
        }
        if !resuming {
            match dialog(
                "是否同时移除已下载的功能模块和运行时？\n这些资源由当前用户的所有 LazyBuddy 安装共享。\n选择“否”仅卸载主程序，仍保留账号和设置。",
                MB_YESNOCANCEL | MB_DEFBUTTON2 | MB_ICONQUESTION,
            ) {
                IDYES => modules = true,
                IDNO => modules = false,
                _ => return Ok(()),
            }
        }
    }
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let stage = std::env::temp_dir().join(format!(
        "lazybuddy-uninstall-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&stage)?;
    fs::copy(install.join("uninstall.exe"), stage.join("remove.exe"))?;
    fs::write(
        stage.join("job.json"),
        serde_json::to_vec(&Removal {
            install,
            parent: std::process::id(),
            modules,
            quiet,
        })
        .map_err(io::Error::other)?,
    )?;
    let mut child = windows::command(&stage.join("remove.exe"))
        .current_dir(&stage)
        .arg("--uninstall-apply")
        .arg(stage.join("job.json"))
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !stage.join("ready").exists() {
        if child.try_wait()?.is_some() {
            return Err(invalid("uninstall helper could not start"));
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid("uninstall helper startup timed out"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

pub fn apply(job: &Path) -> io::Result<()> {
    let stage = std::env::current_exe()?.parent().unwrap().canonicalize()?;
    if job.canonicalize()? != stage.join("job.json")
        || stage.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
        || !stage
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("lazybuddy-uninstall-")
    {
        return Err(invalid("invalid uninstall job"));
    }
    plain(&stage)?;
    let removal: Removal = read_json(job)?;
    let result = (|| {
        plain(&removal.install)?;
        let lock_path = removal.install.join(INSTALL_LOCK);
        let lock = windows::lock(&lock_path)?;
        if removal.install.join(RECOVERY).join(JOURNAL).exists() {
            return Err(invalid("finish update recovery before uninstalling"));
        }
        inventory(&removal.install)?;
        let resuming = removal_pending(&removal.install)?;
        windows::wait_parent(
            removal.parent,
            &removal.install.join("uninstall.exe"),
            &stage.join("ready"),
        )?;
        stop_host(&removal.install, &stage)?;
        if !resuming {
            if removal.modules {
                remove_modules(&removal.install)?;
            }
            clear_startup(&removal.install)?;
        }
        remove_files(&removal.install, || {
            clear_install_registration(&removal.install)
        })?;
        drop(lock);
        let _ = remove_if_present(&lock_path);
        match fs::remove_dir(&removal.install) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
            Err(error) => Err(error),
        }
    })();
    if let Err(error) = &result {
        if !removal.quiet {
            dialog(
                &format!("未能完成卸载，请检查文件占用。\n{error}"),
                MB_OK | MB_ICONERROR,
            );
        }
        fs::write(stage.join("error.txt"), error.to_string())?;
        return result;
    }
    if !removal.quiet {
        dialog(
            "LazyBuddy 已卸载。设置、账号凭据以及未属于本程序的文件已保留。",
            MB_OK | MB_ICONINFORMATION,
        );
    }
    // A separate shell waits for this image to exit, then removes only our fixed staging files.
    fs::write(
        stage.join("cleanup.ps1"),
        r#"param([int]$Worker,[string]$Stage)
$ErrorActionPreference='Stop'
$base=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$path=[IO.Path]::GetFullPath($Stage)
if((Split-Path -Parent $path) -ne $base -or (Split-Path -Leaf $path) -notmatch '^lazybuddy-uninstall-\d+-\d+$'){exit 1}
if((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint){exit 1}
Wait-Process -Id $Worker -ErrorAction SilentlyContinue
foreach($name in @('remove.exe','job.json','ready','host-ready','cleanup.ps1')){Remove-Item -LiteralPath (Join-Path $path $name) -Force -ErrorAction SilentlyContinue}
Remove-Item -LiteralPath $path
"#,
    )?;
    let shell_stage = std::env::temp_dir().join(stage.file_name().unwrap());
    windows::command(Path::new("powershell.exe"))
        .current_dir(std::env::temp_dir())
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(shell_stage.join("cleanup.ps1"))
        .arg("-Worker")
        .arg(std::process::id().to_string())
        .arg("-Stage")
        .arg(shell_stage)
        .spawn()?;
    Ok(())
}

pub fn show_error(error: &io::Error) {
    dialog(&format!("卸载未完成：{error}"), MB_OK | MB_ICONERROR);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Temporary, fixture};
    #[test]
    fn uninstall_registration_must_belong_to_this_installation() {
        let temp = Temporary::new();
        let install = temp.0.join("registered");
        let other = temp.0.join("other");
        fs::create_dir(&install).unwrap();
        fs::create_dir(&other).unwrap();
        assert!(registered_install_matches(
            &install.to_string_lossy(),
            &install
        ));
        assert!(!registered_install_matches(
            &other.to_string_lossy(),
            &install
        ));
        assert!(!registered_install_matches("does-not-exist", &install));
    }
    #[test]
    fn removes_only_inventory_and_keeps_unknown_files() {
        let root = Temporary::new();
        let install = root.0.join("app");
        fixture(&install, "fixture");
        fs::write(install.join("personal.txt"), b"keep").unwrap();
        remove_files(&install, || Ok(())).unwrap();
        assert_eq!(fs::read(install.join("personal.txt")).unwrap(), b"keep");
        assert!(!install.join("lazybuddy.exe").exists());
        assert!(!install.join("uninstall.exe").exists());
    }
    #[test]
    fn changed_or_linked_files_preserve_whole_installation() {
        use std::os::windows::fs::symlink_file;
        let root = Temporary::new();
        let install = root.0.join("app");
        fixture(&install, "fixture");
        fs::write(install.join("lazybuddy-cli.exe"), b"changed").unwrap();
        assert!(remove_files(&install, || Ok(())).is_err());
        assert!(install.join("lazybuddy.exe").exists());
        fs::remove_file(install.join("lazybuddy-cli.exe")).unwrap();
        symlink_file(
            install.join("lazybuddy.exe"),
            install.join("lazybuddy-cli.exe"),
        )
        .unwrap();
        assert!(remove_files(&install, || Ok(())).is_err());
        assert!(install.join("lazybuddy.exe").exists());
        fs::remove_file(install.join("lazybuddy-cli.exe")).unwrap();
    }
    #[test]
    fn changed_uninstaller_preserves_entire_installation() {
        let root = Temporary::new();
        let install = root.0.join("app");
        fixture(&install, "fixture");
        fs::write(install.join("uninstall.exe"), b"changed").unwrap();
        assert!(remove_files(&install, || Ok(())).is_err());
        assert!(install.join("lazybuddy.exe").exists());
    }
    #[test]
    fn locked_file_can_be_retried_from_the_verified_inventory() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = Temporary::new();
        let install = root.0.join("app");
        fixture(&install, "fixture");
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(install.join("module-catalog.json"))
            .unwrap();
        assert!(remove_files(&install, || Ok(())).is_err());
        assert!(removal_pending(&install).unwrap());
        assert!(!install.join("lazybuddy.exe").exists());
        assert!(install.join("uninstall.exe").exists());
        drop(locked);
        fs::write(install.join("personal.txt"), b"keep").unwrap();
        remove_files(&install, || Ok(())).unwrap();
        assert!(!removal_pending(&install).unwrap());
        assert!(!install.join("uninstall.exe").exists());
        assert_eq!(fs::read(install.join("personal.txt")).unwrap(), b"keep");
    }
    #[test]
    fn startup_cleanup_matches_only_this_installation() {
        let root = Temporary::new();
        let install = root.0.join("app");
        fixture(&install, "fixture");
        assert!(startup_matches(
            &format!(
                "\"{}\" --background",
                install.join("lazybuddy.exe").display()
            ),
            &install.join("lazybuddy.exe")
        ));
        assert!(!startup_matches(
            "\"C:\\Other\\lazybuddy.exe\" --background",
            &install.join("lazybuddy.exe")
        ));
    }
}
