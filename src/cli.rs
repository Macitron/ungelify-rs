use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;
use ungelify::vfs::mpk::MagesArchive;
use ungelify::vfs::Archive;

#[derive(Debug, Parser)]
#[command(
    name = "ungelify",
    version,
    about = "A CLI tool for extracting and repacking Mages/Criware archives"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "List out contents of an archive")]
    #[command(arg_required_else_help = true, aliases = ["l", "ls", "list-contents"])]
    List {
        #[arg(help = "The archive to list")]
        archive: PathBuf,
    },
    #[command(about = "Extract a file from an archive")]
    #[command(arg_required_else_help = true, aliases = ["x", "ex"])]
    Extract {
        #[arg(help = "The archive to extract")]
        archive: PathBuf,
        #[arg(
            help = "The name or ID of the file to extract\nIf empty, the whole archive is extracted"
        )]
        entry: Option<String>,
    },
    #[command(about = "Repack an archive with a modified file")]
    #[command(arg_required_else_help = true, aliases = ["r", "re"])]
    Replace {
        #[arg(help = "The archive to repack")]
        archive: PathBuf,
        #[arg(help = "The name of the file to replace")]
        entry_file: PathBuf,
    },
}

pub fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::List { archive } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            archive.list_entries();
        }
        Commands::Extract { archive, entry } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            match entry {
                Some(entry) => {
                    archive.extract_entry(&entry)?;
                }
                None => {
                    archive.extract_all_entries()?;
                }
            }
        }
        Commands::Replace {
            archive,
            entry_file,
        } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            archive.replace_entry(entry_file)?;
        }
    }

    Ok(())
}
