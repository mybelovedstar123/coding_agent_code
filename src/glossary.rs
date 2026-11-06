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
