//! Python `re` semantics over `fancy_regex` (lookaround, backreferences).
//!
//! Ported patterns keep Python's syntax where the two engines agree. Where they differ, the port rewrites the
//! pattern and says so beside it. The usual case is Python's `$`, which also matches before a final newline; in
//! Rust it is written `(?=\n?\z)`.

use fancy_regex::{Captures, Regex};

/// A compiled pattern with Python-style helpers. A match that hits the backtracking limit counts as no match.
pub struct Re(Regex);

impl Re {
    /// Compiles a pattern; an invalid pattern is a programming error in a port.
    pub fn new(pattern: &str) -> Self {
        Re(Regex::new(pattern).unwrap_or_else(|e| panic!("invalid pattern {pattern:?}: {e}")))
    }

    pub fn inner(&self) -> &Regex {
        &self.0
    }
}
