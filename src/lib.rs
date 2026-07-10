pub mod analysis;
pub mod cli;
pub mod dict;
pub mod document;
pub mod error;
pub mod export;
pub mod profile;
pub mod text;

use std::fs;
use std::io;

pub use analysis::{analyze, AnalysisConfig, AnalysisReport, SortMode, WordSourceStat, WordStat};
pub use cli::{parse_args, parse_cli_from, CliCommand};
pub use error::{RebeError, RebeResult};
pub use export::OutputFormat;

pub fn run<I>(args: I) -> RebeResult<()>
where
    I: IntoIterator<Item = String>,
{
    match cli::parse_cli_from(args) {
        Ok(command) => run_command(command),
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            Ok(())
        }
        Err(error) => Err(RebeError::InvalidArgument(error.to_string())),
    }
}

pub fn run_command(command: CliCommand) -> RebeResult<()> {
    match command {
        CliCommand::Analyze(config) => {
            let output_path = config.output.clone();
            let format = config.format;
            let report = analysis::analyze(&config)?;
            let rendered = export::render(&report, format);

            if let Some(path) = output_path {
                fs::write(path, rendered)?;
            } else {
                print!("{rendered}");
            }

            Ok(())
        }
        CliCommand::ProfileInit { path, force } => {
            profile::init_user_profile(&path, force)?;
            println!("Wrote profile: {}", path.display());
            Ok(())
        }
        CliCommand::ProfileAddKnown { path, words } => {
            let added_count = profile::append_known_words(&path, &words)?;

            if added_count == 0 {
                println!("No new known words: {}", path.display());
            } else {
                println!(
                    "Added {added_count} known word(s) to profile: {}",
                    path.display()
                );
            }

            Ok(())
        }
        CliCommand::ProfileAddIgnore { path, words } => {
            let added_count = profile::append_ignored_words(&path, &words)?;

            if added_count == 0 {
                println!("No new ignored words: {}", path.display());
            } else {
                println!(
                    "Added {added_count} ignored word(s) to profile: {}",
                    path.display()
                );
            }

            Ok(())
        }
        CliCommand::Completions { shell } => {
            clap_complete::generate(shell, &mut cli::command(), "rebe", &mut io::stdout());
            Ok(())
        }
    }
}
