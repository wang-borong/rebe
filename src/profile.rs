use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::error::RebeResult;
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
    let mut words = HashSet::new();

    let Some(path) = path else {
        return Ok(words);
    };

    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        let line = line.split('#').next().unwrap_or_default();

        for word in text::tokenize_sentence(line) {
            if let Some(normalized) = text::normalize_word(&word) {
                words.insert(normalized);
            }
        }
    }

    Ok(words)
}

pub fn common_word_set() -> HashSet<String> {
    COMMON_WORDS
        .iter()
        .filter_map(|word| text::normalize_word(word))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_set_contains_normalized_auxiliary_words() {
        let words = common_word_set();
        assert!(words.contains("the"));
        assert!(words.contains("have"));
    }
}
