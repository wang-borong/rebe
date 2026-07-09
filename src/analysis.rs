use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use crate::document::Document;
use crate::error::{RebeError, RebeResult};
use crate::export::OutputFormat;
use crate::profile;
use crate::text;

const MAX_EXAMPLE_CHARS: usize = 240;

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
    pub known_words_path: Option<PathBuf>,
    pub ignore_words_path: Option<PathBuf>,
    pub min_count: usize,
    pub max_count: Option<usize>,
    pub top: Option<usize>,
    pub example_count: usize,
    pub min_word_len: usize,
    pub sort: SortMode,
    pub ignore_common_words: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            format: OutputFormat::Txt,
            known_words_path: None,
            ignore_words_path: None,
            min_count: 1,
            max_count: None,
            top: None,
            example_count: 2,
            min_word_len: 1,
            sort: SortMode::Frequency,
            ignore_common_words: true,
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

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub input: PathBuf,
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
    pub examples: Vec<String>,
}

#[derive(Debug, Default)]
struct WordAccumulator {
    forms: BTreeMap<String, usize>,
    count: usize,
    first_position: usize,
    sentence_indexes: BTreeSet<usize>,
    examples: Vec<String>,
}

pub fn analyze(config: &AnalysisConfig) -> RebeResult<AnalysisReport> {
    config.validate()?;

    let document = Document::load_txt(&config.input)?;
    let known_words = profile::load_word_set(config.known_words_path.as_deref())?;
    let mut ignored_words = profile::load_word_set(config.ignore_words_path.as_deref())?;

    if config.ignore_common_words {
        ignored_words.extend(profile::common_word_set());
    }

    let raw_stats = collect_stats(&document, config.example_count);
    let total_words = raw_stats.values().map(|stat| stat.count).sum::<usize>();
    let unique_words = raw_stats.len();
    let mut candidates =
        build_candidates(raw_stats, total_words, &known_words, &ignored_words, config);

    sort_words(&mut candidates, config.sort);
    apply_cumulative_coverage(&mut candidates, total_words);

    if let Some(top) = config.top {
        candidates.truncate(top);
    }

    Ok(AnalysisReport {
        input: document.path,
        total_words,
        unique_words,
        candidate_words: candidates.len(),
        ignored_words: ignored_words.len(),
        known_words: known_words.len(),
        words: candidates,
    })
}

fn collect_stats(document: &Document, example_count: usize) -> BTreeMap<String, WordAccumulator> {
    let mut stats = BTreeMap::<String, WordAccumulator>::new();
    let mut position = 0;

    for (sentence_index, sentence) in document.sentences.iter().enumerate() {
        let sentence_words = text::tokenize_sentence(sentence);
        let normalized_words = sentence_words
            .iter()
            .filter_map(|word| text::normalize_word(word).map(|normalized| (word, normalized)))
            .collect::<Vec<_>>();

        for (surface_word, normalized_word) in normalized_words {
            position += 1;
            let stat = stats.entry(normalized_word).or_default();

            if stat.count == 0 {
                stat.first_position = position;
            }

            stat.count += 1;
            stat.sentence_indexes.insert(sentence_index);
            *stat.forms.entry(surface_word.clone()).or_insert(0) += 1;

            if example_count > 0 && stat.examples.len() < example_count {
                let example = trim_example(sentence);

                if !stat.examples.iter().any(|existing| existing == &example) {
                    stat.examples.push(example);
                }
            }
        }
    }

    stats
}

fn build_candidates(
    raw_stats: BTreeMap<String, WordAccumulator>,
    total_words: usize,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
    config: &AnalysisConfig,
) -> Vec<WordStat> {
    raw_stats
        .into_iter()
        .filter(|(word, stat)| should_keep_word(word, stat, known_words, ignored_words, config))
        .map(|(word, stat)| {
            let frequency = if total_words == 0 {
                0.0
            } else {
                stat.count as f64 / total_words as f64
            };

            WordStat {
                word,
                forms: sorted_forms(stat.forms),
                count: stat.count,
                frequency,
                cumulative_coverage: 0.0,
                first_position: stat.first_position,
                sentence_count: stat.sentence_indexes.len(),
                examples: stat.examples,
            }
        })
        .collect()
}

fn should_keep_word(
    word: &str,
    stat: &WordAccumulator,
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

    !known_words.contains(word) && !ignored_words.contains(word)
}

fn sorted_forms(forms: BTreeMap<String, usize>) -> Vec<String> {
    let mut forms = forms.into_iter().collect::<Vec<_>>();
    forms.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    forms.into_iter().map(|(word, _)| word).collect()
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
}
