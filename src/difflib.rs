//! `difflib.SequenceMatcher(None, a, b).ratio()` over code points: the one port, shared by every module that
//! fuzzy-matches words or phrases (ownership, tool hints, current statements).
//!
//! isjunk is None and autojunk is on, as in the Python reference: a `b` of 200+ elements drops the elements that
//! occur more than `len(b) // 100 + 1` times from the match index (they still extend a match).

use std::collections::HashMap;

/// `difflib.SequenceMatcher(None, a, b)` over the code points of two strings (isjunk=None, autojunk=True).
struct SequenceMatcher {
    a: Vec<char>,
    b: Vec<char>,
    /// b2j minus the "popular" elements (autojunk); there is no junk with isjunk=None.
    b2j: HashMap<char, Vec<usize>>,
}

impl SequenceMatcher {
    fn new(a: &str, b: &str) -> Self {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        // __chain_b
        let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
        for (i, &elt) in b.iter().enumerate() {
            b2j.entry(elt).or_default().push(i);
        }
        let n = b.len();
        if n >= 200 {
            let ntest = n / 100 + 1;
            b2j.retain(|_, idxs| idxs.len() <= ntest);
        }
        SequenceMatcher { a, b, b2j }
    }
}
