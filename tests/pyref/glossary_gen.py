"""Fixtures for src/glossary.rs from harness/glossary.py (the reference).

Cases:
- every request of every v13-full dataset that names an identifier the records carry (agent.request_identifier):
  the identifier records as (pairID, prompt) items, the agent-built context, and every function's output;
- every identifier the records carry (IDENT_RE over the records, lower-cased), as if a request named it alone, over a
  minimal context: the sweep that reaches every glossary-defining case file;
- synthetic edge texts and contexts.
Writes tests/fixtures/glossary.json."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ce_records as R  # noqa: E402

from harness import glossary as g  # noqa: E402

PREFIX = R.PREFIX
# Identifiers carried by more records than this are not case files (the sweep skips them).
SWEEP_MAX = 40
MINIMAL = PREFIX + "\n" + '{"note": "Stored records.", "memories": []}'


def outputs(items, ident, context, by_id=None):
    by_id = by_id or {}
    terms = g.glossary(items, ident)
    out = {
        "ident": ident,
        # Items are pair ids for dataset records (the test reads the prompt from the memory), else [pid, prompt].
        "items": [pid if pid in by_id and by_id[pid] == prompt else [pid, prompt] for pid, prompt in items],
        # Every statement, as one digest of the statements joined by NUL; the definitions only where there are any.
        "statements_digest": R.digest("\0".join(g.statement(p, ident) for _, p in items)),
        "definitions": [
            [i, [[k, v] for k, v in d.items()]]
            for i, d in enumerate(g.definitions(g.statement(p, ident)) for _, p in items)
            if d
        ],
        "glossary": [[k, v] for k, v in terms.items()],
        "glossed_statements": g.glossed_statements(items, ident, terms),
        "with_glossary": R.changed(context, g.with_glossary(context, PREFIX, items, ident), full=True),
    }
    return out


def fires(c):
    return "digest" in c["with_glossary"]


EDGE_TEXTS = [
    "Local field conventions here: jorailset designates a workstream; lumauset names the accountable owner.",
    "Before the data lands, note that aroilpath records the pending action; eriynnode names the person, "
    "voraenpath is the project.",
    "note that beluxwire is the party that pays us for it; quindle0 stands for the invoice total.",
    "Within this file, alphaone is the first; betatwo means the second; gammathree refers to the third.",
    "For the record, alphaone is the first; betatwo means the second.",
    "In short alphaone is x, betatwo holds y, gammathree tracks z",
    "alphaone is x; betatwo holds y; gammathree tracks z.",
    "alphaone is x; betatwo holds y",
    "Glossary: Alphaone is x; betatwo holds y; gammathree tracks z",
    "Schema: alphaone is x\nbetatwo holds y; gammathree tracks: z.",
    "Field meanings: alphaone is the first, betatwo designates the second, gammathree marks the third.",
    "field meanings — alphaone is; betatwo is ",
    "under the hood: alphaone is the first; alphaone is again; betatwo labels second",
    "For voraenpath the beluxwire on record is Copperfield Systems",
    "Conventions here: x: alphaone is the first: really; betatwo captures é—ü stuff.",
    "",
    "   ",
    "note that",
]
EDGE_GLOSSED = [
    ("For voraenpath the beluxwire on record is Copperfield Systems; beluxwire2 is not.", {"beluxwire": "is the payer", "voraenpath": "names the project"}),
    ("alphaone alphaonex alphaone. betatwo", {"alphaone": "is x", "alphaonex": "is longer", "betatwo": "holds y"}),
    ("nothing here", {"alphaone": "is x"}),
]
