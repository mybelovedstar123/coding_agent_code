//! Case files that define their own field names.
//!
//! Some case files first set out local field conventions ("Local field conventions here: jorailset designates a
//! workstream; lumauset names the accountable owner …", "note that beluxwire is the party that pays us for it; …") and
//! then state facts with those invented names ("For voraenpath the beluxwire on record is Copperfield Systems"). This
//! module reads the definitions from the request's identifier records and returns the glossary plus the records' other
//! statements with each defined name followed by its meaning, so the deciding model reads the facts in plain words.
//!
//! Port of `harness/glossary.py`.

use crate::py;
use crate::pyre::{escape, Re};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::LazyLock;

pub const DEF_VERBS: &str = concat!(
    r"is|are|designates|denotes|names|records|identifies|carries|marks|means|stands for|refers to|",
    r"is the|holds|gives|captures|tracks|labels"
);
pub static DEF_RE: LazyLock<Re> =
    LazyLock::new(|| Re::new(&format!(r"^\s*([a-z][a-z0-9]{{4,}})\s+({DEF_VERBS})\b\s*(.+?)\s*$")));
pub static CLAUSE_SPLIT: LazyLock<Re> =
    LazyLock::new(|| Re::new(&format!(r"[;]|\.\s+|,\s+(?=[a-z][a-z0-9]{{4,}}\s+(?:{DEF_VERBS})\b)")));
pub static INTRO_RE: LazyLock<Re> = LazyLock::new(|| {
    Re::new(concat!(
        r"(?i)(?:conventions here|glossary|note that|field meanings|schema)[^:]*:\s*|before the data lands,?\s*",
        r"note that\s*"
    ))
});
static SCOPE_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\s*(?:within|for|in|under)\b"));
static NON_WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\W+"));
pub const MAX_RECORD_CHARS: usize = 15_000;
pub const GLOSSARY_NOTE: &str = concat!(
    "glossary: the field names these records define for themselves, with their meanings as the records state them; ",
    "glossed_statements are the records' other statements with each defined name followed by its meaning in ",
    "brackets, a host-side annotation of the records: where it differs from the records, the records decide. Answer ",
    "with the value a statement gives for the field asked about, never with the field's own name."
);
