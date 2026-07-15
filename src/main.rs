use agent_sidecar::inits;
use agent_sidecar::ai::service::{prompt};

fn main() -> std::process::ExitCode {
    match inits::init_app() {
        Err(error) => {
            eprintln!("failed to load configuration: {error}");
            return std::process::ExitCode::FAILURE;
        }
        Ok(_) => {}
    }


    let response = prompt("Hello".to_string()).expect("TODO: panic message");
    tracing::info!(response = response.content, "Response received");


    std::process::ExitCode::SUCCESS
}
