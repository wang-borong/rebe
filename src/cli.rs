use std::ffi::OsString;
use std::path::PathBuf;

use clap::error::{Error as ClapError, ErrorKind};
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use crate::analysis::{AnalysisConfig, SortMode};
use crate::dict::MdxDefinitionFormat;
use crate::error::{RebeError, RebeResult};
use crate::export::OutputFormat;
use crate::profile;

#[derive(Debug, Parser)]
#[command(
    name = "rebe",
    version,
    about = "Prepare English reading with focused vocabulary reports",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Analyze a book, article, or directory.
    Analyze(Box<AnalyzeArgs>),
    /// Create and maintain reader profiles.
    Profile(ProfileArgs),
    /// Generate shell completion scripts.
    #[command(visible_alias = "completion")]
    Completions(CompletionArgs),
}

#[derive(Debug, Args)]
struct AnalyzeArgs {
    /// Input file or directory to analyze.
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Write the report to a file instead of stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Output format: txt, csv, or json.
    #[arg(long, value_name = "FORMAT", value_parser = parse_output_format_arg)]
    format: Option<OutputFormat>,

    /// User profile with known, ignored, lemma, and default settings.
    #[arg(long, value_name = "PATH")]
    profile: Option<PathBuf>,

    /// Lemma overrides applied before built-in WordNet lemmatization.
    #[arg(long, value_name = "PATH")]
    lemma_map: Option<PathBuf>,

    /// Known words file; matched words are hidden.
    #[arg(long, value_name = "PATH")]
    known: Option<PathBuf>,

    /// Extra ignored words file.
    #[arg(long, value_name = "PATH")]
    ignore: Option<PathBuf>,

    /// Keep words appearing at least N times.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    min_count: Option<usize>,

    /// Keep words appearing at most N times.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    max_count: Option<usize>,

    /// Keep words with frequency >= R; accepts 0.05, 5, or 5%.
    #[arg(long, value_name = "R", value_parser = parse_ratio_arg)]
    min_frequency: Option<f64>,

    /// Keep words with frequency <= R; accepts 0.05, 5, or 5%.
    #[arg(long, value_name = "R", value_parser = parse_ratio_arg)]
    max_frequency: Option<f64>,

    /// Keep words appearing in at least N source files.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    min_doc_count: Option<usize>,

    /// Keep words appearing in at most N source files.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    max_doc_count: Option<usize>,

    /// Keep words appearing in at least R of source files.
    #[arg(long, value_name = "R", value_parser = parse_ratio_arg)]
    min_doc_frequency: Option<f64>,

    /// Keep words appearing in at most R of source files.
    #[arg(long, value_name = "R", value_parser = parse_ratio_arg)]
    max_doc_frequency: Option<f64>,

    /// Keep words until cumulative coverage reaches R.
    #[arg(long, value_name = "R", value_parser = parse_ratio_arg)]
    coverage: Option<f64>,

    /// Keep only the first N words after sorting.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    top: Option<usize>,

    /// Source examples retained per word.
    #[arg(long, value_name = "N", value_parser = parse_usize_arg)]
    examples: Option<usize>,

