use crate::kad::KadId;
use anyhow::Result;
use crate::kad::md4;

// iMule `SearchManager.h::GetInvalidKeywordChars()`
const INVALID_KEYWORD_CHARS: &str = " ()[]{}<>,._-!?:;\\/\"";

/// Split a user query into iMule-style keyword "words".
///
/// Notes (from iMule `CSearchManager::GetWords`):
/// - Split on a fixed set of punctuation/separators.
/// - Lowercase.
/// - Require >= 3 bytes in UTF-8 (so e.g. "ab" is dropped; "猫" is 3 bytes and kept).
pub fn words(query: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut cur = String::new();

    for ch in query.chars() {
        if INVALID_KEYWORD_CHARS.contains(ch) || ch.is_whitespace() {
            push_word(&mut out, &mut cur);
            continue;
        }
        cur.push(ch);
    }
    push_word(&mut out, &mut cur);

    out
}

fn push_word(out: &mut Vec<String>, cur: &mut String) {
    let w = cur.trim();
    if w.is_empty() {
        cur.clear();
        return;
    }

    let w = w.to_lowercase();
    if w.as_bytes().len() >= 3 {
        // iMule removes duplicates by default. We mimic that for stability.
        if !out.iter().any(|x| x == &w) {
            out.push(w);
        }
    }
    cur.clear();
}

/// iMule keyword hash: MD4(UTF-8(keyword_lowercased)).
pub fn keyword_hash(word: &str) -> KadId {
    KadId(md4::digest(word.as_bytes()))
}

/// Convenience: compute the primary keyword hash for a user query.
///
/// iMule uses the first extracted word as the target.
pub fn query_to_keyword_id(query: &str) -> Result<(String, KadId)> {
    let ws = words(query);
    let first = ws
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("query yielded no valid keyword words"))?;
    let id = keyword_hash(&first);
    Ok((first, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words_like_imule() {
        assert_eq!(words("Hello, world!"), vec!["hello".to_string(), "world".to_string()]);
        assert_eq!(words("ab cd"), Vec::<String>::new()); // too short (< 3 bytes)
        assert_eq!(words("猫"), vec!["猫".to_string()]); // 3 bytes in UTF-8
        assert_eq!(words("Test test TEST"), vec!["test".to_string()]);
    }
}
