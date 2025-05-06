use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use ungelify::mpk::MagesArchive;

#[derive(Debug, Parser)]
#[command(
    name = "ungelify",
    version,
    about = "A CLI tool for extracting and repacking Mages/Criware archives"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(
        about = "List out contents of an archive",
        arg_required_else_help = true,
        aliases = ["l", "ls", "list-contents"])]
    List {
        #[arg(value_name = "ARCHIVE", help = "The archive to list")]
        archive_path: PathBuf,
    },
}

pub fn run(cli: Cli) {
    match cli.command {
        Cmd::List { archive_path } => {
            let mut reader = BufReader::new(File::open(&archive_path).unwrap());
            let mpk = MagesArchive::build(&mut reader);
            mpk.list_entries();
        }
    }
}