    /// Fetch definitions with an external command template.
    #[arg(
        long,
        value_name = "CMD",
        conflicts_with_all = ["define_youdao", "define_mdx"]
    )]
    define_command: Option<String>,

    /// Fetch definitions with the built-in Youdao API client.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["define_command", "define_mdx"]
    )]
    define_youdao: bool,

    /// Fetch definitions from a local MDict .mdx file or directory.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["define_command", "define_youdao"]
    )]
    define_mdx: Option<PathBuf>,

    /// MDX definition format: plain or html.
    #[arg(
        long,
        value_name = "FORMAT",
        value_parser = parse_mdx_definition_format_arg
    )]
    mdx_definition_format: Option<MdxDefinitionFormat>,

    /// Youdao app key; falls back to YOUDAO_APP_KEY or VUE_APP_YOUDAO_APP_KEY.
    #[arg(long, value_name = "KEY")]
    youdao_app_key: Option<String>,

    /// Youdao app secret; falls back to YOUDAO_APP_SECRET or VUE_APP_YOUDAO_APP_SECRET.
    #[arg(long, value_name = "SECRET")]
    youdao_app_secret: Option<String>,

    /// Youdao source language.
    #[arg(long, value_name = "LANG")]
    youdao_from: Option<String>,

    /// Youdao target language.
    #[arg(long, value_name = "LANG")]
    youdao_to: Option<String>,

    /// Maximum words to define; 0 means unlimited.
    #[arg(long, value_name = "N", value_parser = parse_usize_arg)]
    definition_limit: Option<usize>,

    /// Per-word definition lookup timeout in milliseconds.
    #[arg(long, value_name = "N", value_parser = parse_positive_u64_arg)]
    definition_timeout_ms: Option<u64>,

    /// Maximum characters per definition; 0 means unlimited.
    #[arg(long, value_name = "N", value_parser = parse_usize_arg)]
    definition_max_chars: Option<usize>,

    /// Minimum normalized word length.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize_arg)]
    min_word_len: Option<usize>,

    /// Sort mode: frequency or word.
    #[arg(long, value_name = "MODE", value_parser = parse_sort_mode_arg)]
    sort: Option<SortMode>,

    /// Do not hide the built-in common function words.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "ignore_common")]
    include_common: bool,

    /// Hide the built-in common function words.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "include_common")]
    ignore_common: bool,

    /// Do not hide probable proper nouns.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "ignore_proper_nouns"
    )]
    include_proper_nouns: bool,

    /// Hide probable proper nouns.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "include_proper_nouns"
    )]
    ignore_proper_nouns: bool,
}

#[derive(Debug, Args)]
struct ProfileArgs {
    #[command(subcommand)]
    command: ProfileCommand,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    /// Create a template reader profile.
    Init(ProfileInitArgs),
    /// Add words to the profile's known section.
    #[command(alias = "add-known-words")]
    AddKnown(ProfileWordsArgs),
    /// Add words to the profile's ignore section.
    #[command(aliases = ["add-ignored", "add-ignore-words"])]
    AddIgnore(ProfileWordsArgs),
}

#[derive(Debug, Args)]
struct ProfileInitArgs {
    /// Profile file to create.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// Overwrite an existing profile.
    #[arg(short, long, action = ArgAction::SetTrue)]
    force: bool,
}

#[derive(Debug, Args)]
struct ProfileWordsArgs {
    /// Existing profile file to update.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// Words to add to the profile.
    #[arg(value_name = "WORD", required = true, num_args = 1..)]
    words: Vec<String>,
}

#[derive(Debug, Args)]
struct CompletionArgs {
    /// Shell to generate a completion script for.
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Debug, Clone)]
pub enum CliCommand {
    Analyze(Box<AnalysisConfig>),
    ProfileInit { path: PathBuf, force: bool },
    ProfileAddKnown { path: PathBuf, words: Vec<String> },
    ProfileAddIgnore { path: PathBuf, words: Vec<String> },
    Completions { shell: Shell },
}

pub fn parse_cli_from<I, T>(args: I) -> Result<CliCommand, ClapError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let cli = Cli::try_parse_from(normalize_analyze_shorthand(args))?;
    cli.into_command().map_err(to_clap_error)
}

pub fn parse_args<I>(args: I) -> RebeResult<CliCommand>
where
    I: IntoIterator<Item = String>,
{
    parse_cli_from(args).map_err(|error| RebeError::InvalidArgument(error.to_string()))
}

pub fn command() -> clap::Command {
    Cli::command()
}

