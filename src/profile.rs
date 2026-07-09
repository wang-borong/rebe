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

pub fn common_word_set(lemma_map: &text::LemmaMap) -> HashSet<String> {
    COMMON_WORDS
        .iter()
        .filter_map(|word| text::normalize_word_with_lemma_map(word, lemma_map))
        .collect()
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
}
