use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;
use ungelify::vfs::{Archive, ArchiveImpl};

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
    #[command(
        about = "List out contents of an archive",
        arg_required_else_help = true,
        aliases = ["l", "ls", "list-contents"]
    )]
    List {
        #[arg(value_name = "ARCHIVE", help = "The archive to list")]
        archive_path: PathBuf,
    },
    #[command(
        about = "Extract a file from an archive",
        arg_required_else_help = true,
        aliases = ["x", "ex"]
    )]
    Extract {
        #[arg(value_name = "ARCHIVE", help = "The archive to extract")]
        archive_path: PathBuf,
        #[arg(
            help = "The names or IDs of the files to extract.\nIf omitted, the whole archive is extracted"
        )]
        entries: Option<Vec<String>>,
        #[arg(long, short, help = "The directory to extract the entries to")]
        output_dir: Option<PathBuf>,
    },
    #[command(
        about = "Repack an archive with a modified file",
        arg_required_else_help = true,
        aliases = ["r", "re"]
    )]
    Replace {
        #[arg(value_name = "ARCHIVE", help = "The archive to repack")]
        archive_path: PathBuf,
        #[arg(help = "The names of the files to replace")]
        replacement_files: Vec<PathBuf>,
    },
}

pub fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::List { archive_path } => {
            let archive = ArchiveImpl::open(&archive_path)?;
            // archive.list_entries();
        }
        Commands::Extract {
            archive_path,
            entries,
            output_dir,
        } => {
            let archive = ArchiveImpl::open(&archive_path)?;
            // archive.extract_entries(entries, output_dir)?;
        }
        Commands::Replace {
            archive_path,
            replacement_files,
        } => {
            let archive = ArchiveImpl::open(&archive_path)?;
            // archive.replace_entries(&replacement_files)?;
        }
    }

    Ok(())
}