impl Cli {
    fn into_command(self) -> RebeResult<CliCommand> {
        match self.command {
            Command::Analyze(args) => Ok(CliCommand::Analyze(Box::new((*args).into_config()?))),
            Command::Profile(args) => match args.command {
                ProfileCommand::Init(args) => Ok(CliCommand::ProfileInit {
                    path: args.path,
                    force: args.force,
                }),
                ProfileCommand::AddKnown(args) => Ok(CliCommand::ProfileAddKnown {
                    path: args.path,
                    words: args.words,
                }),
                ProfileCommand::AddIgnore(args) => Ok(CliCommand::ProfileAddIgnore {
                    path: args.path,
                    words: args.words,
                }),
            },
            Command::Completions(args) => Ok(CliCommand::Completions { shell: args.shell }),
        }
    }
}

impl AnalyzeArgs {
    fn into_config(self) -> RebeResult<AnalysisConfig> {
        let mut config = AnalysisConfig::default();
        let skip_definition_provider_defaults =
            self.define_command.is_some() || self.define_youdao || self.define_mdx.is_some();

        if let Some(path) = &self.profile {
            config.profile_path = Some(path.clone());
            let user_profile = profile::load_user_profile(Some(path))?;
            apply_profile_defaults(
                &mut config,
                &user_profile,
                skip_definition_provider_defaults,
            )?;
        }

        config.input = self.input;
        config.output = self.output;
        config.profile_path = self.profile;

        if let Some(format) = self.format {
            config.format = format;
        }
        if let Some(path) = self.lemma_map {
            config.lemma_map_path = Some(path);
        }
        if let Some(path) = self.known {
            config.known_words_path = Some(path);
        }
        if let Some(path) = self.ignore {
            config.ignore_words_path = Some(path);
        }
        if let Some(value) = self.min_count {
            config.min_count = value;
        }
        if let Some(value) = self.max_count {
            config.max_count = Some(value);
        }
        if let Some(value) = self.min_frequency {
            config.min_frequency = Some(value);
        }
        if let Some(value) = self.max_frequency {
            config.max_frequency = Some(value);
        }
        if let Some(value) = self.min_doc_count {
            config.min_document_count = Some(value);
        }
        if let Some(value) = self.max_doc_count {
            config.max_document_count = Some(value);
        }
        if let Some(value) = self.min_doc_frequency {
            config.min_document_frequency = Some(value);
        }
        if let Some(value) = self.max_doc_frequency {
            config.max_document_frequency = Some(value);
        }
        if let Some(value) = self.coverage {
            config.coverage_target = Some(value);
        }
        if let Some(value) = self.top {
            config.top = Some(value);
        }
        if let Some(value) = self.examples {
            config.example_count = value;
        }
        if let Some(value) = self.define_command {
            config.define_command = Some(value);
        }
        if self.define_youdao {
            config.define_youdao = true;
        }
        if let Some(path) = self.define_mdx {
            config.define_mdx_path = Some(path);
        }
        if let Some(format) = self.mdx_definition_format {
            config.mdx_definition_format = format;
        }
        if let Some(value) = self.youdao_app_key {
            config.youdao_app_key = Some(value);
        }
        if let Some(value) = self.youdao_app_secret {
            config.youdao_app_secret = Some(value);
        }
        if let Some(value) = self.youdao_from {
            config.youdao_from = Some(value);
        }
        if let Some(value) = self.youdao_to {
            config.youdao_to = Some(value);
        }
        if let Some(value) = self.definition_limit {
            config.definition_limit = if value == 0 { None } else { Some(value) };
        }
        if let Some(value) = self.definition_timeout_ms {
            config.definition_timeout_ms = value;
        }
        if let Some(value) = self.definition_max_chars {
            config.definition_max_chars = if value == 0 { None } else { Some(value) };
        }
        if let Some(value) = self.min_word_len {
            config.min_word_len = value;
        }
        if let Some(value) = self.sort {
            config.sort = value;
        }
        if self.include_common {
            config.ignore_common_words = false;
        }
        if self.ignore_common {
            config.ignore_common_words = true;
        }
        if self.include_proper_nouns {
            config.ignore_proper_nouns = false;
        }
        if self.ignore_proper_nouns {
            config.ignore_proper_nouns = true;
        }

        config.validate()?;

        Ok(config)
    }
}

