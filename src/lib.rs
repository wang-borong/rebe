pub mod analysis;
pub mod cli;
pub mod dict;
pub mod document;
pub mod error;
pub mod export;
pub mod profile;
pub mod text;

use std::fs;

pub use analysis::{analyze, AnalysisConfig, AnalysisReport, SortMode, WordSourceStat, WordStat};
pub use cli::{parse_args, CliCommand};
pub use error::{RebeError, RebeResult};
pub use export::OutputFormat;

pub fn run<I>(args: I) -> RebeResult<()>
where
    I: IntoIterator<Item = String>,
{
    match cli::parse_args(args)? {
        CliCommand::Help => {
            println!("{}", cli::help_text());
            Ok(())
        }
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
    }
}
