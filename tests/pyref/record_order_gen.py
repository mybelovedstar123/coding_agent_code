"""Differential fixtures for src/record_order.rs from harness/record_order.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset. Cases
are the agent's call (`copies_note` over a request's identifier records, chosen as agent.request_identifier does) for
every memory_cases question and tool_cases prompt, plus every identifier the records carry, plus crafted copy sets.
Writes tests/fixtures/record_order.json."""

import glob
import json
import os
import random
import re
import sys
from datetime import datetime, timedelta, timezone

sys.dont_write_bytecode = True
SUBMIT = os.environ["DITTO_SUBMIT_DIR"]  # The directory holding build-agent/ and case/datasets/.
sys.path.insert(0, f"{SUBMIT}/build-agent")

from harness import record_order as ro, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "record_order.json")
DATASETS = sorted(glob.glob(f"{SUBMIT}/case/datasets/v13-full-*.json"))
# agent.py
rng = random.Random(118)
IDENT_RE = re.compile(r"\b(?=[A-Za-z0-9-]*\d)(?=[A-Za-z0-9-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9-]{7,}\b")


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    raw = {}
    for w in d["memory_waves"]:
        if w.get("user_id") == "miner":
            for p in w.get("pairs") or []:
                raw[p["pair_id"]] = p
    for t in d["tool_cases"]:
        for p in t.get("prerequisite_pairs") or []:
            raw[p["pair_id"]] = p
    pairs = {
        pid: store.Pair(pid, "", store.parse_ts(p.get("timestamp")), p.get("prompt") or "", p.get("response") or "")
        for pid, p in raw.items()
    }
    requests = [m["question"] for m in d["memory_cases"]] + [t["prompt"] for t in d["tool_cases"]]
    return pairs, list(dict.fromkeys(requests))


def records_for(pairs, ident):
    found = [p for p in pairs.values() if ident in f"{p.prompt}\n{p.response}".lower()]
    return sorted(found, key=store.content_key)


def request_identifier(pairs, user_input):
    """agent.request_identifier."""
    best = ("", [])
    for ident in dict.fromkeys(t.lower() for t in IDENT_RE.findall(user_input)):
        found = [p for p in pairs.values() if ident in f"{p.prompt}\n{p.response}".lower()]
        if found and (not best[1] or len(found) < len(best[1])):
            best = (ident, found)
    return best[0], sorted(best[1], key=store.content_key)


def case(items, ident, table):
    """One copies_note case; the items' records go into `table` (pairID -> [timestamp text, isoformat, prompt])."""
    for pid, stamp, ts, prompt in items:
        table[pid] = [stamp, ts.isoformat(), prompt]
    tup = [(pid, stamp, ts, prompt) for pid, stamp, ts, prompt in items]
    sets = ro.copy_sets(tup, ident)
    re_sets = ro.reordered_sets(tup, ident)
    return {
        "ident": ident,
        "items": [pid for pid, _, _, _ in items],
        "statements": [ro.statement(prompt, ident) for _, _, _, prompt in items],
        "words": [sorted(ro.words(ro.statement(prompt, ident))) for _, _, _, prompt in items],
        "copy_sets": [[pid for pid, _, _ in s] for s in sets],
        "reordered_sets": [[pid for pid, _, _ in s] for s in re_sets],
        "note": ro.copies_note(tup, ident),
    }


def items_of(records):
    return [(p.pair_id, store.fmt_ts(p.ts), p.ts, p.prompt) for p in records]


