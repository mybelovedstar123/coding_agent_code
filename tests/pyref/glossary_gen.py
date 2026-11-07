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


def edge():
    texts = [{"text": t, "definitions": [[k, v] for k, v in g.definitions(t).items()]} for t in EDGE_TEXTS]
    stmts = [
        {"prompt": p, "ident": i, "statement": g.statement(p, i)}
        for p, i in [
            ('Case file c53a7436ffb5c91e4: "Local field conventions here: x is y"}', "c53a7436ffb5c91e4"),
            ("Case file C53A7436FFB5C91E4:: {note}} ", "c53a7436ffb5c91e4"),
            ("İİİ case c53a7436ffb5c91e4: after", "c53a7436ffb5c91e4"),
            ("no identifier here", "c53a7436ffb5c91e4"),
            ('c53a7436ffb5c91e4', "c53a7436ffb5c91e4"),
            ('"quoted" c53a7436ffb5c91e4 :"  spaced "  ', "c53a7436ffb5c91e4"),
        ]
    ]
    glossed = [{"text": t, "terms": [[k, v] for k, v in terms.items()], "value": g.glossed(t, terms)} for t, terms in EDGE_GLOSSED]
    ident = "case-0001x"
    items = [
        ("p1", f"Case file {ident}: Local field conventions here: jorailset designates a workstream; lumauset names the accountable owner; beluxwire is the payer."),
        ("p2", f"Case file {ident}: For voraenpath the beluxwire on record is Copperfield Systems."),
        ("p3", f"Case file {ident}: For voraenpath the beluxwire on record is Copperfield Systems!"),
        ("p4", f"Case file {ident}: The lumauset for jorailset Atlas is Dana Whitfield."),
        ("p5", f"Case file {ident}: Nothing defined here."),
    ]
    big = items + [("p6", "x" * 15_000)]
    contexts = [
        None,
        "",
        "no newline",
        PREFIX + "\n{bad",
        MINIMAL,
        PREFIX + "\n" + '{"memories": [], "glossary": {"old": "x"}, "note": null}',
        PREFIX + "\n" + '{"note": "é", "extra": 1.50}',
    ]
    cases = [outputs(items, ident, ctx) | {"context": ctx} for ctx in contexts]
    cases.append(outputs(big, ident, MINIMAL) | {"context": MINIMAL})
    cases.append(outputs(items[1:], ident, MINIMAL) | {"context": MINIMAL})
    return {"texts": texts, "statements": stmts, "glossed": glossed, "cases": cases}


def main():
    datasets = []
    totals = {"requests": 0, "requests_firing": 0, "sweep": 0, "sweep_firing": 0}
    for path in R.DATASETS:
        name, mem, flat, requests = R.load(path)
        req_cases = []
        by_id = {pid: p.prompt for pid, p in mem.pairs.items()}
        for category, q in requests:
            ident, ident_records = R.request_identifier(mem, q)
            if not ident:
                continue
            ctx, spec = R.build_context(mem, q, ident_records)
            items = [(p.pair_id, p.prompt) for p in ident_records]
            c = outputs(items, ident, ctx, by_id) | {"category": category, "question": q, "context_spec": spec, "context_digest": R.digest(ctx)}
            req_cases.append(c)
        idents = sorted({t.lower() for p in mem.pairs.values() for t in R.IDENT_RE.findall(f"{p.prompt}\n{p.response}")})
        sweep = []
        for ident in idents:
            found, records = R.request_identifier(mem, ident)
            if found != ident or not records or len(records) > SWEEP_MAX:
                continue
            items = [(p.pair_id, p.prompt) for p in records]
            sweep.append(outputs(items, ident, MINIMAL, by_id) | {"context": MINIMAL})
        totals["requests"] += len(req_cases)
        totals["requests_firing"] += sum(fires(c) for c in req_cases)
        totals["sweep"] += len(sweep)
        totals["sweep_firing"] += sum(fires(c) for c in sweep)
        print(f"{name}: {len(req_cases)} identifier requests ({sum(fires(c) for c in req_cases)} firing), "
              f"{len(sweep)} sweep identifiers ({sum(fires(c) for c in sweep)} firing)", file=sys.stderr)
        datasets.append({"name": name, "user_id": mem.user_id, "records": flat, "requests": req_cases, "sweep": sweep})
    e = edge()
    print(f"edge: {len(e['texts'])} texts, {len(e['cases'])} context cases", file=sys.stderr)
    path = R.write("glossary", {"prefix": PREFIX, "datasets": datasets, "edge": e})
    print(f"wrote {path}: {totals}", file=sys.stderr)
