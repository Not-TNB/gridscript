use clap::Parser as ClapParser;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(ClapParser)]
struct Cli {
    script: PathBuf,
    #[arg(long)]
    seed: Option<u64>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let source = match std::fs::read_to_string(&cli.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", cli.script.display());
            return ExitCode::FAILURE;
        }
    };

    match gridscript::parser::parse(&source) {
        Ok(program) => {
            println!("{program:#?}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("parse error: {e}");
            ExitCode::FAILURE
        }
    }
}
