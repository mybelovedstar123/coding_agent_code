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

    /// `find_longest_match(alo, ahi, blo, bhi)` as (i, j, k). Popular elements are not in b2j but are not junk, so
    /// the extension loops may still extend over them (the junk-only loops are no-ops without junk).
    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (a, b) = (&self.a, &self.b);
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        // j2len[j + 1] holds the previous row's length ending at b[j]; only the entries listed in `prev_set` are
        // live (the Python dict holds just those keys), the rest are 0 like `j2len.get(j - 1, 0)`.
        let lb = b.len();
        let mut prev = vec![0usize; lb + 1];
        let mut cur = vec![0usize; lb + 1];
        let mut prev_set: Vec<usize> = Vec::new();
        let mut cur_set: Vec<usize> = Vec::new();
        for (i, ai) in a.iter().enumerate().take(ahi).skip(alo) {
            if let Some(js) = self.b2j.get(ai) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = prev[j] + 1;
                    cur[j + 1] = k;
                    cur_set.push(j + 1);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            for &x in &prev_set {
                prev[x] = 0;
            }
            prev_set.clear();
            std::mem::swap(&mut prev, &mut cur);
            std::mem::swap(&mut prev_set, &mut cur_set);
        }
        while besti > alo && bestj > blo && a[besti - 1] == b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && a[besti + bestsize] == b[bestj + bestsize] {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    /// The sum of the sizes of `get_matching_blocks()` (merging adjacent blocks does not change it).
    fn matches(&self) -> usize {
        let mut queue = vec![(0, self.a.len(), 0, self.b.len())];
        let mut total = 0;
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                total += k;
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        total
    }

    /// `ratio()`: `2.0 * matches / length`, 1.0 for two empty sequences.
    fn ratio(&self) -> f64 {
        let length = self.a.len() + self.b.len();
        if length == 0 {
            return 1.0;
        }
        2.0 * self.matches() as f64 / length as f64
    }
}

/// `difflib.SequenceMatcher(None, a, b).ratio()`.
pub fn sequence_ratio(a: &str, b: &str) -> f64 {
    SequenceMatcher::new(a, b).ratio()
}
