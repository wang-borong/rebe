use std::path::PathBuf;

use crate::analysis::{AnalysisConfig, SortMode};
use crate::dict::MdxDefinitionFormat;
use crate::error::{RebeError, RebeResult};
use crate::export::OutputFormat;
use crate::profile;

#[derive(Debug, Clone)]
pub enum CliCommand {
    Analyze(AnalysisConfig),
    ProfileInit { path: PathBuf, force: bool },
    ProfileAddKnown { path: PathBuf, words: Vec<String> },
    ProfileAddIgnore { path: PathBuf, words: Vec<String> },
    Help,
}

pub fn parse_args<I>(args: I) -> RebeResult<CliCommand>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter().skip(1).collect::<Vec<_>>();

    if args.is_empty() {
        return Ok(CliCommand::Help);
    }

    if is_help_flag(&args[0]) || args[0] == "help" {
        return Ok(CliCommand::Help);
    }

    if args[0] == "profile" {
        args.remove(0);
        return parse_profile_args(args);
    }

    if args[0] == "analyze" {
        args.remove(0);
    }

    parse_analyze_args(args)
}

pub fn help_text() -> &'static str {
    "Read English Book Easily\n\
\n\
USAGE:\n\
    rebe analyze <INPUT> [OPTIONS]\n\
    rebe <INPUT> [OPTIONS]\n\
    rebe profile init <PATH> [--force]\n\
    rebe profile add-known <PATH> <WORD>...\n\
    rebe profile add-ignore <PATH> <WORD>...\n\
\n\
OPTIONS:\n\
    -o, --output <PATH>       Write result to a file instead of stdout\n\
        --format <FORMAT>     Output format: txt, csv, json (default: txt)\n\
        --profile <PATH>      User profile with [known], [ignore], [lemma], and [defaults] sections\n\
        --lemma-map <PATH>    Lemma override file; supports 'surface lemma', 'surface=lemma', or 'surface,lemma'\n\
        --known <PATH>        Known words file; matched words are hidden\n\
        --ignore <PATH>       Extra ignored words file\n\
        --min-count <N>       Keep words appearing at least N times (default: 1)\n\
        --max-count <N>       Keep words appearing at most N times\n\
        --min-frequency <R>   Keep words with frequency >= R; accepts 0.05, 5, or 5%\n\
        --max-frequency <R>   Keep words with frequency <= R; accepts 0.05, 5, or 5%\n\
        --min-doc-count <N>   Keep words appearing in at least N source files\n\
        --max-doc-count <N>   Keep words appearing in at most N source files\n\
        --min-doc-frequency <R> Keep words appearing in at least R of source files\n\
        --max-doc-frequency <R> Keep words appearing in at most R of source files\n\
        --coverage <R>        Keep words until cumulative coverage reaches R\n\
        --top <N>             Keep only the first N words after sorting\n\
        --examples <N>        Examples per word from source text (default: 2)\n\
        --define-command <CMD> Fetch definitions with an external command template\n\
        --define-youdao       Fetch definitions with the built-in Youdao API client\n\
        --define-mdx <PATH>   Fetch definitions from a local MDict .mdx file or directory\n\
        --mdx-definition-format <FORMAT> MDX definition format: plain, html (default: plain)\n\
        --youdao-app-key <KEY> Youdao app key; falls back to YOUDAO_APP_KEY or VUE_APP_YOUDAO_APP_KEY\n\
        --youdao-app-secret <SECRET> Youdao app secret; falls back to YOUDAO_APP_SECRET or VUE_APP_YOUDAO_APP_SECRET\n\
        --youdao-from <LANG>  Youdao source language (default: en)\n\
        --youdao-to <LANG>    Youdao target language (default: zh-CHS)\n\
        --definition-limit <N> Max words to define when a definition provider is used (default: 50; 0 = unlimited)\n\
        --definition-timeout-ms <N> Per-word definition lookup timeout (default: 10000)\n\
        --definition-max-chars <N> Max characters per definition (default: 600; 0 = unlimited)\n\
        --min-word-len <N>    Minimum normalized word length (default: 1)\n\
        --sort <MODE>         Sort mode: frequency, word (default: frequency)\n\
        --include-common      Do not hide the built-in common function words\n\
        --ignore-common       Hide the built-in common function words\n\
        --include-proper-nouns Do not hide probable proper nouns\n\
        --ignore-proper-nouns Hide probable proper nouns\n\
    -h, --help                Print this help\n\
\n\
PROFILE COMMANDS:\n\
    rebe profile init <PATH>      Create a template user profile; refuses to overwrite by default\n\
    rebe profile init <PATH> -f   Overwrite an existing profile\n\
    rebe profile add-known <PATH> <WORD>... Append new words to the profile [known] section\n\
    rebe profile add-ignore <PATH> <WORD>... Append new words to the profile [ignore] section\n\
\n\
EXAMPLE:\n\
    rebe analyze book.txt --known known_words.txt --min-count 3 --format csv -o words.csv\n"
}

