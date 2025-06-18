use crate::cli::Cli;
use clap::Parser;

mod cli;

fn main() {
    let args = Cli::parse();
    cli::run(args);
}
