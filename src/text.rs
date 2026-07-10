use std::collections::HashMap;
use std::sync::OnceLock;

use wordnet_lemmatizer::{Lemmatizer, Pos};

pub type LemmaMap = HashMap<String, String>;

static WORDNET_LEMMATIZER: OnceLock<Lemmatizer> = OnceLock::new();

const NOUN_FIRST_POSITIONS: [Pos; 4] = [Pos::Noun, Pos::Verb, Pos::Adj, Pos::Adv];
const VERB_FIRST_POSITIONS: [Pos; 4] = [Pos::Verb, Pos::Noun, Pos::Adj, Pos::Adv];
const ADJECTIVE_FIRST_POSITIONS: [Pos; 4] = [Pos::Adj, Pos::Noun, Pos::Verb, Pos::Adv];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub surface: String,
    pub normalized: String,
    pub is_capitalized: bool,
}

pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut previous_was_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !previous_was_space && !current.is_empty() {
                current.push(' ');
            }
            previous_was_space = true;
            continue;
        }

        previous_was_space = false;
        current.push(ch);

        if matches!(ch, '.' | '!' | '?' | ';') {
            push_sentence(&mut sentences, &mut current);
            previous_was_space = false;
        }
    }

    push_sentence(&mut sentences, &mut current);
    sentences
}

pub fn tokenize_sentence(sentence: &str) -> Vec<String> {
    tokenize_sentence_details(sentence)
        .into_iter()
        .map(|token| token.surface)
        .collect()
}

pub fn tokenize_sentence_details(sentence: &str) -> Vec<Token> {
    tokenize_sentence_details_with_lemma_map(sentence, &LemmaMap::new())
}

pub fn tokenize_sentence_details_with_lemma_map(
    sentence: &str,
    lemma_map: &LemmaMap,
) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars = sentence.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_ascii_alphabetic()
            || (ch == '\'' && is_apostrophe_inside_word(&chars, index, &current))
        {
            current.push(ch);
        } else {
            push_token(&mut tokens, &mut current, lemma_map);
        }

        index += 1;
    }

    push_token(&mut tokens, &mut current, lemma_map);
    tokens
}

pub fn clean_word(word: &str) -> Option<String> {
    let mut cleaned = word
        .trim_matches(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
        .to_ascii_lowercase();

    if cleaned.ends_with("'s") && cleaned.len() > 2 {
        cleaned.truncate(cleaned.len() - 2);
    }

    cleaned = cleaned.trim_matches('\'').to_string();

    if cleaned.chars().any(|ch| ch.is_ascii_alphabetic()) {
        Some(cleaned)
    } else {
        None
    }
}

pub fn normalize_word(word: &str) -> Option<String> {
    normalize_word_with_lemma_map(word, &LemmaMap::new())
}

pub fn normalize_word_with_lemma_map(word: &str, lemma_map: &LemmaMap) -> Option<String> {
    let cleaned = clean_word(word)?;

    if let Some(lemma) = lemma_map.get(&cleaned) {
        return Some(lemma.clone());
    }

    Some(lemmatize_word(&cleaned))
}

fn lemmatize_word(word: &str) -> String {
    for pos in preferred_parts_of_speech(word) {
        let lemma = wordnet_lemmatizer().lemmatize(word, *pos);

        if lemma != word {
            return lemma;
        }
    }

    stem_unknown_word(word)
}

fn wordnet_lemmatizer() -> &'static Lemmatizer {
    WORDNET_LEMMATIZER.get_or_init(Lemmatizer::embedded)
}

fn preferred_parts_of_speech(word: &str) -> &'static [Pos] {
    if word.ends_with("ing") || word.ends_with("ed") || word.ends_with("ies") {
        return &VERB_FIRST_POSITIONS;
    }

    if word.ends_with("er")
        || word.ends_with("est")
        || matches!(word, "best" | "better" | "worse" | "worst")
    {
        return &ADJECTIVE_FIRST_POSITIONS;
    }

    &NOUN_FIRST_POSITIONS
}