fn parse_profile_args(args: Vec<String>) -> RebeResult<CliCommand> {
    if args.is_empty() || is_help_flag(&args[0]) || args[0] == "help" {
        return Ok(CliCommand::Help);
    }

    match args[0].as_str() {
        "init" => parse_profile_init_args(args),
        "add-known" | "add-known-words" => parse_profile_add_known_args(args),
        "add-ignore" | "add-ignored" | "add-ignore-words" => parse_profile_add_ignore_args(args),
        command => Err(RebeError::InvalidArgument(format!(
            "unknown profile command: {command}; expected init, add-known, or add-ignore"
        ))),
    }
}

fn parse_profile_init_args(args: Vec<String>) -> RebeResult<CliCommand> {
    let mut path = None;
    let mut force = false;
    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "-f" | "--force" => {
                force = true;
            }
            _ if arg.starts_with('-') => {
                return Err(RebeError::InvalidArgument(format!("unknown option: {arg}")));
            }
            _ => {
                if path.is_some() {
                    return Err(RebeError::InvalidArgument(format!(
                        "unexpected extra profile path: {arg}"
                    )));
                }

                path = Some(PathBuf::from(arg));
            }
        }

        index += 1;
    }

    let path = path.ok_or_else(|| {
        RebeError::InvalidArgument("missing profile path for profile init".to_string())
    })?;

    Ok(CliCommand::ProfileInit { path, force })
}

fn parse_profile_add_known_args(args: Vec<String>) -> RebeResult<CliCommand> {
    if args.iter().skip(1).any(|arg| is_help_flag(arg)) {
        return Ok(CliCommand::Help);
    }

    let (path, words) = parse_profile_add_words_args(args, "add-known")?;

    Ok(CliCommand::ProfileAddKnown { path, words })
}

fn parse_profile_add_ignore_args(args: Vec<String>) -> RebeResult<CliCommand> {
    if args.iter().skip(1).any(|arg| is_help_flag(arg)) {
        return Ok(CliCommand::Help);
    }

    let (path, words) = parse_profile_add_words_args(args, "add-ignore")?;

    Ok(CliCommand::ProfileAddIgnore { path, words })
}

fn parse_profile_add_words_args(
    args: Vec<String>,
    command: &str,
) -> RebeResult<(PathBuf, Vec<String>)> {
    let mut path = None;
    let mut words = Vec::new();
    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            _ if arg.starts_with('-') => {
                return Err(RebeError::InvalidArgument(format!("unknown option: {arg}")));
            }
            _ => {
                if path.is_none() {
                    path = Some(PathBuf::from(arg));
                } else {
                    words.push(arg.clone());
                }
            }
        }

        index += 1;
    }

    let path = path.ok_or_else(|| {
        RebeError::InvalidArgument(format!("missing profile path for profile {command}"))
    })?;

    if words.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "profile {command} expects at least one word"
        )));
    }

    Ok((path, words))
}

