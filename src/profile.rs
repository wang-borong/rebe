use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::{RebeError, RebeResult};
use crate::text;

const COMMON_WORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "am",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "just",
    "me",
    "more",
    "most",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "same",
    "she",
    "should",
    "so",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "would",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

pub const DEFAULT_USER_PROFILE_TEMPLATE: &str = r#"# Rebe user profile.
# Lines beginning with # are comments.

[known]
# reader
# written words

[ignore]
# alice
# bob

[lemma]
# mice = mouse
# went go

[defaults]
# min-count = 2
# format = csv
# definition-max-chars = 600
# define-mdx = /path/to/dictionary.mdx
# mdx-definition-format = plain
"#;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserProfile {
    pub known_words: Vec<String>,
    pub ignored_words: Vec<String>,
    pub lemma_map: text::LemmaMap,
    pub defaults: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
enum ProfileWordSection {
    Known,
    Ignore,
}

impl ProfileWordSection {
    fn name(self) -> &'static str {
        match self {
            Self::Known => "known",
            Self::Ignore => "ignore",
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            Self::Known => "profile add-known",
            Self::Ignore => "profile add-ignore",
        }
    }

    fn words<'a>(self, profile: &'a UserProfile) -> &'a [String] {
        match self {
            Self::Known => &profile.known_words,
            Self::Ignore => &profile.ignored_words,
        }
    }
}

pub fn load_word_set(path: Option<&Path>) -> RebeResult<HashSet<String>> {
    load_word_set_with_lemma_map(path, &text::LemmaMap::new())
}

pub fn load_word_set_with_lemma_map(
    path: Option<&Path>,
    lemma_map: &text::LemmaMap,
) -> RebeResult<HashSet<String>> {
    let mut words = HashSet::new();

    let Some(path) = path else {
        return Ok(words);
    };

    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        let line = line.split('#').next().unwrap_or_default();

        for word in text::tokenize_sentence(line) {
            if let Some(normalized) = text::normalize_word_with_lemma_map(&word, lemma_map) {
                words.insert(normalized);
            }
        }
    }

    Ok(words)
}

pub fn normalize_word_items(words: &[String], lemma_map: &text::LemmaMap) -> HashSet<String> {
    words
        .iter()
        .filter_map(|word| text::normalize_word_with_lemma_map(word, lemma_map))
        .collect()
}

pub fn load_lemma_map(path: Option<&Path>) -> RebeResult<text::LemmaMap> {
    let mut lemma_map = text::LemmaMap::new();

    let Some(path) = path else {
        return Ok(lemma_map);
    };

    let content = fs::read_to_string(path)?;

    for (line_index, line) in content.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();

        if line.is_empty() {
            continue;
        }

        let (surface, lemma) = parse_lemma_pair(line).ok_or_else(|| {
            RebeError::InvalidArgument(format!(
                "invalid lemma map line {}: expected 'surface lemma', 'surface=lemma', or 'surface,lemma'",
                line_index + 1
            ))
        })?;
        let surface = text::clean_word(surface).ok_or_else(|| {
            RebeError::InvalidArgument(format!(
                "invalid lemma map line {}: empty surface word",
                line_index + 1
            ))
        })?;
        let lemma = text::clean_word(lemma).ok_or_else(|| {
            RebeError::InvalidArgument(format!(
                "invalid lemma map line {}: empty lemma word",
                line_index + 1
            ))
        })?;

        lemma_map.insert(surface, lemma);
    }

    Ok(lemma_map)
}

pub fn load_user_profile(path: Option<&Path>) -> RebeResult<UserProfile> {
    let Some(path) = path else {
        return Ok(UserProfile::default());
    };

    let content = fs::read_to_string(path)?;
    parse_user_profile(&content)
}

pub fn init_user_profile(path: &Path, force: bool) -> RebeResult<()> {
    if force {
        fs::write(path, DEFAULT_USER_PROFILE_TEMPLATE)?;
        return Ok(());
    }

    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(DEFAULT_USER_PROFILE_TEMPLATE.as_bytes())?;

    Ok(())
}

pub fn append_known_words(path: &Path, words: &[String]) -> RebeResult<usize> {
    append_profile_words(path, words, ProfileWordSection::Known)
}

pub fn append_ignored_words(path: &Path, words: &[String]) -> RebeResult<usize> {
    append_profile_words(path, words, ProfileWordSection::Ignore)
}

