"""Differential fixtures for src/ownership.rs from harness/ownership.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset (in
mem.pairs insertion order: first insertion position, latest value). Requests are memory_cases questions plus
tool_cases prompts, plus synthetic holder questions built from the records' handover sentences so the ownership path
fires often. Writes tests/fixtures/ownership.json."""

import difflib
import glob
import json
import os
import random
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import config, ownership, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "ownership.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
PREFIX = config.MEMORY_CONTEXT_PREFIX
rng = random.Random(118)
# agent.py IDENT_RE: requests naming a case-file identifier never reach with_ownership (the agent calls it only when
# the request has no identifier records), and best_chain over their long event-history records is slow in Python;
# only a sample of them is run through best_chain.
IDENT_RE = re.compile(r"\b(?=[A-Za-z0-9-]*\d)(?=[A-Za-z0-9-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9-]{7,}\b")
IDENT_SAMPLE = 6


def memo_best_chain(idx):
    """best_chain memoised per request (same results; it is a pure function of the index and the request)."""
    orig = idx.best_chain
    cache = {}

    def best_chain(request):
        if request not in cache:
            cache[request] = orig(request)
        return cache[request]

    idx.best_chain = best_chain


class P:
    def __init__(self, d):
        self.pair_id = d["pair_id"]
        self.ts = store.parse_ts(d.get("timestamp"))
        self.prompt = d.get("prompt") or ""
        self.response = d.get("response") or ""


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    pairs = {}
    for w in d["memory_waves"]:
        if w.get("user_id") == "miner":
            for p in w.get("pairs") or []:
                pairs[p["pair_id"]] = P(p)
    for t in d["tool_cases"]:
        for p in t.get("prerequisite_pairs") or []:
            pairs[p["pair_id"]] = P(p)
    requests = [m["question"] for m in d["memory_cases"]] + [t["prompt"] for t in d["tool_cases"]]
    return pairs, list(dict.fromkeys(requests))


def chain_list(ch):
    return [[ts, s] for ts, s in ch]


def make_context(pairs, request, k=8, note=True, cut=None):
    """A retrieved-record context: the k records sharing the most request tokens, oldest first, some excerpted."""
    want = set(ownership.tokens(request)) - ownership.STOP
    scored = []
    for i, (pid, p) in enumerate(pairs.items()):
        score = len(want & set(ownership.tokens(p.prompt)))
        if score:
            scored.append((-score, i, pid))
    scored.sort()
    items = []
    for rank, (_, _, pid) in enumerate(scored[:k]):
        p = pairs[pid]
        text = store.render_pair(p)
        if cut and rank % 2 == 1:
            text = text[:cut] + " …"
        items.append({"pairID": pid, "timestamp": store.fmt_ts(p.ts), "text": text})
    items.sort(key=lambda it: it["timestamp"])
    payload = {}
    if note:
        payload["note"] = "Records retrieved for this request, oldest first."
    payload["memories"] = items
    return PREFIX + "\n" + json.dumps(payload, ensure_ascii=False)


def typo(word):
    if len(word) < 4:
        return word
    i = rng.randrange(1, len(word))
    op = rng.randrange(4)
    if op == 0:
        return word[:i] + word[i + 1 :]
    if op == 1:
        return word[:i] + rng.choice("aeiourstln") + word[i:]
    if op == 2 and i < len(word) - 1:
        return word[:i] + word[i + 1] + word[i] + word[i + 2 :]
    return word[:i] + rng.choice("aeiourstln") + word[i + 1 :]


HOLDER_TEMPLATES = [
    "Who owns the {p} now?",
    "who's holding the {p} these days",
    "Remind me who is running the {p} at the moment.",
    "The {p} thing - who has it now? I need their email.",
    "Who is accountable for the {p} as things stand?",
    "whose is the {p} now",
    "Who ended up with the {p}?",
    "Who do i chase about the {p}?",
    "Quick one: the {p} saga, who's the current owner and their work email?",
    "Who is {h} the {p} thread now?",
    "Which person is {h} for the {p} right now?",
    "The {p} - {w} it now?",
    "Who's on point for {p} today?",
    "Tell me who took over the {p} project.",
    "Who is the internal owner of the {p}?",
    "Who is the intrnal lead on {p} now?",
]


def synth_requests(pairs, n):
    """Holder questions about lower-case phrases of the records' handover sentences."""
    grams = []
    for p in pairs.values():
        for s in ownership.SENT.split(p.prompt):
            if not (ownership.OWNERSHIP.search(s) and ownership.PERSON.search(s)):
                continue
            toks = ownership.tokens(s)
            for m in (2, 3, 4):
                for i in range(len(toks) - m + 1):
                    g = toks[i : i + m]
                    if g[0] in ownership.STOP or g[-1] in ownership.STOP:
                        continue
                    if sum(w not in ownership.STOP for w in g) < 2:
                        continue
                    grams.append(" ".join(g))
    grams = sorted(set(grams))
    rng.shuffle(grams)
    out = []
    for g in grams[:n]:
        t = rng.choice(HOLDER_TEMPLATES)
        h = rng.choice(["holding", "responsible", "running", "accountable", "owner"])
        if rng.random() < 0.4:
            h = typo(h)
        w = rng.choice(["whose", "whos", "whoes", "who owns", "who holds", "who runs", "wose"])
        p = g
        if rng.random() < 0.25:
            ws = g.split()
            j = rng.randrange(len(ws))
            ws[j] = typo(ws[j])
            p = " ".join(ws)
        out.append(t.format(p=p, h=h, w=w))
    return out


HOLDERISH = [
    "owns", "owner", "holding", "accountable", "responsible", "running", "whose", "internal", "internally",
    "owner's", "owns's", "holdings", "acountable", "responsable", "runing", "ownr", "hodling", "whos", "whoes",
    "whose", "whole", "while", "whip", "wham", "whoze", "whosee", "intrnal", "internaly", "interal", "intern",
    "interesting", "integral", "responsibly", "accountably", "Owner", "OWNS", "Running", "running's",
]
