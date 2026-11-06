//! Declared output typography normalization (version 2).
//!
//! A fixed, request-independent normalization applied to `final_text` only:
//!
//! 1. a character table that maps typographic variants some models emit inside identifiers and numbers onto their
//!    plain equivalents;
//! 2. markdown strong-emphasis markers removed from paired spans (`**17 days**` -> `17 days`). The span's text is
//!    kept as written; only the paired markers go. Asterisks that are not a paired span set off from the surrounding
//!    words (an operator, `a**b**c` in code) and underscores of any kind (identifiers) are left alone.
//!
//! It never changes digits, signs, words, units, identifiers' letters, code, or meaningful whitespace, and it does
//! not depend on the request, the answer's meaning, or its length.

use crate::pyre::Re;
use std::sync::LazyLock;

/// A paired strong-emphasis span on one line, not glued to a word on either side, whose text neither starts nor
/// ends with a space.
static STRONG: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?<!\w)\*\*(?=\S)([^*\n]+?)(?<=\S)\*\*(?!\w)"));

fn table(c: char) -> char {
    match c {
        '\u{2010}' | '\u{2011}' => '-',                           // HYPHEN, NON-BREAKING HYPHEN
        '\u{00a0}' | '\u{2007}' | '\u{2009}' | '\u{202f}' => ' ', // NO-BREAK, FIGURE, THIN, NARROW NO-BREAK SPACE
        c => c,
    }
}