fn normalize_analyze_shorthand<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<OsString>>();

    let Some(first_argument) = args.get(1) else {
        return args;
    };
    let first_argument = first_argument.to_string_lossy();

    if !matches!(
        first_argument.as_ref(),
        "analyze"
            | "profile"
            | "completions"
            | "completion"
            | "help"
            | "-h"
            | "--help"
            | "-V"
            | "--version"
    ) {
        args.insert(1, OsString::from("analyze"));
    }

    args
}

fn to_clap_error(error: RebeError) -> ClapError {
    ClapError::raw(ErrorKind::ValueValidation, error.to_string())
}

fn apply_profile_defaults(
    config: &mut AnalysisConfig,
    user_profile: &profile::UserProfile,
    skip_definition_provider_defaults: bool,
) -> RebeResult<()> {
    for (key, value) in &user_profile.defaults {
        match key.as_str() {
            "format" => {
                config.format = OutputFormat::parse(value)?;
            }
            "min_count" => {
                config.min_count = parse_positive_usize(value, "profile default min-count")?;
            }
            "max_count" => {
                config.max_count = Some(parse_positive_usize(value, "profile default max-count")?);
            }
            "min_frequency" => {
                config.min_frequency = Some(parse_ratio(value, "profile default min-frequency")?);
            }
            "max_frequency" => {
                config.max_frequency = Some(parse_ratio(value, "profile default max-frequency")?);
            }
            "min_doc_count" | "min_document_count" => {
                config.min_document_count = Some(parse_positive_usize(
                    value,
                    "profile default min-doc-count",
                )?);
            }
            "max_doc_count" | "max_document_count" => {
                config.max_document_count = Some(parse_positive_usize(
                    value,
                    "profile default max-doc-count",
                )?);
            }
            "min_doc_frequency" | "min_document_frequency" => {
                config.min_document_frequency =
                    Some(parse_ratio(value, "profile default min-doc-frequency")?);
            }
            "max_doc_frequency" | "max_document_frequency" => {
                config.max_document_frequency =
                    Some(parse_ratio(value, "profile default max-doc-frequency")?);
            }
            "coverage" => {
                config.coverage_target = Some(parse_ratio(value, "profile default coverage")?);
            }
            "top" => {
                config.top = Some(parse_positive_usize(value, "profile default top")?);
            }
            "examples" => {
                config.example_count = parse_usize(value, "profile default examples")?;
            }
            "min_word_len" => {
                config.min_word_len = parse_positive_usize(value, "profile default min-word-len")?;
            }
            "sort" => {
                config.sort = SortMode::parse(value)?;
            }
            "include_common" => {
                config.ignore_common_words = !parse_bool(value, "profile default include-common")?;
            }
            "ignore_common_words" => {
                config.ignore_common_words =
                    parse_bool(value, "profile default ignore-common-words")?;
            }
            "include_proper_nouns" => {
                config.ignore_proper_nouns =
                    !parse_bool(value, "profile default include-proper-nouns")?;
            }
            "ignore_proper_nouns" => {
                config.ignore_proper_nouns =
                    parse_bool(value, "profile default ignore-proper-nouns")?;
            }
            "define_command" => {
                if !skip_definition_provider_defaults {
                    config.define_command =
                        Some(non_empty_profile_value(value, "define-command")?.to_string());
                }
            }
            "define_youdao" => {
                if !skip_definition_provider_defaults {
                    config.define_youdao = parse_bool(value, "profile default define-youdao")?;
                }
            }
            "define_mdx" | "mdx" | "mdx_path" => {
                if !skip_definition_provider_defaults {
                    config.define_mdx_path =
                        Some(PathBuf::from(non_empty_profile_value(value, "define-mdx")?));
                }
            }
            "mdx_definition_format" => {
                config.mdx_definition_format = MdxDefinitionFormat::parse(value)?;
            }
            "youdao_app_key" => {
                config.youdao_app_key =
                    Some(non_empty_profile_value(value, "youdao-app-key")?.to_string());
            }
            "youdao_app_secret" => {
                config.youdao_app_secret =
                    Some(non_empty_profile_value(value, "youdao-app-secret")?.to_string());
            }
            "youdao_from" => {
                config.youdao_from =
                    Some(non_empty_profile_value(value, "youdao-from")?.to_string());
            }
            "youdao_to" => {
                config.youdao_to = Some(non_empty_profile_value(value, "youdao-to")?.to_string());
            }
            "definition_limit" => {
                let limit = parse_usize(value, "profile default definition-limit")?;
                config.definition_limit = if limit == 0 { None } else { Some(limit) };
            }
            "definition_timeout_ms" => {
                config.definition_timeout_ms =
                    parse_positive_u64(value, "profile default definition-timeout-ms")?;
            }
            "definition_max_chars" => {
                let max_chars = parse_usize(value, "profile default definition-max-chars")?;
                config.definition_max_chars = if max_chars == 0 {
                    None
                } else {
                    Some(max_chars)
                };
            }
            _ => {
                return Err(RebeError::InvalidArgument(format!(
                    "unsupported profile default: {key}"
                )));
            }
        }
    }

    Ok(())
}

