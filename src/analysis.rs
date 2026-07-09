use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use crate::dict::{
    DefinitionCommand, DefinitionProvider, MdxDefinitionClient, MdxDefinitionFormat,
    YoudaoDefinitionClient, YoudaoDefinitionConfig, DEFAULT_DEFINITION_MAX_CHARS,
};
use crate::document::{load_documents, Document};
use crate::error::{RebeError, RebeResult};
use crate::export::OutputFormat;
use crate::profile;
use crate::text;

const MAX_EXAMPLE_CHARS: usize = 240;
const DEFAULT_DEFINITION_LIMIT: usize = 50;
const DEFAULT_DEFINITION_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Frequency,
    Word,
}

impl SortMode {
    pub fn parse(value: &str) -> RebeResult<Self> {
        match value {
            "frequency" | "freq" | "count" => Ok(Self::Frequency),
            "word" | "alpha" | "alphabetical" => Ok(Self::Word),
            _ => Err(RebeError::InvalidArgument(format!(
                "unsupported sort mode: {value}; expected frequency or word"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub format: OutputFormat,
    pub profile_path: Option<PathBuf>,
    pub lemma_map_path: Option<PathBuf>,
    pub known_words_path: Option<PathBuf>,
    pub ignore_words_path: Option<PathBuf>,
    pub min_count: usize,
    pub max_count: Option<usize>,
    pub min_frequency: Option<f64>,
    pub max_frequency: Option<f64>,
    pub min_document_count: Option<usize>,
    pub max_document_count: Option<usize>,
    pub min_document_frequency: Option<f64>,
    pub max_document_frequency: Option<f64>,
    pub coverage_target: Option<f64>,
    pub top: Option<usize>,
    pub example_count: usize,
    pub min_word_len: usize,
    pub sort: SortMode,
    pub ignore_common_words: bool,
    pub ignore_proper_nouns: bool,
    pub define_command: Option<String>,
    pub define_youdao: bool,
    pub define_mdx_path: Option<PathBuf>,
    pub mdx_definition_format: MdxDefinitionFormat,
    pub youdao_app_key: Option<String>,
    pub youdao_app_secret: Option<String>,
    pub youdao_from: Option<String>,
    pub youdao_to: Option<String>,
    pub definition_limit: Option<usize>,
    pub definition_timeout_ms: u64,
    pub definition_max_chars: Option<usize>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            format: OutputFormat::Txt,
            profile_path: None,
            lemma_map_path: None,
            known_words_path: None,
            ignore_words_path: None,
            min_count: 1,
            max_count: None,
            min_frequency: None,
            max_frequency: None,
            min_document_count: None,
            max_document_count: None,
            min_document_frequency: None,
            max_document_frequency: None,
            coverage_target: None,
            top: None,
            example_count: 2,
            min_word_len: 1,
            sort: SortMode::Frequency,
            ignore_common_words: true,
            ignore_proper_nouns: true,
            define_command: None,
            define_youdao: false,
            define_mdx_path: None,
            mdx_definition_format: MdxDefinitionFormat::Plain,
            youdao_app_key: None,
            youdao_app_secret: None,
            youdao_from: None,
            youdao_to: None,
            definition_limit: Some(DEFAULT_DEFINITION_LIMIT),
            definition_timeout_ms: DEFAULT_DEFINITION_TIMEOUT_MS,
            definition_max_chars: Some(DEFAULT_DEFINITION_MAX_CHARS),
        }
    }
}

impl AnalysisConfig {
    pub fn validate(&self) -> RebeResult<()> {
        if self.input.as_os_str().is_empty() {
            return Err(RebeError::InvalidArgument("missing input file".to_string()));
        }

        if let Some(max_count) = self.max_count {
            if max_count < self.min_count {
                return Err(RebeError::InvalidArgument(
                    "--max-count must be greater than or equal to --min-count".to_string(),
                ));
            }
        }

        if let (Some(min_frequency), Some(max_frequency)) = (self.min_frequency, self.max_frequency)
        {
            if max_frequency < min_frequency {
                return Err(RebeError::InvalidArgument(
                    "--max-frequency must be greater than or equal to --min-frequency".to_string(),
                ));
            }
        }

        if let (Some(min_count), Some(max_count)) =
            (self.min_document_count, self.max_document_count)
        {
            if max_count < min_count {
                return Err(RebeError::InvalidArgument(
                    "--max-doc-count must be greater than or equal to --min-doc-count".to_string(),
                ));
            }
        }

        if let (Some(min_frequency), Some(max_frequency)) =
            (self.min_document_frequency, self.max_document_frequency)
        {
            if max_frequency < min_frequency {
                return Err(RebeError::InvalidArgument(
                    "--max-doc-frequency must be greater than or equal to --min-doc-frequency"
                        .to_string(),
                ));
            }
        }

        if self.coverage_target.is_some() && self.sort != SortMode::Frequency {
            return Err(RebeError::InvalidArgument(
                "--coverage requires frequency sorting".to_string(),
            ));
        }

        let definition_provider_count = usize::from(self.define_command.is_some())
            + usize::from(self.define_youdao)
            + usize::from(self.define_mdx_path.is_some());

        if definition_provider_count > 1 {
            return Err(RebeError::InvalidArgument(
                "--define-command, --define-youdao, and --define-mdx cannot be used together"
                    .to_string(),
            ));
        }

        if let Some(command) = &self.define_command {
            DefinitionCommand::new(
                command.clone(),
                self.definition_timeout_ms,
                self.definition_max_chars,
            )?;
        }

        if self.define_youdao {
            YoudaoDefinitionConfig::from_options(
                self.youdao_app_key.clone(),
                self.youdao_app_secret.clone(),
                self.youdao_from.clone(),
                self.youdao_to.clone(),
                self.definition_timeout_ms,
                self.definition_max_chars,
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub input: PathBuf,
    pub source_files: Vec<PathBuf>,
    pub total_words: usize,
    pub unique_words: usize,
    pub candidate_words: usize,
    pub ignored_words: usize,
    pub known_words: usize,
    pub words: Vec<WordStat>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WordStat {
    pub word: String,
    pub forms: Vec<String>,
    pub count: usize,
    pub frequency: f64,
    pub cumulative_coverage: f64,
    pub first_position: usize,
    pub sentence_count: usize,
    pub document_count: usize,
    pub document_frequency: f64,
    pub sources: Vec<WordSourceStat>,
    pub definition: Option<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WordSourceStat {
    pub source: String,
    pub count: usize,
}

#[derive(Debug, Default)]
struct WordAccumulator {
    forms: BTreeMap<String, usize>,
    count: usize,
    first_position: usize,
    sentence_indexes: BTreeSet<usize>,
    document_indexes: BTreeSet<usize>,
    document_counts: BTreeMap<usize, usize>,
    capitalized_non_initial_count: usize,
    lowercase_observation_count: usize,
    examples: Vec<String>,
}

pub fn analyze(config: &AnalysisConfig) -> RebeResult<AnalysisReport> {
    config.validate()?;

    let documents = load_documents(&config.input)?;
    let source_files = documents
        .iter()
        .map(|document| document.path.clone())
        .collect::<Vec<_>>();
    let user_profile = profile::load_user_profile(config.profile_path.as_deref())?;
    let mut lemma_map = user_profile.lemma_map;
    lemma_map.extend(profile::load_lemma_map(config.lemma_map_path.as_deref())?);

    let mut known_words = profile::normalize_word_items(&user_profile.known_words, &lemma_map);
    known_words.extend(profile::load_word_set_with_lemma_map(
        config.known_words_path.as_deref(),
        &lemma_map,
    )?);

    let mut ignored_words = profile::normalize_word_items(&user_profile.ignored_words, &lemma_map);
    ignored_words.extend(profile::load_word_set_with_lemma_map(
        config.ignore_words_path.as_deref(),
        &lemma_map,
    )?);

    if config.ignore_common_words {
        ignored_words.extend(profile::common_word_set(&lemma_map));
    }

    let raw_stats = collect_stats(&documents, config.example_count, &lemma_map);
    let total_words = raw_stats.values().map(|stat| stat.count).sum::<usize>();
    let unique_words = raw_stats.len();
    let mut candidates = build_candidates(
        raw_stats,
        total_words,
        documents.len(),
        &source_files,
        &known_words,
        &ignored_words,
        config,
    );

    sort_words(&mut candidates, config.sort);
    apply_cumulative_coverage(&mut candidates, total_words);
    apply_coverage_target(&mut candidates, config.coverage_target);

    if let Some(top) = config.top {
        candidates.truncate(top);
    }

    apply_definitions(&mut candidates, config)?;

    Ok(AnalysisReport {
        input: config.input.clone(),
        source_files,
        total_words,
        unique_words,
        candidate_words: candidates.len(),
        ignored_words: ignored_words.len(),
        known_words: known_words.len(),
        words: candidates,
    })
}

fn collect_stats(
    documents: &[Document],
    example_count: usize,
    lemma_map: &text::LemmaMap,
) -> BTreeMap<String, WordAccumulator> {
    let mut stats = BTreeMap::<String, WordAccumulator>::new();
    let mut position = 0;
    let mut global_sentence_index = 0;

    for (document_index, document) in documents.iter().enumerate() {
        for sentence in &document.sentences {
            let tokens = text::tokenize_sentence_details_with_lemma_map(sentence, lemma_map);

            for (token_index, token) in tokens.into_iter().enumerate() {
                position += 1;
                let stat = stats.entry(token.normalized).or_default();

                if stat.count == 0 {
                    stat.first_position = position;
                }

                stat.count += 1;
                stat.sentence_indexes.insert(global_sentence_index);
                stat.document_indexes.insert(document_index);
                *stat.document_counts.entry(document_index).or_insert(0) += 1;
                *stat.forms.entry(token.surface).or_insert(0) += 1;

                if token_index > 0 {
                    if token.is_capitalized {
                        stat.capitalized_non_initial_count += 1;
                    } else {
                        stat.lowercase_observation_count += 1;
                    }
                }

                if example_count > 0 && stat.examples.len() < example_count {
                    let example = trim_example(sentence);

                    if !stat.examples.iter().any(|existing| existing == &example) {
                        stat.examples.push(example);
                    }
                }
            }

            global_sentence_index += 1;
        }
    }

    stats
}

fn build_candidates(
    raw_stats: BTreeMap<String, WordAccumulator>,
    total_words: usize,
    total_documents: usize,
    source_files: &[PathBuf],
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
    config: &AnalysisConfig,
) -> Vec<WordStat> {
    raw_stats
        .into_iter()
        .filter(|(word, stat)| {
            should_keep_word(
                word,
                stat,
                total_words,
                total_documents,
                known_words,
                ignored_words,
                config,
            )
        })
        .map(|(word, stat)| {
            let frequency = if total_words == 0 {
                0.0
            } else {
                stat.count as f64 / total_words as f64
            };
            let document_frequency = if total_documents == 0 {
                0.0
            } else {
                stat.document_indexes.len() as f64 / total_documents as f64
            };

            WordStat {
                word,
                forms: sorted_forms(stat.forms),
                count: stat.count,
                frequency,
                cumulative_coverage: 0.0,
                first_position: stat.first_position,
                sentence_count: stat.sentence_indexes.len(),
                document_count: stat.document_indexes.len(),
                document_frequency,
                sources: build_source_stats(stat.document_counts, source_files),
                definition: None,
                examples: stat.examples,
            }
        })
        .collect()
}

fn should_keep_word(
    word: &str,
    stat: &WordAccumulator,
    total_words: usize,
    total_documents: usize,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
    config: &AnalysisConfig,
) -> bool {
    if word.chars().count() < config.min_word_len {
        return false;
    }

    if stat.count < config.min_count {
        return false;
    }

    if config
        .max_count
        .map(|max_count| stat.count > max_count)
        .unwrap_or(false)
    {
        return false;
    }

    let frequency = if total_words == 0 {
        0.0
    } else {
        stat.count as f64 / total_words as f64
    };

    if config
        .min_frequency
        .map(|min_frequency| frequency < min_frequency)
        .unwrap_or(false)
    {
        return false;
    }

    if config
        .max_frequency
        .map(|max_frequency| frequency > max_frequency)
        .unwrap_or(false)
    {
        return false;
    }

    if config
        .min_document_count
        .map(|min_count| stat.document_indexes.len() < min_count)
        .unwrap_or(false)
    {
        return false;
    }

    if config
        .max_document_count
        .map(|max_count| stat.document_indexes.len() > max_count)
        .unwrap_or(false)
    {
        return false;
    }

    let document_frequency = if total_documents == 0 {
        0.0
    } else {
        stat.document_indexes.len() as f64 / total_documents as f64
    };

    if config
        .min_document_frequency
        .map(|min_frequency| document_frequency < min_frequency)
        .unwrap_or(false)
    {
        return false;
    }

    if config
        .max_document_frequency
        .map(|max_frequency| document_frequency > max_frequency)
        .unwrap_or(false)
    {
        return false;
    }

    if config.ignore_proper_nouns && is_probable_proper_noun(stat) {
        return false;
    }

    !known_words.contains(word) && !ignored_words.contains(word)
}

fn sorted_forms(forms: BTreeMap<String, usize>) -> Vec<String> {
    let mut forms = forms.into_iter().collect::<Vec<_>>();
    forms.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    forms.into_iter().map(|(word, _)| word).collect()
}

fn build_source_stats(
    document_counts: BTreeMap<usize, usize>,
    source_files: &[PathBuf],
) -> Vec<WordSourceStat> {
    document_counts
        .into_iter()
        .filter_map(|(document_index, count)| {
            source_files.get(document_index).map(|path| WordSourceStat {
                source: path.display().to_string(),
                count,
            })
        })
        .collect()
}

fn sort_words(words: &mut [WordStat], sort_mode: SortMode) {
    match sort_mode {
        SortMode::Frequency => {
            words.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then_with(|| left.first_position.cmp(&right.first_position))
                    .then_with(|| left.word.cmp(&right.word))
            });
        }
        SortMode::Word => {
            words.sort_by(|left, right| {
                let word_order = left.word.cmp(&right.word);

                if word_order == Ordering::Equal {
                    right.count.cmp(&left.count)
                } else {
                    word_order
                }
            });
        }
    }
}

fn apply_cumulative_coverage(words: &mut [WordStat], total_words: usize) {
    if total_words == 0 {
        return;
    }

    let mut cumulative_count = 0usize;

    for word in words {
        cumulative_count += word.count;
        word.cumulative_coverage = cumulative_count as f64 / total_words as f64;
    }
}

fn apply_coverage_target(words: &mut Vec<WordStat>, coverage_target: Option<f64>) {
    let Some(coverage_target) = coverage_target else {
        return;
    };

    let keep_count = words
        .iter()
        .position(|word| word.cumulative_coverage >= coverage_target)
        .map(|index| index + 1)
        .unwrap_or(words.len());

    words.truncate(keep_count);
}

fn apply_definitions(words: &mut [WordStat], config: &AnalysisConfig) -> RebeResult<()> {
    let Some(mut provider) = definition_provider(config)? else {
        return Ok(());
    };

    for (index, word) in words.iter_mut().enumerate() {
        if config
            .definition_limit
            .map(|limit| index >= limit)
            .unwrap_or(false)
        {
            break;
        }

        word.definition = provider.lookup(&word.word)?;
    }

    Ok(())
}

fn definition_provider(config: &AnalysisConfig) -> RebeResult<Option<DefinitionProvider>> {
    if let Some(command_template) = &config.define_command {
        let command = DefinitionCommand::new(
            command_template.clone(),
            config.definition_timeout_ms,
            config.definition_max_chars,
        )?;
        return Ok(Some(DefinitionProvider::Command(command)));
    }

    if config.define_youdao {
        let youdao_config = YoudaoDefinitionConfig::from_options(
            config.youdao_app_key.clone(),
            config.youdao_app_secret.clone(),
            config.youdao_from.clone(),
            config.youdao_to.clone(),
            config.definition_timeout_ms,
            config.definition_max_chars,
        )?;
        return Ok(Some(DefinitionProvider::Youdao(
            YoudaoDefinitionClient::new(youdao_config),
        )));
    }

    if let Some(path) = &config.define_mdx_path {
        return Ok(Some(DefinitionProvider::Mdx(MdxDefinitionClient::open(
            path,
            config.definition_max_chars,
            config.mdx_definition_format,
        )?)));
    }

    Ok(None)
}

fn is_probable_proper_noun(stat: &WordAccumulator) -> bool {
    stat.capitalized_non_initial_count > 0 && stat.lowercase_observation_count == 0
}

fn trim_example(sentence: &str) -> String {
    let mut chars = sentence.chars();
    let trimmed = chars.by_ref().take(MAX_EXAMPLE_CHARS).collect::<String>();

    if chars.next().is_some() {
        format!("{}...", trimmed.trim_end())
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn analyzes_txt_with_known_word_filter() {
        let input_path = temp_file_path("book", "txt");
        let known_path = temp_file_path("known", "txt");

        fs::write(
            &input_path,
            "Rust helps reading. Reading Rust books helps readers.",
        )
        .expect("write input");
        fs::write(&known_path, "rust\n").expect("write known");

        let config = AnalysisConfig {
            input: input_path.clone(),
            known_words_path: Some(known_path.clone()),
            ignore_common_words: true,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let words = report
            .words
            .iter()
            .map(|stat| (stat.word.as_str(), stat.count))
            .collect::<Vec<_>>();

        assert_eq!(report.total_words, 8);
        assert!(words.contains(&("read", 2)));
        assert!(words.contains(&("help", 2)));
        assert!(!words.iter().any(|(word, _)| *word == "rust"));

        fs::remove_file(input_path).ok();
        fs::remove_file(known_path).ok();
    }

    #[test]
    fn applies_count_range_and_top() {
        let input_path = temp_file_path("range", "txt");
        fs::write(&input_path, "alpha alpha alpha beta beta gamma").expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            min_count: 2,
            max_count: Some(3),
            top: Some(1),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");

        assert_eq!(report.words.len(), 1);
        assert_eq!(report.words[0].word, "alpha");
        assert_eq!(report.words[0].count, 3);

        fs::remove_file(input_path).ok();
    }

    #[test]
    fn analyzes_directory_text_files() {
        let dir_path = temp_dir_path("corpus");
        fs::create_dir(&dir_path).expect("create dir");
        fs::write(dir_path.join("a.txt"), "target alpha alpha.").expect("write first file");
        fs::write(dir_path.join("b.md"), "target beta.").expect("write second file");
        fs::write(dir_path.join("skip.json"), "target ignored.").expect("write ignored file");

        let config = AnalysisConfig {
            input: dir_path.clone(),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze directory");
        let target = report
            .words
            .iter()
            .find(|word| word.word == "target")
            .expect("target word");

        assert_eq!(report.source_files.len(), 2);
        assert_eq!(report.total_words, 5);
        assert_eq!(target.count, 2);
        assert_eq!(target.document_count, 2);
        assert_eq!(target.document_frequency, 1.0);

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn applies_lemma_map_to_text_and_filters() {
        let input_path = temp_file_path("lemma", "txt");
        let lemma_path = temp_file_path("lemma_map", "txt");

        fs::write(&input_path, "Children read. A child reads.").expect("write input");
        fs::write(&lemma_path, "children child\nreads read\n").expect("write lemma map");

        let config = AnalysisConfig {
            input: input_path.clone(),
            lemma_map_path: Some(lemma_path.clone()),
            ignore_common_words: true,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let child = report
            .words
            .iter()
            .find(|word| word.word == "child")
            .expect("child lemma");
        let read = report
            .words
            .iter()
            .find(|word| word.word == "read")
            .expect("read lemma");

        assert_eq!(child.count, 2);
        assert_eq!(read.count, 2);

        fs::remove_file(input_path).ok();
        fs::remove_file(lemma_path).ok();
    }

    #[test]
    fn applies_user_profile_words_and_lemmas() {
        let input_path = temp_file_path("profile_input", "txt");
        let profile_path = temp_file_path("profile", "ini");

        fs::write(&input_path, "Mice gather. Mouse gather. Alice gather.").expect("write input");
        fs::write(
            &profile_path,
            r#"
            [known]
            mice

            [ignore]
            alice

            [lemma]
            mice = mouse
            "#,
        )
        .expect("write profile");

        let config = AnalysisConfig {
            input: input_path.clone(),
            profile_path: Some(profile_path.clone()),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let words = report
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert_eq!(words, vec!["gather"]);
        assert_eq!(report.known_words, 1);
        assert!(!words.contains(&"mouse"));
        assert!(!words.contains(&"alice"));

        fs::remove_file(input_path).ok();
        fs::remove_file(profile_path).ok();
    }

    #[test]
    fn applies_document_count_filters() {
        let dir_path = temp_dir_path("doc_filter");
        fs::create_dir(&dir_path).expect("create dir");
        fs::write(dir_path.join("a.txt"), "target alpha.").expect("write first file");
        fs::write(dir_path.join("b.txt"), "target beta.").expect("write second file");
        fs::write(dir_path.join("c.txt"), "gamma.").expect("write third file");

        let config = AnalysisConfig {
            input: dir_path.clone(),
            min_document_count: Some(2),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");

        assert_eq!(report.words.len(), 1);
        assert_eq!(report.words[0].word, "target");
        assert_eq!(report.words[0].document_count, 2);
        assert!((report.words[0].document_frequency - (2.0 / 3.0)).abs() < f64::EPSILON);

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn applies_frequency_ratio_range() {
        let input_path = temp_file_path("frequency", "txt");
        fs::write(&input_path, "alpha alpha alpha beta beta gamma delta").expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            min_frequency: Some(0.2),
            max_frequency: Some(0.3),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");

        assert_eq!(report.words.len(), 1);
        assert_eq!(report.words[0].word, "beta");

        fs::remove_file(input_path).ok();
    }

    #[test]
    fn truncates_by_coverage_target() {
        let input_path = temp_file_path("coverage", "txt");
        fs::write(&input_path, "alpha alpha alpha beta beta gamma").expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            coverage_target: Some(0.75),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let words = report
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert_eq!(words, vec!["alpha", "beta"]);
        assert!(report.words[1].cumulative_coverage >= 0.75);

        fs::remove_file(input_path).ok();
    }

    #[test]
    fn filters_probable_proper_nouns() {
        let input_path = temp_file_path("proper", "txt");
        fs::write(&input_path, "We meet Alice. We meet Bob. We study ideas.").expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let words = report
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert!(!words.contains(&"alice"));
        assert!(!words.contains(&"bob"));
        assert!(words.contains(&"idea"));

        fs::remove_file(input_path).ok();
    }

    #[test]
    fn attaches_definitions_from_external_command() {
        let input_path = temp_file_path("definition", "txt");
        fs::write(&input_path, "reader reader").expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            define_command: Some("printf 'meaning:%s' {word}".to_string()),
            definition_limit: Some(1),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");

        assert_eq!(report.words.len(), 1);
        assert_eq!(
            report.words[0].definition,
            Some("meaning:reader".to_string())
        );

        fs::remove_file(input_path).ok();
    }

    #[test]
    fn trims_long_examples() {
        let input_path = temp_file_path("long_example", "txt");
        let text = format!("{} vocabulary", "a".repeat(400));
        fs::write(&input_path, text).expect("write input");

        let config = AnalysisConfig {
            input: input_path.clone(),
            ignore_common_words: false,
            ..AnalysisConfig::default()
        };

        let report = analyze(&config).expect("analyze");
        let vocabulary = report
            .words
            .iter()
            .find(|word| word.word == "vocabulary")
            .expect("vocabulary word");

        assert!(vocabulary.examples[0].len() <= MAX_EXAMPLE_CHARS + 3);
        assert!(vocabulary.examples[0].ends_with("..."));

        fs::remove_file(input_path).ok();
    }

    fn temp_file_path(name: &str, extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_{name}_{nanos}.{extension}"))
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_{name}_{nanos}"))
    }
}
