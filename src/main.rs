use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match ringmaster::run_from(std::env::args_os()).await {
        Ok(Some(output)) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
