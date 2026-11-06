//! Query-focused excerpts of long memory records.
//!
//! Real conversation records are often long and mostly irrelevant to any one question. Instead of truncating a
//! record by position (which can cut away the one sentence that matters), keep the sentences that match the query
//! terms — tolerating typos — plus their immediate neighbours, in original order.