fn parse_output_format_arg(value: &str) -> Result<OutputFormat, String> {
    OutputFormat::parse(value).map_err(|error| error.to_string())
}

fn parse_mdx_definition_format_arg(value: &str) -> Result<MdxDefinitionFormat, String> {
    MdxDefinitionFormat::parse(value).map_err(|error| error.to_string())
}

fn parse_sort_mode_arg(value: &str) -> Result<SortMode, String> {
    SortMode::parse(value).map_err(|error| error.to_string())
}

fn parse_positive_usize_arg(value: &str) -> Result<usize, String> {
    parse_positive_usize(value, "value").map_err(|error| error.to_string())
}

fn parse_usize_arg(value: &str) -> Result<usize, String> {
    parse_usize(value, "value").map_err(|error| error.to_string())
}

fn parse_positive_u64_arg(value: &str) -> Result<u64, String> {
    parse_positive_u64(value, "value").map_err(|error| error.to_string())
}

fn parse_ratio_arg(value: &str) -> Result<f64, String> {
    parse_ratio(value, "value").map_err(|error| error.to_string())
}

fn parse_bool(value: &str, option: &str) -> RebeResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(RebeError::InvalidArgument(format!(
            "{option} expects true or false, got {value}"
        ))),
    }
}

fn non_empty_profile_value<'a>(value: &'a str, option: &str) -> RebeResult<&'a str> {
    let value = value.trim();

    if value.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "profile default {option} cannot be empty"
        )));
    }

    Ok(value)
}

fn parse_usize(value: &str, option: &str) -> RebeResult<usize> {
    value
        .parse::<usize>()
        .map_err(|_| RebeError::InvalidArgument(format!("{option} expects a number, got {value}")))
}

fn parse_positive_usize(value: &str, option: &str) -> RebeResult<usize> {
    let parsed = parse_usize(value, option)?;

    if parsed == 0 {
        return Err(RebeError::InvalidArgument(format!(
            "{option} must be greater than 0"
        )));
    }

    Ok(parsed)
}

fn parse_u64(value: &str, option: &str) -> RebeResult<u64> {
    value
        .parse::<u64>()
        .map_err(|_| RebeError::InvalidArgument(format!("{option} expects a number, got {value}")))
}

fn parse_positive_u64(value: &str, option: &str) -> RebeResult<u64> {
    let parsed = parse_u64(value, option)?;

    if parsed == 0 {
        return Err(RebeError::InvalidArgument(format!(
            "{option} must be greater than 0"
        )));
    }

    Ok(parsed)
}

