use agent_sidecar::inits;

fn main() -> std::process::ExitCode {
    match inits::init_app() {
        Err(error) => {
            eprintln!("failed to load configuration: {error}");
            return std::process::ExitCode::FAILURE;
        }
        Ok(_) => {}
    }

    let config = agent_sidecar::config::app::get();

    tracing::info!(log_level = config.log_level.as_str(), "application started");

    std::process::ExitCode::SUCCESS
}
