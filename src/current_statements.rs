//! Change statements in a case file and the earlier statements they supersede.
//!
//! A user's notes about one thing are often filed several times (copies, typos), and a later note says that something
//! changed ("Update — …", "X has taken over …", "Rescheduled: … is now … instead", "Revised list — … is off it, … is on
//! it", "Changed my mind …"). Copies are refiled, so filing dates do not show which note came last; the change note
//! itself says it replaces the earlier value. This module groups the notes of the request's identifier records (copies
//! by text similarity), finds the notes that state a change on the request's topic, and lists each with the earlier
//! plain notes of that topic it supersedes, so the deciding model sees which statement is current.
//!
//! Port of `harness/current_statements.py`.

