use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match httpyac::cli::run().await {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(err) => {
            eprintln!("httpyac-rs: {err:#}");
            ExitCode::from(1)
        }
    }
}
