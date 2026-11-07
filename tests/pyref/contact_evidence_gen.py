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


def case(mem, category, q, context, spec=None):
    channel = ce.channel_records(mem, q)
    texts = ce.context_texts(context)
    named_ctx = ce.people_named(mem, texts)
    emails = ce.email_records(mem, named_ctx)
    named = ce.possessive_nicknames(mem, q)
    identity = ce.identity_records(mem, named)
    out = {
        "category": category,
        "question": q,
        # A dataset context is rebuilt from its spec (and checked against its digest); an edge context is given.
        "context": context if spec is None else None,
        "context_spec": spec,
        "context_digest": R.digest(context),
        "asks_for_handle": ce.asks_for_handle(q),
        "asks_owner_contact": ce.asks_owner_contact(q),
        "person_name_forms": sorted(ce.person_name_forms(mem, q)),
        "channel_records": R.ids(channel),
        "has_removal_instruction": ce.has_removal_instruction(channel),
        "with_removal_note": R.changed(context, ce.with_removal_note(context, PREFIX)),
        "context_texts": {"digest": R.digest(texts), "value": texts if len(texts) <= 2000 else None},
        "people_named": sorted(named_ctx),
        "people_named_question": sorted(ce.people_named(mem, q)),
        "email_records": R.ids(emails),
        "with_records_email": R.changed(context, ce.with_records(context, PREFIX, emails, render_pair, fmt_ts)),
        "with_records_channel": R.changed(context, ce.with_records(context, PREFIX, channel, render_pair, fmt_ts)),
        "possessive_nicknames": [[k, v] for k, v in named.items()],
        "identity_records": R.ids(identity),
        "related_person_note": ce.related_person_note(named) if named else "",
        "with_records_and_note": R.changed(
            context,
            ce.with_records_and_note(
                context, PREFIX, identity, ce.related_person_note(named) if named else "", render_pair, fmt_ts
            ),
        ),
        "removed_handle_note": ce.removed_handle_note(mem, q),
        "pipeline": R.changed(context, pipeline(mem, q, context), full=True),
    }
    return out


def fires(c):
    return bool(
        c["channel_records"]
        or c["has_removal_instruction"]
        or c["email_records"] and c["asks_owner_contact"]
        or c["possessive_nicknames"]
        or c["removed_handle_note"]
        or "digest" in c["pipeline"]
    )


# ---- a small crafted memory for the branches the datasets reach rarely or never
EDGE_PAIRS = [
    {"pair_id": "e01", "timestamp": "2025-01-02T09:00:00Z", "prompt": "My cousin Olivia Cantu goes by “Ollie” with everyone.", "response": "Noted."},
    {"pair_id": "e02", "timestamp": "2025-01-03T09:00:00Z", "prompt": "Ollie's Signal handle is @ollie.c — use it for quick pings.", "response": "Saved."},
    {"pair_id": "e03", "timestamp": "2025-02-03T09:00:00Z", "prompt": "Please remove Ollie's old handle @ollie.c, she retired it.", "response": "Done."},
    {"pair_id": "e04", "timestamp": "2025-01-05T09:00:00+02:00", "prompt": "Bartholomew Reyes-Vance — everyone calls him \"Bart\".", "response": "Got it."},
    {"pair_id": "e05", "timestamp": "2025-01-06T09:00:00", "prompt": "Bart's hemdle on the team chat is @bart_rv.", "response": "Saved."},
    {"pair_id": "e06", "timestamp": "2025-01-07T09:00:00Z", "prompt": "Bartholomew Reyes-Vance's email is bart.rv@corp.example.io.", "response": "Saved."},
    {"pair_id": "e07", "timestamp": "2025-01-08T09:00:00Z", "prompt": "Samuel Ortiz is “Sam” to his friends.", "response": "Noted."},
    {"pair_id": "e08", "timestamp": "2025-01-09T09:00:00Z", "prompt": "Samantha Lee also goes by “Sam”.", "response": "Noted."},
    {"pair_id": "e09", "timestamp": "2025-01-10T09:00:00Z", "prompt": "Olivia Cantu is also called “Liv” at work.", "response": "Noted."},
    {"pair_id": "e10", "timestamp": "2025-01-11T09:00:00Z", "prompt": "Sam's signal handle: @sam.o. I don’t want it on file after March.", "response": "Okay."},
    {"pair_id": "e11", "timestamp": "2025-01-12T09:00:00Z", "prompt": "Liv's email: liv+work@cantu.example.com", "response": "Saved."},
    {"pair_id": "e12", "timestamp": "2025-01-13T09:00:00Z", "prompt": "Dana Whitfield owns the Harbor project now; she's “Dee”.", "response": "Noted."},
    {"pair_id": "e13", "timestamp": "2025-01-14T09:00:00Z", "prompt": "Dee's address is 14 Elm Row and her e-mail is dee@whit.example.org.", "response": "Saved."},
    {"pair_id": "e14", "timestamp": "", "prompt": "Undated: Bart's handle changed to @bart.new, don’t hand it out.", "response": ""},
    {"pair_id": "e15", "timestamp": "2025-01-14T09:00:00Z", "prompt": "Ollie handle hnale typo: @ollie.c", "response": "  "},
]