fn parse_ratio(value: &str, option: &str) -> RebeResult<f64> {
    let trimmed = value.trim();
    let without_percent = trimmed.strip_suffix('%').unwrap_or(trimmed);
    let parsed = without_percent.parse::<f64>().map_err(|_| {
        RebeError::InvalidArgument(format!("{option} expects a ratio, got {value}"))
    })?;
    let ratio = if trimmed.ends_with('%') || parsed > 1.0 {
        parsed / 100.0
    } else {
        parsed
    };

    if !(0.0..=1.0).contains(&ratio) || ratio == 0.0 {
        return Err(RebeError::InvalidArgument(format!(
            "{option} must be greater than 0 and less than or equal to 1"
        )));
    }

    Ok(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_analyze_command() {
        let args = vec![
            "rebe".to_string(),
            "analyze".to_string(),
            "book.txt".to_string(),
            "--min-count".to_string(),
            "3".to_string(),
            "--format".to_string(),
            "csv".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.input, PathBuf::from("book.txt"));
                assert_eq!(config.min_count, 3);
                assert_eq!(config.format, OutputFormat::Csv);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_analyze_shorthand_with_leading_option() {
        let args = vec![
            "rebe".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "book.txt".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.input, PathBuf::from("book.txt"));
                assert_eq!(config.format, OutputFormat::Json);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_profile_init_command() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "init".to_string(),
            "profile.ini".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::ProfileInit { path, force } => {
                assert_eq!(path, PathBuf::from("profile.ini"));
                assert!(!force);
            }
            _ => panic!("expected profile init command"),
        }
    }

    #[test]
    fn parses_profile_init_force_command() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "init".to_string(),
            "profile.ini".to_string(),
            "--force".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::ProfileInit { path, force } => {
                assert_eq!(path, PathBuf::from("profile.ini"));
                assert!(force);
            }
            _ => panic!("expected profile init command"),
        }
    }

    #[test]
    fn rejects_profile_init_without_path() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "init".to_string(),
        ];

        let err = parse_args(args).expect_err("profile init should require path");
        assert!(err.to_string().contains("<PATH>"));
    }

    #[test]
    fn parses_profile_add_known_command() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-known".to_string(),
            "profile.ini".to_string(),
            "reader".to_string(),
            "finished books".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::ProfileAddKnown { path, words } => {
                assert_eq!(path, PathBuf::from("profile.ini"));
                assert_eq!(
                    words,
                    vec!["reader".to_string(), "finished books".to_string()]
                );
            }
            _ => panic!("expected profile add-known command"),
        }
    }

    #[test]
    fn parses_profile_command_aliases() {
        let command = parse_args(vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-ignore-words".to_string(),
            "profile.ini".to_string(),
            "alice".to_string(),
        ])
        .expect("parse alias");

        assert!(matches!(command, CliCommand::ProfileAddIgnore { .. }));
    }

    #[test]
    fn rejects_profile_add_known_without_words() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-known".to_string(),
            "profile.ini".to_string(),
        ];

        let err = parse_args(args).expect_err("profile add-known should require words");
        assert!(err.to_string().contains("<WORD>"));
    }

    #[test]
    fn rejects_missing_input() {
        let args = vec!["rebe".to_string(), "analyze".to_string()];
        let err = parse_args(args).expect_err("missing input should fail");
        assert!(err.to_string().contains("<INPUT>"));
    }

    #[test]
    fn parses_ratio_options() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--min-frequency".to_string(),
            "2%".to_string(),
            "--max-frequency".to_string(),
            "20".to_string(),
            "--coverage".to_string(),
            "0.8".to_string(),
            "--include-proper-nouns".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.min_frequency, Some(0.02));
                assert_eq!(config.max_frequency, Some(0.2));
                assert_eq!(config.coverage_target, Some(0.8));
                assert!(!config.ignore_proper_nouns);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn rejects_invalid_ratio() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--coverage".to_string(),
            "120".to_string(),
        ];

        let err = parse_args(args).expect_err("invalid ratio should fail");
        assert!(err.to_string().contains("value must be greater than 0"));
    }

    #[test]
    fn parses_definition_options() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--define-command".to_string(),
            "printf 'meaning %s' {word}".to_string(),
            "--definition-limit".to_string(),
            "3".to_string(),
            "--definition-timeout-ms".to_string(),
            "2000".to_string(),
            "--definition-max-chars".to_string(),
            "120".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(
                    config.define_command,
                    Some("printf 'meaning %s' {word}".to_string())
                );
                assert_eq!(config.definition_limit, Some(3));
                assert_eq!(config.definition_timeout_ms, 2000);
                assert_eq!(config.definition_max_chars, Some(120));
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_unlimited_definition_max_chars() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--definition-max-chars".to_string(),
            "0".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.definition_max_chars, None);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn applies_profile_defaults_from_file() {
        let profile_path = temp_file_path("profile_defaults", "ini");
        fs::write(
            &profile_path,
            r#"
            [defaults]
            min-count = 2
            format = json
            sort = word
            include-common = true
            definition-limit = 0
            definition-max-chars = 300
            "#,
        )
        .expect("write profile");

        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--profile".to_string(),
            profile_path.display().to_string(),
        ];
        let command = parse_args(args).expect("parse args");

        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.min_count, 2);
                assert_eq!(config.format, OutputFormat::Json);
                assert_eq!(config.sort, SortMode::Word);
                assert!(!config.ignore_common_words);
                assert_eq!(config.definition_limit, None);
                assert_eq!(config.definition_max_chars, Some(300));
            }
            _ => panic!("expected analyze command"),
        }

        fs::remove_file(profile_path).ok();
    }

    #[test]
    fn command_line_overrides_profile_defaults() {
        let profile_path = temp_file_path("profile_override", "ini");
        fs::write(
            &profile_path,
            r#"
            [defaults]
            min-count = 2
            format = json
            include-common = true
            "#,
        )
        .expect("write profile");

        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--profile".to_string(),
            profile_path.display().to_string(),
            "--min-count".to_string(),
            "1".to_string(),
            "--format".to_string(),
            "csv".to_string(),
            "--ignore-common".to_string(),
        ];
        let command = parse_args(args).expect("parse args");

        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.min_count, 1);
                assert_eq!(config.format, OutputFormat::Csv);
                assert!(config.ignore_common_words);
            }
            _ => panic!("expected analyze command"),
        }

        fs::remove_file(profile_path).ok();
    }

    #[test]
    fn explicit_definition_provider_ignores_profile_provider_default() {
        let profile_path = temp_file_path("profile_provider", "ini");
        fs::write(
            &profile_path,
            r#"
            [defaults]
            define-mdx = dicts/longman.mdx
            mdx-definition-format = html
            "#,
        )
        .expect("write profile");

        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--profile".to_string(),
            profile_path.display().to_string(),
            "--define-youdao".to_string(),
            "--youdao-app-key".to_string(),
            "key".to_string(),
            "--youdao-app-secret".to_string(),
            "secret".to_string(),
        ];
        let command = parse_args(args).expect("parse args");

        match command {
            CliCommand::Analyze(config) => {
                assert!(config.define_youdao);
                assert_eq!(config.define_mdx_path, None);
                assert_eq!(config.mdx_definition_format, MdxDefinitionFormat::Html);
            }
            _ => panic!("expected analyze command"),
        }

        fs::remove_file(profile_path).ok();
    }

    #[test]
    fn applies_profile_defaults_directly() {
        let mut defaults = BTreeMap::new();
        defaults.insert("top".to_string(), "10".to_string());
        defaults.insert("min_frequency".to_string(), "2%".to_string());
        defaults.insert("define_mdx".to_string(), "dicts/longman.mdx".to_string());
        defaults.insert("mdx_definition_format".to_string(), "html".to_string());
        let user_profile = profile::UserProfile {
            defaults,
            ..profile::UserProfile::default()
        };
        let mut config = AnalysisConfig::default();

        apply_profile_defaults(&mut config, &user_profile, false).expect("apply defaults");

        assert_eq!(config.top, Some(10));
        assert_eq!(config.min_frequency, Some(0.02));
        assert_eq!(
            config.define_mdx_path,
            Some(PathBuf::from("dicts/longman.mdx"))
        );
        assert_eq!(config.mdx_definition_format, MdxDefinitionFormat::Html);
    }

    #[test]
    fn parses_youdao_definition_options() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--define-youdao".to_string(),
            "--youdao-app-key".to_string(),
            "key".to_string(),
            "--youdao-app-secret".to_string(),
            "secret".to_string(),
            "--youdao-from".to_string(),
            "en".to_string(),
            "--youdao-to".to_string(),
            "zh-CHS".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert!(config.define_youdao);
                assert_eq!(config.youdao_app_key, Some("key".to_string()));
                assert_eq!(config.youdao_app_secret, Some("secret".to_string()));
                assert_eq!(config.youdao_from, Some("en".to_string()));
                assert_eq!(config.youdao_to, Some("zh-CHS".to_string()));
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_mdx_definition_option() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--define-mdx".to_string(),
            "dicts/longman.mdx".to_string(),
            "--mdx-definition-format".to_string(),
            "html".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(
                    config.define_mdx_path,
                    Some(PathBuf::from("dicts/longman.mdx"))
                );
                assert_eq!(config.mdx_definition_format, MdxDefinitionFormat::Html);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn rejects_multiple_definition_providers() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--define-command".to_string(),
            "printf 'meaning %s' {word}".to_string(),
            "--define-youdao".to_string(),
        ];

        let err = parse_args(args).expect_err("definition providers should conflict");
        assert!(err.to_string().contains("cannot be used with"));
    }

    #[test]
    fn rejects_definition_command_without_placeholder() {
        let args = vec![
            "rebe".to_string(),
            "book.txt".to_string(),
            "--define-command".to_string(),
            "printf missing".to_string(),
        ];

        let err = parse_args(args).expect_err("definition command should require placeholder");
        assert!(err.to_string().contains("--define-command"));
    }

    #[test]
    fn parses_lemma_and_document_filter_options() {
        let args = vec![
            "rebe".to_string(),
            "articles".to_string(),
            "--lemma-map".to_string(),
            "lemma.txt".to_string(),
            "--min-doc-count".to_string(),
            "2".to_string(),
            "--max-doc-count".to_string(),
            "5".to_string(),
            "--min-doc-frequency".to_string(),
            "20%".to_string(),
            "--max-doc-frequency".to_string(),
            "0.8".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::Analyze(config) => {
                assert_eq!(config.lemma_map_path, Some(PathBuf::from("lemma.txt")));
                assert_eq!(config.min_document_count, Some(2));
                assert_eq!(config.max_document_count, Some(5));
                assert_eq!(config.min_document_frequency, Some(0.2));
                assert_eq!(config.max_document_frequency, Some(0.8));
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn rejects_invalid_document_count_range() {
        let args = vec![
            "rebe".to_string(),
            "articles".to_string(),
            "--min-doc-count".to_string(),
            "3".to_string(),
            "--max-doc-count".to_string(),
            "2".to_string(),
        ];

        let err = parse_args(args).expect_err("invalid document count range should fail");
        assert!(err.to_string().contains("--max-doc-count"));
    }

    #[test]
    fn parses_completion_command() {
        let command = parse_args(vec![
            "rebe".to_string(),
            "completions".to_string(),
            "bash".to_string(),
        ])
        .expect("parse completions");

        match command {
            CliCommand::Completions { shell } => assert_eq!(shell, Shell::Bash),
            _ => panic!("expected completions command"),
        }
    }

    #[test]
    fn generated_command_includes_completion_subcommand() {
        let command = command();
        assert!(command
            .get_subcommands()
            .any(|subcommand| subcommand.get_name() == "completions"));
    }

    fn temp_file_path(name: &str, extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_cli_{name}_{nanos}.{extension}"))
    }
}
