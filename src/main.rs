use clap::Parser;
use cli::Cli;
use std::process;

mod cli;

fn main() {
    let args = Cli::parse();

    if let Err(e) = cli::run(args) {
        eprintln!("{e}");
        process::exit(1);
    }
}