fn append_profile_words(
    path: &Path,
    words: &[String],
    section: ProfileWordSection,
) -> RebeResult<usize> {
    if words.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "{} expects at least one word",
            section.command_name()
        )));
    }

    let content = fs::read_to_string(path)?;
    let profile = parse_user_profile(&content)?;
    let mut profile_words = normalize_word_items(section.words(&profile), &profile.lemma_map);
    let mut words_to_append = Vec::new();

    for value in words {
        for word in text::tokenize_sentence(value) {
            let Some(normalized) = text::normalize_word_with_lemma_map(&word, &profile.lemma_map)
            else {
                continue;
            };

            if profile_words.insert(normalized) {
                words_to_append.push(word);
            }
        }
    }

    if words_to_append.is_empty() {
        return Ok(0);
    }

    let mut file = OpenOptions::new().append(true).open(path)?;

    if !content.ends_with('\n') {
        writeln!(file)?;
    }

    writeln!(file)?;
    writeln!(file, "[{}]", section.name())?;

    for word in &words_to_append {
        writeln!(file, "{word}")?;
    }

    Ok(words_to_append.len())
}

pub fn common_word_set(lemma_map: &text::LemmaMap) -> HashSet<String> {
    COMMON_WORDS
        .iter()
        .filter_map(|word| text::normalize_word_with_lemma_map(word, lemma_map))
        .collect()
}

fn parse_user_profile(content: &str) -> RebeResult<UserProfile> {
    let mut profile = UserProfile::default();
    let mut current_section = None;

    for (line_index, line) in content.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();

        if line.is_empty() {
            continue;
        }

        if let Some(section) = parse_profile_section(line) {
            current_section = Some(section?);
            continue;
        }

        match current_section {
            Some(ProfileSection::Known) => {
                profile.known_words.extend(text::tokenize_sentence(line));
            }
            Some(ProfileSection::Ignore) => {
                profile.ignored_words.extend(text::tokenize_sentence(line));
            }
            Some(ProfileSection::Lemma) => {
                let (surface, lemma) = parse_lemma_pair(line).ok_or_else(|| {
                    RebeError::InvalidArgument(format!(
                        "invalid profile line {}: expected a lemma pair inside [lemma]",
                        line_index + 1
                    ))
                })?;
                let surface = text::clean_word(surface).ok_or_else(|| {
                    RebeError::InvalidArgument(format!(
                        "invalid profile line {}: empty lemma surface word",
                        line_index + 1
                    ))
                })?;
                let lemma = text::clean_word(lemma).ok_or_else(|| {
                    RebeError::InvalidArgument(format!(
                        "invalid profile line {}: empty lemma word",
                        line_index + 1
                    ))
                })?;

                profile.lemma_map.insert(surface, lemma);
            }
            Some(ProfileSection::Defaults) => {
                let (key, value) = parse_default_pair(line).ok_or_else(|| {
                    RebeError::InvalidArgument(format!(
                        "invalid profile line {}: expected 'key = value' inside [defaults]",
                        line_index + 1
                    ))
                })?;
                let key = normalize_default_key(key);

                if key.is_empty() {
                    return Err(RebeError::InvalidArgument(format!(
                        "invalid profile line {}: empty default key",
                        line_index + 1
                    )));
                }

                profile.defaults.insert(key, value.trim().to_string());
            }
            None => {
                return Err(RebeError::InvalidArgument(format!(
                    "invalid profile line {}: expected [known], [ignore], [lemma], or [defaults] section",
                    line_index + 1
                )));
            }
        }
    }

    Ok(profile)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileSection {
    Known,
    Ignore,
    Lemma,
    Defaults,
}

fn parse_profile_section(line: &str) -> Option<RebeResult<ProfileSection>> {
    if !(line.starts_with('[') && line.ends_with(']')) {
        return None;
    }

    let section = line
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .to_ascii_lowercase();
    let parsed = match section.as_str() {
        "known" | "known_words" => Ok(ProfileSection::Known),
        "ignore" | "ignored" | "ignore_words" | "ignored_words" => Ok(ProfileSection::Ignore),
        "lemma" | "lemmas" | "lemma_map" => Ok(ProfileSection::Lemma),
        "defaults" | "default" | "settings" => Ok(ProfileSection::Defaults),
        _ => Err(RebeError::InvalidArgument(format!(
            "unsupported profile section: [{section}]"
        ))),
    };

    Some(parsed)
}

fn parse_default_pair(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;

    Some((key.trim(), value.trim()))
}

fn normalize_default_key(key: &str) -> String {
    key.trim().replace('-', "_").to_ascii_lowercase()
}