fn parse_analyze_args(args: Vec<String>) -> RebeResult<CliCommand> {
    let mut config = AnalysisConfig::default();
    let profile_path = find_profile_path(&args)?;

    if let Some(path) = &profile_path {
        config.profile_path = Some(path.clone());
        let user_profile = profile::load_user_profile(Some(path))?;
        let skip_definition_provider_defaults = contains_any_option(
            &args,
            &["--define-command", "--define-youdao", "--define-mdx"],
        );

        apply_profile_defaults(
            &mut config,
            &user_profile,
            skip_definition_provider_defaults,
        )?;
    }

    let mut input = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];

        match arg.as_str() {
            "-h" | "--help" => return Ok(CliCommand::Help),
            "-o" | "--output" => {
                config.output = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--format" => {
                config.format = OutputFormat::parse(&next_value(&args, &mut index, arg)?)?;
            }
            "--profile" => {
                config.profile_path = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--lemma-map" => {
                config.lemma_map_path = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--known" => {
                config.known_words_path = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--ignore" => {
                config.ignore_words_path = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--min-count" => {
                config.min_count = parse_positive_usize(&next_value(&args, &mut index, arg)?, arg)?;
            }
            "--max-count" => {
                config.max_count = Some(parse_positive_usize(
                    &next_value(&args, &mut index, arg)?,
                    arg,
                )?);
            }
            "--min-frequency" => {
                config.min_frequency =
                    Some(parse_ratio(&next_value(&args, &mut index, arg)?, arg)?);
            }
            "--max-frequency" => {
                config.max_frequency =
                    Some(parse_ratio(&next_value(&args, &mut index, arg)?, arg)?);
            }
            "--min-doc-count" => {
                config.min_document_count = Some(parse_positive_usize(
                    &next_value(&args, &mut index, arg)?,
                    arg,
                )?);
            }
            "--max-doc-count" => {
                config.max_document_count = Some(parse_positive_usize(
                    &next_value(&args, &mut index, arg)?,
                    arg,
                )?);
            }
            "--min-doc-frequency" => {
                config.min_document_frequency =
                    Some(parse_ratio(&next_value(&args, &mut index, arg)?, arg)?);
            }
            "--max-doc-frequency" => {
                config.max_document_frequency =
                    Some(parse_ratio(&next_value(&args, &mut index, arg)?, arg)?);
            }
            "--coverage" => {
                config.coverage_target =
                    Some(parse_ratio(&next_value(&args, &mut index, arg)?, arg)?);
            }
            "--top" => {
                config.top = Some(parse_positive_usize(
                    &next_value(&args, &mut index, arg)?,
                    arg,
                )?);
            }
            "--examples" => {
                config.example_count = parse_usize(&next_value(&args, &mut index, arg)?, arg)?;
            }
            "--define-command" => {
                config.define_command = Some(next_value(&args, &mut index, arg)?);
            }
            "--define-youdao" => {
                config.define_youdao = true;
            }
            "--define-mdx" => {
                config.define_mdx_path = Some(PathBuf::from(next_value(&args, &mut index, arg)?));
            }
            "--mdx-definition-format" => {
                config.mdx_definition_format =
                    MdxDefinitionFormat::parse(&next_value(&args, &mut index, arg)?)?;
            }
            "--youdao-app-key" => {
                config.youdao_app_key = Some(next_value(&args, &mut index, arg)?);
            }
            "--youdao-app-secret" => {
                config.youdao_app_secret = Some(next_value(&args, &mut index, arg)?);
            }
            "--youdao-from" => {
                config.youdao_from = Some(next_value(&args, &mut index, arg)?);
            }
            "--youdao-to" => {
                config.youdao_to = Some(next_value(&args, &mut index, arg)?);
            }
            "--definition-limit" => {
                let limit = parse_usize(&next_value(&args, &mut index, arg)?, arg)?;
                config.definition_limit = if limit == 0 { None } else { Some(limit) };
            }
            "--definition-timeout-ms" => {
                config.definition_timeout_ms =
                    parse_positive_u64(&next_value(&args, &mut index, arg)?, arg)?;
            }
            "--definition-max-chars" => {
                let max_chars = parse_usize(&next_value(&args, &mut index, arg)?, arg)?;
                config.definition_max_chars = if max_chars == 0 {
                    None
                } else {
                    Some(max_chars)
                };
            }
            "--min-word-len" => {
                config.min_word_len =
                    parse_positive_usize(&next_value(&args, &mut index, arg)?, arg)?;
            }
            "--sort" => {
                config.sort = SortMode::parse(&next_value(&args, &mut index, arg)?)?;
            }
            "--include-common" => {
                config.ignore_common_words = false;
            }
            "--ignore-common" => {
                config.ignore_common_words = true;
            }
            "--include-proper-nouns" => {
                config.ignore_proper_nouns = false;
            }
            "--ignore-proper-nouns" => {
                config.ignore_proper_nouns = true;
            }
            _ if arg.starts_with('-') => {
                return Err(RebeError::InvalidArgument(format!("unknown option: {arg}")));
            }
            _ => {
                if input.is_some() {
                    return Err(RebeError::InvalidArgument(format!(
                        "unexpected extra input: {arg}"
                    )));
                }

                input = Some(PathBuf::from(arg));
            }
        }

        index += 1;
    }

    config.input =
        input.ok_or_else(|| RebeError::InvalidArgument("missing input file".to_string()))?;
    config.validate()?;

    Ok(CliCommand::Analyze(config))
}