def crafted():
    base = datetime(2025, 3, 1, 9, 0, tzinfo=timezone.utc)
    st = [
        "the venue deposit was paid to the harbour hall team",
        "the caterer confirmed the tasting menu for forty guests",
        "the florist moved delivery to the friday morning slot",
        "the band signed the contract for the evening set",
    ]

    def rec(pid, minutes, text):
        ts = base + timedelta(minutes=minutes)
        return (pid, store.fmt_ts(ts), ts, text)

    out = []
    ident = "evt-77aa12"
    # Two copy sets whose filing spans interleave: reordered.
    out.append(
        (
            [
                rec("p1", 0, f"Calendar entry EVT-77AA12: {st[0]}\nfiled by calendar"),
                rec("p2", 10, f"Forwarded text evt-77aa12: {st[1]}"),
                rec("p3", 20, f"List snapshot evt-77aa12: {st[0].replace('paid', 'payed')}"),
                rec("p4", 30, f"evt-77aa12: {st[1]} today"),
            ],
            ident,
        )
    )
    # Copy sets that do not overlap in time: no note.
    out.append(
        (
            [
                rec("q1", 0, f"evt-77aa12: {st[0]}"),
                rec("q2", 5, f"evt-77aa12: {st[0]}"),
                rec("q3", 50, f"evt-77aa12: {st[1]}"),
                rec("q4", 60, f"evt-77aa12: {st[1]}"),
            ],
            ident,
        )
    )
    # Equal spans touching at one instant (strict comparison): no overlap.
    out.append(
        (
            [
                rec("r1", 0, f"evt-77aa12: {st[2]}"),
                rec("r2", 10, f"evt-77aa12: {st[2]}"),
                rec("r3", 10, f"evt-77aa12: {st[3]}"),
                rec("r4", 20, f"evt-77aa12: {st[3]}"),
            ],
            ident,
        )
    )
    # Three sets, one nested inside another, one apart; short statements skipped; Jaccard just under / over 0.7.
    out.append(
        (
            [
                rec("s1", 0, f"evt-77aa12: {st[0]}"),
                rec("s2", 5, f"evt-77aa12: {st[1]}"),
                rec("s3", 6, f"evt-77aa12: {st[1]}"),
                rec("s4", 30, f"evt-77aa12: {st[0]}"),
                rec("s5", 100, f"evt-77aa12: {st[2]}"),
                rec("s6", 110, f"evt-77aa12: {st[2]}"),
                rec("s7", 7, "evt-77aa12: too short"),
                rec("s8", 8, "evt-77aa12: too short"),
                rec("s9", 9, "evt-77aa12: alpha bravo charlie delta echo foxtrot golf hotel india juliet"),
                rec("s10", 12, "evt-77aa12: alpha bravo charlie delta echo foxtrot golf kilo lima mike"),
                rec("s11", 3, "evt-77aa12: alpha bravo charlie delta echo foxtrot golf hotel india oscar"),
                rec("s12", 4, "no identifier here: alpha bravo charlie delta echo"),
            ],
            ident,
        )
    )
    # An upper-case identifier argument never matches the lower-cased text.
    out.append(([rec("u1", 0, f"EVT-77AA12: {st[0]}"), rec("u2", 9, f"evt-77aa12: {st[0]}")], "EVT-77AA12"))
    # Non-ASCII around the identifier and in the statement.
    out.append(
        (
            [
                rec("v1", 0, f"Café — évt-77aa12: {st[0]} ünd €5"),
                rec("v2", 1, f"evt-77aa12: {st[1]} — £5 ‘ok’"),
                rec("v3", 2, f"ÉVT-77AA12 evt-77aa12: {st[0]} ünd €5"),
                rec("v4", 3, f"evt-77aa12: {st[1]} — £5 ‘ok’"),
            ],
            ident,
        )
    )
    out.append(([], ident))
    return out


def main():
    sets = []
    totals = {"cases": 0, "notes": 0}
    for path in DATASETS:
        pairs, requests = load(path)
        cases = []
        table = {}
        seen = set()
        for r in requests:
            ident, records = request_identifier(pairs, r)
            if not ident or ident in seen:
                continue
            seen.add(ident)
            cases.append(case(items_of(records), ident, table))
        idents = sorted({t.lower() for p in pairs.values() for t in IDENT_RE.findall(p.prompt)} - seen)
        for ident in idents:
            records = records_for(pairs, ident)
            if len(records) >= 2:
                cases.append(case(items_of(records), ident, table))
        # The cases with copy sets again, their filing times shuffled among the records (other orders, other
        # overlaps); the shuffled records get new pairIDs so the table stays one record per pairID.
        for c in [c for c in cases if c["copy_sets"]]:
            for k in range(6):
                base = [(pid, *table[pid]) for pid in c["items"]]
                stamps = [(st, iso) for _, st, iso, _ in base]
                rng.shuffle(stamps)
                items = [
                    (f"{pid}-v{k}", st, datetime.fromisoformat(iso), prompt)
                    for (pid, _, _, prompt), (st, iso) in zip(base, stamps)
                ]
                cases.append(case(items, c["ident"], table))
        sets.append({"dataset": os.path.basename(path), "records": table, "cases": cases})
        totals["cases"] += len(cases)
        totals["notes"] += sum(1 for c in cases if c["note"])
    table = {}
    crafted_cases = [case(items, ident, table) for items, ident in crafted()]
    sets.append({"dataset": "crafted", "records": table, "cases": crafted_cases})
    totals["cases"] += len(crafted_cases)
    totals["notes"] += sum(1 for c in crafted_cases if c["note"])
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump({"datasets": sets}, f, ensure_ascii=False)
    print(json.dumps(totals))


if __name__ == "__main__":
    main()
