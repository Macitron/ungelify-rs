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
            help = "The names or IDs of the files to extract.\nIf omitted, the whole archive is extracted"
        )]
        entries: Option<Vec<String>>,
    },
    #[command(about = "Repack an archive with a modified file")]
    #[command(arg_required_else_help = true, aliases = ["r", "re"])]
    Replace {
        #[arg(help = "The archive to repack")]
        archive: PathBuf,
        #[arg(help = "The names of the files to replace")]
        replacement_files: Vec<PathBuf>,
    },
}

pub fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::List { archive } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            archive.list_entries();
        }
        Commands::Extract { archive, entries } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            match entries {
                Some(list) => {
                    archive.extract_entries(&list)?;
                }
                None => {
                    archive.extract_all_entries()?;
                }
            }
        }
        Commands::Replace {
            archive,
            replacement_files,
        } => {
            let archive: MagesArchive = Archive::from_file(archive)?;
            archive.replace_entries(&replacement_files)?;
        }
    }

    Ok(())
}