fn stem_unknown_word(word: &str) -> String {
    if word.len() > 4 && word.ends_with("ies") {
        let mut stem = word[..word.len() - 3].to_string();
        stem.push('y');
        return stem;
    }

    if word.len() > 5 && word.ends_with("ing") {
        return trim_double_consonant(&word[..word.len() - 3]);
    }

    if word.len() > 4 && word.ends_with("ed") {
        return trim_double_consonant(&word[..word.len() - 2]);
    }

    if word.len() > 4 && word.ends_with("es") {
        return word[..word.len() - 2].to_string();
    }

    if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        return word[..word.len() - 1].to_string();
    }

    word.to_string()
}

fn trim_double_consonant(stem: &str) -> String {
    let mut chars = stem.chars().collect::<Vec<_>>();

    if chars.len() < 2 {
        return stem.to_string();
    }

    let last = chars[chars.len() - 1];
    let previous = chars[chars.len() - 2];

    if last == previous && is_ascii_consonant(last) {
        chars.pop();
        return chars.into_iter().collect();
    }

    stem.to_string()
}

fn is_ascii_consonant(ch: char) -> bool {
    ch.is_ascii_alphabetic() && !matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u')
}

fn is_apostrophe_inside_word(chars: &[char], index: usize, current: &str) -> bool {
    !current.is_empty()
        && chars
            .get(index + 1)
            .map(|next| next.is_ascii_alphabetic())
            .unwrap_or(false)
}

fn push_sentence(sentences: &mut Vec<String>, current: &mut String) {
    let sentence = current.trim();

    if !sentence.is_empty() {
        sentences.push(sentence.to_string());
    }

    current.clear();
}

fn push_token(tokens: &mut Vec<Token>, current: &mut String, lemma_map: &LemmaMap) {
    if let Some(surface) = clean_word(current) {
        if let Some(normalized) = normalize_word_with_lemma_map(&surface, lemma_map) {
            tokens.push(Token {
                surface,
                normalized,
                is_capitalized: starts_with_uppercase(current),
            });
        }
    }

    current.clear();
}

fn starts_with_uppercase(word: &str) -> bool {
    word.chars()
        .find(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_words_and_apostrophes() {
        let words = tokenize_sentence("Alice's well-known readers are reading books.");
        assert_eq!(
            words,
            vec!["alice", "well", "known", "readers", "are", "reading", "books"]
        );
    }

    #[test]
    fn normalizes_common_suffixes() {
        assert_eq!(normalize_word("readers"), Some("reader".to_string()));
        assert_eq!(normalize_word("reading"), Some("read".to_string()));
        assert_eq!(normalize_word("studies"), Some("study".to_string()));
        assert_eq!(normalize_word("stopped"), Some("stop".to_string()));
    }

    #[test]
    fn lemmatizes_wordnet_irregular_forms() {
        assert_eq!(normalize_word("children"), Some("child".to_string()));
        assert_eq!(normalize_word("geese"), Some("goose".to_string()));
        assert_eq!(normalize_word("mice"), Some("mouse".to_string()));
        assert_eq!(normalize_word("ran"), Some("run".to_string()));
        assert_eq!(normalize_word("went"), Some("go".to_string()));
        assert_eq!(normalize_word("better"), Some("good".to_string()));
    }

    #[test]
    fn falls_back_to_suffix_normalization_for_unknown_words() {
        assert_eq!(normalize_word("foobars"), Some("foobar".to_string()));
    }

    #[test]
    fn token_details_preserve_capitalization_signal() {
        let tokens = tokenize_sentence_details("We met Alice and bob.");

        assert_eq!(tokens[2].surface, "alice");
        assert!(tokens[2].is_capitalized);
        assert_eq!(tokens[4].surface, "bob");
        assert!(!tokens[4].is_capitalized);
    }

    #[test]
    fn lemma_map_overrides_default_normalization() {
        let mut lemma_map = LemmaMap::new();
        lemma_map.insert("mice".to_string(), "rodent".to_string());

        assert_eq!(
            normalize_word_with_lemma_map("mice", &lemma_map),
            Some("rodent".to_string())
        );
    }
}
