use std::process::ExitCode;

fn main() -> ExitCode {
    match ringmaster::run_from(std::env::args()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
