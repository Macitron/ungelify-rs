use clap::Parser;
use cli::Cli;
use std::process;
use ungelify::vfs::mpk::MagesArchive;

mod cli;

fn main() {
    let mpk = MagesArchive::build("resources/script.mpk").unwrap();
    println!("archive: {mpk:#?}");

    // let args = Cli::parse();
    //
    // if let Err(e) = cli::run(args) {
    //     eprintln!("ungelify: {e}");
    //     process::exit(1);
    // }
}
