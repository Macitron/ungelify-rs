use clap::Parser;
use cli::Cli;

mod cli;

fn main() {
    let args = Cli::parse();

    if let Err(e) = cli::run(args) {
        eprintln!("an error occurred: {}", e);
    }
}
