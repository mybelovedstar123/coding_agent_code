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

    /// `re.search(p, s) is not None`.
    pub fn is_match(&self, s: &str) -> bool {
        self.0.is_match(s).unwrap_or(false)
    }

    /// `re.search(p, s)` as (start, end) byte offsets.
    pub fn search(&self, s: &str) -> Option<(usize, usize)> {
        self.0.find(s).ok().flatten().map(|m| (m.start(), m.end()))
    }

    /// `re.search(p, s)` as the matched text.
    pub fn search_str<'a>(&self, s: &'a str) -> Option<&'a str> {
        self.0.find(s).ok().flatten().map(|m| m.as_str())
    }

    /// `re.search(p, s)` with its groups.
    pub fn captures<'a>(&self, s: &'a str) -> Option<Captures<'a>> {
        self.0.captures(s).ok().flatten()
    }

    /// `re.match(p, s)`: a match that starts at offset 0.
    pub fn match_start<'a>(&self, s: &'a str) -> Option<Captures<'a>> {
        self.captures_iter(s)
            .into_iter()
            .next()
            .filter(|c| c.get(0).map(|m| m.start()) == Some(0))
    }

    /// Every non-overlapping match, with its groups.
    pub fn captures_iter<'a>(&self, s: &'a str) -> Vec<Captures<'a>> {
        self.0.captures_iter(s).filter_map(|c| c.ok()).collect()
    }

    /// Every non-overlapping match as (start, end, text).
    pub fn find_iter<'a>(&self, s: &'a str) -> Vec<(usize, usize, &'a str)> {
        self.0
            .find_iter(s)
            .filter_map(|m| m.ok())
            .map(|m| (m.start(), m.end(), m.as_str()))
            .collect()
    }

    /// `re.findall(p, s)` for a pattern with no group (whole matches) or one group (that group's text, empty when
    /// it did not take part).
    pub fn findall(&self, s: &str) -> Vec<String> {
        let groups = self.0.captures_len() - 1;
        self.captures_iter(s)
            .iter()
            .map(|c| {
                let idx = if groups == 0 { 0 } else { 1 };
                c.get(idx).map(|m| m.as_str().to_string()).unwrap_or_default()
            })
            .collect()
    }

    /// `re.findall(p, s)` for a pattern with several groups: one tuple of group texts per match.
    pub fn findall_groups(&self, s: &str) -> Vec<Vec<String>> {
        let groups = self.0.captures_len() - 1;
        self.captures_iter(s)
            .iter()
            .map(|c| {
                (1..=groups)
                    .map(|g| c.get(g).map(|m| m.as_str().to_string()).unwrap_or_default())
                    .collect()
            })
            .collect()
    }

    /// `re.sub(p, f, s)`: every match replaced by `f(match)`.
    pub fn sub_with(&self, s: &str, mut f: impl FnMut(&Captures) -> String) -> String {
        let mut out = String::with_capacity(s.len());
        let mut last = 0;
        for c in self.captures_iter(s) {
            let m = c.get(0).expect("group 0");
            out.push_str(&s[last..m.start()]);
            out.push_str(&f(&c));
            last = m.end();
        }
        out.push_str(&s[last..]);
        out
    }

    /// `re.sub(p, literal, s)`: every match replaced by a fixed string (no group references).
    pub fn sub(&self, s: &str, replacement: &str) -> String {
        self.sub_with(s, |_| replacement.to_string())
    }

    /// `re.split(p, s)` for a pattern without groups: the pieces between matches. Python never splits on an
    /// empty match at the very start (3.7+ splits on empty matches elsewhere); patterns ported here never match
    /// empty, so empty matches are skipped.
    pub fn split<'a>(&self, s: &'a str) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut last = 0;
        for (start, end, _) in self.find_iter(s) {
            if end == start {
                continue;
            }
            out.push(&s[last..start]);
            last = end;
        }
        out.push(&s[last..]);
        out
    }
}

/// `re.escape(s)` for use inside a pattern.
pub fn escape(s: &str) -> String {
    fancy_regex::escape(s).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_helpers() {
        let re = Re::new(r"(?<=[.!?;])\s+(?=[A-Z0-9])|\n+");
        assert_eq!(re.split("One. Two\n\nthree"), vec!["One.", "Two", "three"]);
        assert_eq!(
            Re::new(r"\b(\w)(\w)").findall_groups("ab cd"),
            vec![vec!["a", "b"], vec!["c", "d"]]
        );
        assert_eq!(Re::new(r"a(b)?").findall("a ab"), vec!["", "b"]);
        assert_eq!(
            Re::new(r"\d+").sub_with("a1b22", |c| format!("<{}>", &c[0])),
            "a<1>b<22>"
        );
    }
}
