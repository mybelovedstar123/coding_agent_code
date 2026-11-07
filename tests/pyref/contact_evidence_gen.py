"""Fixtures for src/contact_evidence.rs from harness/contact_evidence.py (the reference).

For every request of every v13-full dataset (and a set of synthetic edge requests over a small crafted memory), the
expected output of every public function, plus the agent's contact add-on chain (agent._memory_context order) over a
context built the way the agent builds it. Writes tests/fixtures/contact_evidence.json."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ce_records as R  # noqa: E402

from harness import contact_evidence as ce  # noqa: E402
from harness.store import fmt_ts, render_pair  # noqa: E402

PREFIX = R.PREFIX


def attempt(fn):
    """{"value": result} or {"raises": exception type} (the Rust port documents its fallback for these)."""
    try:
        return {"value": fn()}
    except Exception as err:  # noqa: BLE001
        return {"raises": type(err).__name__}


def pipeline(mem, q, context):
    """The contact add-ons in agent._memory_context order."""
    if ce.has_removal_instruction(ce.channel_records(mem, q)):
        context = ce.with_removal_note(context, PREFIX)
    if ce.asks_owner_contact(q):
        forms = ce.people_named(mem, ce.context_texts(context))
        records = ce.email_records(mem, forms) if forms else []
        if records:
            context = ce.with_records(context, PREFIX, records, render_pair, fmt_ts)
    named = ce.possessive_nicknames(mem, q)
    records = ce.identity_records(mem, named) if named else []
    if records:
        context = ce.with_records_and_note(
            context, PREFIX, records, ce.related_person_note(named), render_pair, fmt_ts
        )
    note = ce.removed_handle_note(mem, q)
    if note:
        context = ce.with_records_and_note(context, PREFIX, [], note, render_pair, fmt_ts)
    return context
