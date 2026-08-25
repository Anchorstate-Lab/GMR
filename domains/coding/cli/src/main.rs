use std::process::ExitCode;

use clap::Parser;
use coding_anchor::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("gmr: cannot start the runtime: {e}");
            return ExitCode::from(2);
        }
    };
    let outcome = runtime.block_on(coding_anchor::run(cli));
    runtime.shutdown_background();
    match outcome {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("gmr: {e}");
            ExitCode::from(2)
        }
    }
}