fn parse_lemma_pair(line: &str) -> Option<(&str, &str)> {
    for separator in ["=>", "=", ","].iter() {
        if let Some((surface, lemma)) = line.split_once(separator) {
            return Some((surface.trim(), lemma.trim()));
        }
    }

    let mut parts = line.split_whitespace();
    let surface = parts.next()?;
    let lemma = parts.next()?;

    Some((surface, lemma))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_set_contains_normalized_auxiliary_words() {
        let words = common_word_set(&text::LemmaMap::new());
        assert!(words.contains("the"));
        assert!(words.contains("have"));
    }

    #[test]
    fn parses_lemma_pairs() {
        assert_eq!(
            parse_lemma_pair("children child"),
            Some(("children", "child"))
        );
        assert_eq!(parse_lemma_pair("went=go"), Some(("went", "go")));
        assert_eq!(parse_lemma_pair("mice,mouse"), Some(("mice", "mouse")));
    }

    #[test]
    fn parses_user_profile_sections() {
        let profile = parse_user_profile(
            r#"
            # Personal reading profile
            [known]
            reader
            written words

            [ignore]
            Alice, Bob

            [lemma]
            mice = mouse
            went go

            [defaults]
            min-count = 2
            format = csv
            "#,
        )
        .expect("profile");

        assert_eq!(
            profile.known_words,
            vec![
                "reader".to_string(),
                "written".to_string(),
                "words".to_string()
            ]
        );
        assert_eq!(
            profile.ignored_words,
            vec!["alice".to_string(), "bob".to_string()]
        );
        assert_eq!(profile.lemma_map.get("mice"), Some(&"mouse".to_string()));
        assert_eq!(profile.lemma_map.get("went"), Some(&"go".to_string()));
        assert_eq!(profile.defaults.get("min_count"), Some(&"2".to_string()));
        assert_eq!(profile.defaults.get("format"), Some(&"csv".to_string()));
    }

    #[test]
    fn rejects_profile_content_before_section() {
        let err = parse_user_profile("reader\n").expect_err("profile should require sections");
        assert!(err.to_string().contains("expected [known]"));
    }

    #[test]
    fn rejects_unknown_profile_section() {
        let err =
            parse_user_profile("[unknown]\nreader\n").expect_err("unknown section should fail");
        assert!(err.to_string().contains("unsupported profile section"));
    }

    #[test]
    fn rejects_invalid_default_line() {
        let err = parse_user_profile("[defaults]\nmin-count 2\n")
            .expect_err("default line should require equals");
        assert!(err.to_string().contains("expected 'key = value'"));
    }

    #[test]
    fn initializes_profile_without_overwriting() {
        let path = temp_file_path("init_profile", "ini");

        init_user_profile(&path, false).expect("init profile");
        let content = fs::read_to_string(&path).expect("read profile");
        assert!(content.contains("[known]"));
        assert!(content.contains("[defaults]"));

        let err = init_user_profile(&path, false).expect_err("should not overwrite profile");
        assert!(err.to_string().contains("File exists") || err.to_string().contains("exists"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn force_initializes_profile() {
        let path = temp_file_path("force_profile", "ini");
        fs::write(&path, "custom").expect("write existing profile");

        init_user_profile(&path, true).expect("force init profile");
        let content = fs::read_to_string(&path).expect("read profile");
        assert!(content.contains("[lemma]"));
        assert!(!content.contains("custom"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn appends_new_known_words_without_rewriting_profile() {
        let path = temp_file_path("append_known", "ini");
        fs::write(
            &path,
            r#"# Keep this comment.

[known]
reader

[lemma]
mice = mouse
"#,
        )
        .expect("write profile");

        let added = append_known_words(
            &path,
            &[
                "reader".to_string(),
                "mouse".to_string(),
                "finished books".to_string(),
            ],
        )
        .expect("append known");
        let content = fs::read_to_string(&path).expect("read profile");

        assert_eq!(added, 3);
        assert!(content.contains("# Keep this comment."));
        assert!(content.contains("\n[known]\nmouse\nfinished\nbooks\n"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn skips_known_words_when_appending() {
        let path = temp_file_path("append_duplicate", "ini");
        fs::write(
            &path,
            r#"[known]
mice

[lemma]
mice = mouse
"#,
        )
        .expect("write profile");

        let added = append_known_words(&path, &["mouse".to_string()]).expect("append known");

        assert_eq!(added, 0);

        fs::remove_file(path).ok();
    }

    #[test]
    fn appends_new_ignored_words_without_rewriting_profile() {
        let path = temp_file_path("append_ignore", "ini");
        fs::write(
            &path,
            r#"# Keep this comment.

[ignore]
alice

[lemma]
mice = mouse
"#,
        )
        .expect("write profile");

        let added = append_ignored_words(
            &path,
            &[
                "Alice".to_string(),
                "mouse".to_string(),
                "project terms".to_string(),
            ],
        )
        .expect("append ignored words");
        let content = fs::read_to_string(&path).expect("read profile");

        assert_eq!(added, 3);
        assert!(content.contains("# Keep this comment."));
        assert!(content.contains("\n[ignore]\nmouse\nproject\nterms\n"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn skips_ignored_words_when_appending() {
        let path = temp_file_path("append_ignore_duplicate", "ini");
        fs::write(
            &path,
            r#"[ignore]
mice

[lemma]
mice = mouse
"#,
        )
        .expect("write profile");

        let added =
            append_ignored_words(&path, &["mouse".to_string()]).expect("append ignored words");

        assert_eq!(added, 0);

        fs::remove_file(path).ok();
    }

    fn temp_file_path(name: &str, extension: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_profile_{name}_{nanos}.{extension}"))
    }
}
