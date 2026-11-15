#![windows_subsystem = "windows"]
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let executable = std::env::current_exe().unwrap();
    if args.is_empty()
        && executable
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq_ignore_ascii_case("uninstall.exe")
    {
        return match std::process::Command::new(executable.with_file_name("lazybuddy-updater.exe"))
            .arg("--uninstall")
            .spawn()
        {
            Ok(_) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                lazybuddy_app_updater::uninstall::show_error(&error);
                std::process::ExitCode::FAILURE
            }
        };
    }
    if args.first().is_some_and(|arg| arg == "--uninstall") {
        if args
            .iter()
            .skip(1)
            .any(|arg| arg != "--yes" && arg != "--remove-modules")
        {
            return std::process::ExitCode::FAILURE;
        }
        return match lazybuddy_app_updater::uninstall::start(
            args.iter().any(|arg| arg == "--yes"),
            args.iter().any(|arg| arg == "--remove-modules"),
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                if !args.iter().any(|arg| arg == "--yes") {
                    lazybuddy_app_updater::uninstall::show_error(&error);
                }
                std::process::ExitCode::FAILURE
            }
        };
    }
    if args.len() == 2 && args[0] == "--uninstall-apply" {
        return if lazybuddy_app_updater::uninstall::apply(std::path::Path::new(&args[1])).is_ok() {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }
    if args.len() != 2 || args[0] != "--apply" {
        return std::process::ExitCode::FAILURE;
    }
    match lazybuddy_app_updater::apply_job(std::path::Path::new(&args[1])) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}
