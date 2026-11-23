use lazybuddy_module_manager::Manager;
use std::{env, io, path::PathBuf, process::ExitCode};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let root = lazybuddy_host_control::data_dir()?;
    let manager = Manager::new(root.join("modules"));
    let command = args
        .next()
        .ok_or("expected modules list|install-local <directory>|uninstall <id>")?;
    if command == "host" {
        let request = args.next().ok_or("host request JSON required")?;
        if args.next().is_some() {
            return Err("unexpected argument".into());
        }
        let request = serde_json::from_str(&request)?;
        serde_json::to_writer(io::stdout(), &lazybuddy_host_control::call_sync(&request)?)?;
        return Ok(());
    }
    if command == "call" {
        let id = args.next().ok_or("module ID required")?;
        let request = args.next().ok_or("request JSON required")?;
        if args.next().is_some() {
            return Err("unexpected argument".into());
        }
        let request = serde_json::from_str(&request)?;
        let mut launch = manager.launch(&id)?;
        if let Some(scope) = lazybuddy_host_control::isolated_credential_scope()? {
            let data = root
                .join("services")
                .join(&id)
                .to_string_lossy()
                .into_owned();
            launch.environment.extend([
                ("LAZYBUDDY_CREDENTIAL_SCOPE".into(), scope),
                ("LAZYBUDDY_DATA_DIR".into(), data.clone()),
                ("LAZYBUDDY_NEWS_DATA_DIR".into(), data),
            ]);
        }
        let mut client = lazybuddy_service_client::Client::start_with_environment(
            &launch.executable,
            &launch.arguments,
            &launch.environment,
        )?;
        serde_json::to_writer(io::stdout(), &client.request(&request)?)?;
        return Ok(());
    }
    if command != "modules" {
        return Err("unknown command".into());
    }
    let operation = args.next();
    if matches!(
        operation.as_deref(),
        Some("install-local" | "install" | "uninstall")
    ) {
        let status = lazybuddy_host_control::call_sync(&lazybuddy_host_control::Request {
            version: 1,
            command: "status".into(),
            args: serde_json::json!({}),
        });
        match status {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err("LazyBuddy is running or its state cannot be confirmed; quit it before changing packages with the CLI".into()),
        }
    }
    match operation.as_deref() {
        Some("list") => {
            if args.next().is_some() {
                return Err("unexpected argument".into());
            }
            serde_json::to_writer(io::stdout(), &manager.list()?)?;
        }
        Some("install-local") => {
            let source = args.next().ok_or("package directory required")?;
            if args.next().is_some() {
                return Err("unexpected argument".into());
            }
            serde_json::to_writer(
                io::stdout(),
                &manager.install_local(&PathBuf::from(source))?,
            )?;
        }
        Some("install") => {
            let id = args.next().ok_or("module ID required")?;
            let catalog_url = args.next().ok_or("signed catalog HTTPS URL required")?;
            let key_path = args
                .next()
                .ok_or("trusted raw Ed25519 public key file required")?;
            if args.next().is_some() {
                return Err("unexpected argument".into());
            }
            let key = std::fs::read(key_path)?;
            let catalog = lazybuddy_module_manager::package::download_catalog(&catalog_url, &key)?;
            serde_json::to_writer(io::stdout(), &manager.install_from_catalog(&catalog, &id)?)?;
        }
        Some("uninstall") => {
            let id = args.next().ok_or("module ID required")?;
            if args.next().is_some() {
                return Err("unexpected argument".into());
            }
            manager.uninstall(&id)?;
            println!("{{\"ok\":true}}");
        }
        _ => return Err("unknown modules command".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
