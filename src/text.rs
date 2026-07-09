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
    let mut words = Vec::new();
    let mut current = String::new();
    let chars = sentence.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_ascii_alphabetic() {
            current.push(ch.to_ascii_lowercase());
        } else if ch == '\'' && is_apostrophe_inside_word(&chars, index, &current) {
            current.push(ch);
        } else {
            push_word(&mut words, &mut current);
        }

        index += 1;
    }

    push_word(&mut words, &mut current);
    words
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
    let cleaned = clean_word(word)?;
    Some(stem_word(&cleaned))
}

fn stem_word(word: &str) -> String {
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

fn push_word(words: &mut Vec<String>, current: &mut String) {
    if let Some(cleaned) = clean_word(current) {
        words.push(cleaned);
    }

    current.clear();
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
}
