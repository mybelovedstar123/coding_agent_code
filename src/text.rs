//! Shared text utilities.

use crate::pyre::Re;
use std::sync::LazyLock;

static TOKEN_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z0-9]+(?:[.'@_-][a-z0-9]+)*"));

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "did", "do", "does", "for", "from", "had", "has", "have",
    "he", "her", "his", "how", "i", "if", "in", "is", "it", "its", "me", "my", "of", "on", "or", "our", "she", "so",
    "that", "the", "their", "them", "they", "this", "to", "was", "we", "were", "what", "when", "where", "which", "who",
    "whom", "why", "will", "with", "you", "your", "yours", "about", "into", "than", "then", "there", "these", "those",
    "can", "could", "would", "should", "just", "also", "am", "been", "being",
];

pub fn is_stopword(t: &str) -> bool {
    STOPWORDS.contains(&t)
}
