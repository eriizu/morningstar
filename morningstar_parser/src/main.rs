use clap::Parser;
use morningstar_parser::*;

fn main() -> std::process::ExitCode {
    let opt = Opt::parse();
    let mut parser = MorningstarPasrer::new();

    match parser.run_with_opt(&opt) {
        Ok(tt) => {
            println!(
                "Parsed {} journeys, {} patterns, {} excpetions",
                tt.journeys.len(),
                tt.service_patterns.len(),
                tt.excpetions.len()
            );
            std::process::ExitCode::SUCCESS
        }
        Err(err) => {
            parser.spinner.fail(&err.to_string());
            std::process::ExitCode::SUCCESS
        }
    }
}
