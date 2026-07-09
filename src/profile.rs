use std::collections::HashSet;
use std::fs;
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserProfile {
    pub known_words: Vec<String>,
    pub ignored_words: Vec<String>,
    pub lemma_map: text::LemmaMap,
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
            None => {
                return Err(RebeError::InvalidArgument(format!(
                    "invalid profile line {}: expected [known], [ignore], or [lemma] section",
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
        _ => Err(RebeError::InvalidArgument(format!(
            "unsupported profile section: [{section}]"
        ))),
    };

    Some(parsed)
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
}
