use clap::Parser;
use ungelify::cli;
use ungelify::cli::Cli;

fn main() {
    let args = Cli::parse();

    if let Err(e) = cli::run(args) {
        eprintln!("an error occurred: {}", e);
    }
}
