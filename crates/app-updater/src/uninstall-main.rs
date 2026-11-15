#![windows_subsystem = "windows"]
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--uninstall-apply" {
        return if lazybuddy_app_updater::uninstall::apply(std::path::Path::new(&args[1])).is_ok() {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }
    if args
        .iter()
        .any(|arg| arg != "--yes" && arg != "--remove-modules")
    {
        return std::process::ExitCode::FAILURE;
    }
    let quiet = args.iter().any(|arg| arg == "--yes");
    match lazybuddy_app_updater::uninstall::start(
        quiet,
        args.iter().any(|arg| arg == "--remove-modules"),
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if !quiet {
                lazybuddy_app_updater::uninstall::show_error(&error);
            }
            std::process::ExitCode::FAILURE
        }
    }
}
