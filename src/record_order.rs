//! Copied notes: a case file whose records hold the same statements several times (one note rendered as a calendar
//! entry, a forwarded text, a list snapshot...). The copies' timestamps say when each copy was filed, not when its
//! event happened, so the order of events has to come from what the records say. Detection is structural
//! (near-equal statements after the identifier); the note is advisory and never names a value.
//!
//! Port of `harness/record_order.py`. Records are (pairID, timestamp text, timestamp, prompt).