fn find_profile_path(args: &[String]) -> RebeResult<Option<PathBuf>> {
    let mut profile_path = None;
    let mut index = 0;

    while index < args.len() {
        if args[index] == "--profile" {
            let path = args.get(index + 1).cloned().ok_or_else(|| {
                RebeError::InvalidArgument("missing value for --profile".to_string())
            })?;
            profile_path = Some(PathBuf::from(path));
            index += 1;
        }

        index += 1;
    }

    Ok(profile_path)
}

fn contains_any_option(args: &[String], options: &[&str]) -> bool {
    args.iter()
        .any(|arg| options.iter().any(|option| arg == option))
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

fn next_value(args: &[String], index: &mut usize, option: &str) -> RebeResult<String> {
    *index += 1;

    args.get(*index)
        .cloned()
        .ok_or_else(|| RebeError::InvalidArgument(format!("missing value for {option}")))
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

fn is_help_flag(arg: &str) -> bool {
    arg == "-h" || arg == "--help"
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
        assert!(err.to_string().contains("missing profile path"));
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
    fn rejects_profile_add_known_without_words() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-known".to_string(),
            "profile.ini".to_string(),
        ];

        let err = parse_args(args).expect_err("profile add-known should require words");
        assert!(err.to_string().contains("expects at least one word"));
    }

    #[test]
    fn parses_profile_add_ignore_command() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-ignore".to_string(),
            "profile.ini".to_string(),
            "alice".to_string(),
            "project terms".to_string(),
        ];

        let command = parse_args(args).expect("parse args");
        match command {
            CliCommand::ProfileAddIgnore { path, words } => {
                assert_eq!(path, PathBuf::from("profile.ini"));
                assert_eq!(
                    words,
                    vec!["alice".to_string(), "project terms".to_string()]
                );
            }
            _ => panic!("expected profile add-ignore command"),
        }
    }

    #[test]
    fn rejects_profile_add_ignore_without_words() {
        let args = vec![
            "rebe".to_string(),
            "profile".to_string(),
            "add-ignore".to_string(),
            "profile.ini".to_string(),
        ];

        let err = parse_args(args).expect_err("profile add-ignore should require words");
        assert!(err
            .to_string()
            .contains("profile add-ignore expects at least one word"));
    }

    #[test]
    fn rejects_missing_input() {
        let args = vec!["rebe".to_string(), "analyze".to_string()];
        let err = parse_args(args).expect_err("missing input should fail");
        assert!(err.to_string().contains("missing input"));
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
        assert!(err.to_string().contains("--coverage"));
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
            "--youdao-app-key".to_string(),
            "key".to_string(),
            "--youdao-app-secret".to_string(),
            "secret".to_string(),
        ];

        let err = parse_args(args).expect_err("definition providers should conflict");
        assert!(err
            .to_string()
            .contains("--define-command, --define-youdao, and --define-mdx"));
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

    fn temp_file_path(name: &str, extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_cli_{name}_{nanos}.{extension}"))
    }
}
