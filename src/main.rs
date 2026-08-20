use clap::Parser as ClapParser;
use gridscript::interpreter;
use gridscript::interpreter::context::RunContext;
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
            let mut ctx = RunContext::std(&program, cli.seed);
            let result = interpreter::run(&program, &mut ctx);

            for w in &ctx.warnings {
                eprintln!("warning: {w}");
            }

            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("parse error: {e}");
            ExitCode::FAILURE
        }
    }
}
