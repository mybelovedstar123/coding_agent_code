//! Event histories in stored records -> the state they describe at an effective step.
//!
//! Records of an event history state their own rules (events apply by effective step, not document order; an
//! assignment replaces a field's value; add/remove change only the named member of a set; state persists until
//! changed). The events come as tables (JSON lists of objects, CSV, markdown tables with entity / field / value /
//! effective step / operation columns) or as sentences that quote the entity, field and value and name the step. This
//! module reads the events, applies those rules up to a step, and returns every entity's fields at that step, so the
//! deciding model sees the state the records describe next to the records themselves.
//!
//! Python semantics carried here (private helpers, see the report): `json.loads` (NaN/Infinity, arbitrary-precision
//! ints, the 4300-digit int limit), `int()` of a str/float, `str.splitlines()`, the `csv` module's reader state machine
//! with `DictReader`, and Python's `\s`/`\w`/`\b` classes in patterns (`py_pattern`).

