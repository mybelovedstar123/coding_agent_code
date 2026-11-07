"""Fixtures for src/current_statements.rs from harness/current_statements.py (the reference).

Cases:
- every request of every v13-full dataset that names an identifier the records carry (agent.request_identifier):
  the identifier records as (pairID, prompt) items, the agent-built context and every function's output;
- every identifier the records carry (IDENT_RE over the records, lower-cased; at most SWEEP_MAX records), with the
  request set to the records' own statements (so every change note is on topic) over a minimal context;
- difflib ratios over record statements (pairs, typo'd copies, 200+ character texts for the autojunk rule), the
  fuzzy cue matcher on typo'd cues, and synthetic list changes.
Writes tests/fixtures/current_statements.json."""

import difflib
import os
import random
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ce_records as R  # noqa: E402

from harness import current_statements as cs  # noqa: E402

PREFIX = R.PREFIX
SWEEP_MAX = 40
MINIMAL = PREFIX + "\n" + '{"note": "Stored records.", "memories": []}'


def outputs(items, ident, request, context, by_id):
    """Every function's output. The direct calls are made only on short identifier records (the agent reaches them
    only through with_current_statements, which skips records over MAX_RECORD_CHARS); long ones keep null there."""
    short = sum(len(p) for _, p in items) <= cs.MAX_RECORD_CHARS
    notes = cs.notes_of(items, ident) if short else []
    return {
        "ident": ident,
        "request": request,
        "items": [pid if by_id.get(pid) == prompt else [pid, prompt] for pid, prompt in items],
        "words": sorted(cs.words(request)),
        "notes": [
            {
                "text": n["text"],
                "records": n["records"],
                "is_change": cs.is_change(n["text"]),
                "is_aside": cs.is_aside(n["text"]),
                "list_items": cs.list_items(n["text"]),
                "words": sorted(cs.words(n["text"])),
            }
            for n in notes
        ],
        "short": short,
        "current_statements": cs.current_statements(items, ident, request) if short else None,
        "with_current_statements": R.changed(
            context, cs.with_current_statements(context, PREFIX, items, ident, request), full=True
        ),
    }


def fires(c):
    return "digest" in c["with_current_statements"]


def typo(rng, text):
    """One random edit (swap, drop, double or replace a letter)."""
    if len(text) < 2:
        return text + "x"
    i = rng.randrange(len(text) - 1)
    op = rng.randrange(4)
    if op == 0:
        return text[:i] + text[i + 1] + text[i] + text[i + 2 :]
    if op == 1:
        return text[:i] + text[i + 1 :]
    if op == 2:
        return text[:i] + text[i] + text[i:]
    return text[:i] + rng.choice("abcdefghijklmnopqrstuvwxyz ") + text[i + 1 :]


def ratios(statements, rng):
    out = []
    texts = [t.lower() for t in statements if t]
    for _ in range(600):
        a, b = rng.choice(texts), rng.choice(texts)
        out.append((a, b))
    for t in rng.sample(texts, min(300, len(texts))):
        u = t
        for _ in range(rng.randrange(1, 6)):
            u = typo(rng, u)
        out.append((t, u))
        out.append((u[: rng.randrange(len(u) + 1)], t))
    out += [("", ""), ("", "a"), ("a", ""), ("abc", "abc"), ("é—x", "e—x"), ("a" * 250, "a" * 249 + "b"),
            (" ".join(["aa bb"] * 60), " ".join(["aa bc"] * 60)), ("x" * 199 + "y", "x" * 200)]
    return [{"a": a, "b": b, "ratio": difflib.SequenceMatcher(None, a, b).ratio()} for a, b in out]


def phrases(rng, statements):
    cues = [c for c in cs.CHANGE_CUES] + ["separately", "unrelated"]
    out = []
    for cue in cues:
        variants = [cue, typo(rng, cue), typo(rng, typo(rng, cue)), cue.upper(), cue[1:]]
        for v in variants:
            for ctx in (f"so {v} now", f"{v}", f"x{v}", f"{v}s here", "nothing"):
                low = ctx.lower()
                out.append({"low": low, "cue": cue, "value": cs.has_phrase(low, cue)})
    for t in rng.sample(statements, min(200, len(statements))):
        low = t.lower()
        cue = rng.choice(cues)
        out.append({"low": low, "cue": cue, "value": cs.has_phrase(low, cue)})
    return out


LIST_CASES = [
    ("Shopping list: milk, eggs, bread and butter.", "Revised list — eggs is off it, cheese is on it."),
    ("Packing needs: tent, stove, rope", "Update: added a lantern and the stove is gone"),
    ("Guests — Ann, Bob, Cy, Di", "Changed my mind: plus some Eve. Bob no longer coming"),
    ("Menu wants: soup, salad and pie.", "now serving the cake, salad dropped."),
    ("Agenda: budget, hiring, roadmap.", "update — roadmap is on it, budget is on it"),
    ("No list here", "added stuff"),
    ("Kit: a, b", "a and b are on it"),
]
