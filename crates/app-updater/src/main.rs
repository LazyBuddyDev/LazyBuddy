#![windows_subsystem = "windows"]
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [action, job] if action == "--apply" => {
            lazybuddy_app_updater::apply_job(std::path::Path::new(job))
        }
        [action, parent] if action == "--recover" => parent
            .to_str()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| std::io::Error::other("invalid recovery parent"))
            .and_then(lazybuddy_app_updater::recover_job),
        _ => return std::process::ExitCode::FAILURE,
    };
    if result.is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}
