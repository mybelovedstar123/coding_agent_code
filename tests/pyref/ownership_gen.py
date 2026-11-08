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
SUBMIT = os.environ["DITTO_SUBMIT_DIR"]  # The directory holding build-agent/ and case/datasets/.
sys.path.insert(0, f"{SUBMIT}/build-agent")

from harness import config, ownership, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "ownership.json")
DATASETS = sorted(glob.glob(f"{SUBMIT}/case/datasets/v13-full-*.json"))
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


def holder_requests():
    out = []
    frames = [
        "Who is {w} for the moto rollout?", "{w} it?", "Is the plan {w}?", "I {w} the thread", "the {w} one",
        "Who's {w} the bakery job now?", "who {w} the kiln repair these days?", "{w}",
    ]
    for w in HOLDERISH:
        for f in frames:
            out.append(f.format(w=w))
    for _ in range(300):
        w = typo(rng.choice(HOLDERISH[:9]))
        out.append(rng.choice(frames).format(w=w))
    out += [
        "Who is on point for the fit-out now?", "who's on poiint for it", "Who has it now?", "Who has the keys now?",
        "Who do I chase about the invoice?", "whose is it anyway", "who owes it", "who hoolds it", "who heads it",
        "The present owner of the gallery job?", "the currnt ownr", "the prsent owner", "Who, as things stand, runs it?",
        "Whoever holds the lease at this point?", "who are we waiting on at the moment", "Whom do we ask today",
        "the person holding it", "the person accountible", "the person running it", "who is responsibile",
        "who's acountable", "who ruinning it", "who owns it", "who ons it", "Who? now", "who is it now?",
        "Who is it? now", "", "WHO OWNS IT NOW", "whoever, these days", "who…now", "who\nnow",
    ]
    return out


def sm_cases(pairs, requests):
    toks = sorted({t for p in pairs.values() for t in ownership.tokens(p.prompt)})
    out = []
    for _ in range(1500):
        a, b = rng.choice(toks), rng.choice(toks)
        if rng.random() < 0.5:
            b = typo(a)
        out.append((a, b))
    texts = [p.prompt[: rng.randrange(150, 1200)] for p in pairs.values()]
    for _ in range(250):
        a, b = rng.choice(texts), rng.choice(texts)
        if rng.random() < 0.3:
            b = a[rng.randrange(len(a) // 2 + 1):] + " " + b[:200]
        out.append((a, b))
    for _ in range(100):
        a = rng.choice(texts)
        b = "".join(typo(w) + " " for w in a.split())
        out.append((a, b))
    for _ in range(100):
        a = rng.choice(requests)
        b = rng.choice(texts)
        out.append((a, b))
    for n in (199, 200, 201, 250, 300, 450, 1000):
        a = "".join(rng.choice("ab c") for _ in range(n))
        b = "".join(rng.choice("abc d") for _ in range(n))
        out.append((a, b))
        out.append((b, a))
        out.append(("a" * n, "a" * (n // 2) + "b" * (n // 2)))
        out.append(("x" + "a" * n, "a" * n + "x"))
    out += [("", ""), ("", "a"), ("a", ""), ("abc", "abc"), ("é—x", "e—x"), ("😀a", "a😀"), ("owner", "owners")]
    out += [(w, h) for w in HOLDERISH for h in ownership.HOLDER_WORDS + ("whose", "internal", "internally")]
    return [[a, b, difflib.SequenceMatcher(None, a, b).ratio()] for a, b in out]


def holder_chains(pairs, idx):
    """Synthetic chains from the records' handover sentences (1-4 sentences each), plus crafted takeover lines."""
    sents = []
    for p in pairs.values():
        for s in ownership.SENT.split(p.prompt):
            if ownership.OWNERSHIP.search(s) and ownership.PERSON.search(s):
                sents.append(s.strip())
    crafted = [
        "Northwind Logistics took over the deliveries from Harbor Freight after the delay.",
        "Maya Chen took over the bakery fit-out from Tom Reyes in March.",
        "Tom Reyes handed the bakery fit-out to Maya Chen.",
        "The fit-out passed quietly to Lena Ortiz, who now owns it.",
        "Status: Lena Ortiz to Paul Gray for the fit-out.",
        "It is Paul Gray's now, the fit-out.",
        "With Anna Bell in charge of the fit-out, things moved.",
        "The ball is with Anna Bell on the fit-out.",
        "Bright Studios picked up the fit-out.",
        "Paul Gray has taken over the fit-out from Bright Studios.",
        "Mark O'Neil is the one accountable for the fit-out now.",
        "Jean-Luc Picard now runs the fit-out; Anna Bell stepped back.",
    ]
    sents += crafted
    out = []
    for _ in range(600):
        k = rng.randrange(1, 5)
        chain = [["2025-01-0%dT00:00:00Z" % (i + 1), rng.choice(sents)] for i in range(k)]
        out.append(chain)
    for i in range(len(crafted)):
        for j in range(len(crafted)):
            out.append([["t1", crafted[i]], ["t2", crafted[j]]])
    orgs_sets = [sorted(idx.orgs), ["Maya Chen", "Paul Gray"], []]
    res = []
    for ch in out:
        orgs = rng.choice(orgs_sets)
        tup = [(a, b) for a, b in ch]
        res.append(
            {
                "chain": ch,
                "orgs": orgs,
                "holder_now": ownership.holder_now(tup, set(orgs)),
                "first_named_holder": ownership.first_named_holder(tup),
            }
        )
    return res


def ownership_cases(pairs, idx, requests, full):
    """with_ownership over each request with a retrieved-record context; variants on the firing ones."""
    cases = []
    fired = 0
    variant_budget = 12
    for r in requests:
        if IDENT_RE.search(r):
            continue
        phrase, chain = idx.best_chain(r)
        holder = ownership.asks_holder(r)
        if not chain and rng.random() > (0.15 if holder else 0.03):
            continue
        ctxs = [make_context(pairs, r, k=6, cut=160)]
        if chain:
            fired += 1
            if fired % 3 == 0:
                ctxs.append(make_context(pairs, r, k=3, note=False))
            if variant_budget > 0:
                variant_budget -= 1
                src = idx.sources(phrase)
                base = json.loads(ctxs[0].split("\n", 1)[1])
                weird = dict(base)
                weird["memories"] = list(base["memories"]) + [
                    "not a dict",
                    {"timestamp": "2024-01-01T00:00:00Z", "text": "no id"},
                    {"pairID": 7, "timestamp": None, "text": "numeric id"},
                    {"pairID": src[0], "text": "stale excerpt without timestamp"},
                ]
                ctxs.append(PREFIX + "\n" + json.dumps(weird, ensure_ascii=False))
                ctxs.append(PREFIX + "\n" + json.dumps({"ownership": "old", "note": None}, ensure_ascii=False))
                ctxs.append(PREFIX + "\n" + json.dumps({"note": 3.5, "memories": {"a": 1}}))
                ctxs.append(PREFIX + "\n" + json.dumps({"memories": [], "note": "x"}, ensure_ascii=True))
                ctxs.append(PREFIX + "\n{not json")
                ctxs.append(PREFIX + "\n" + json.dumps({"note": "dup"}) + "  ")
                ctxs.append("no newline here")
                ctxs.append("")
                ctxs.append(None)
                ctxs.append("custom prefix\n" + json.dumps({"memories": base["memories"][:1]}))
        for ctx in ctxs:
            out = ownership.with_ownership(ctx, PREFIX, idx, r, lambda pid: full[pid])
            case = {"request": r, "context": ctx, "same": out == ctx}
            if out != ctx:
                case["out"] = out
            cases.append(case)
    return cases, fired


def main():
    sets = []
    sm_all = []
    totals = {"requests": 0, "fired": 0, "ownership_cases": 0, "ownership_changed": 0}
    for path in DATASETS:
        pairs, requests = load(path)
        records = [(pid, store.fmt_ts(p.ts), p.prompt) for pid, p in pairs.items()]
        idx = ownership.IndexV2(records)
        memo_best_chain(idx)
        full = {pid: {"pairID": pid, "timestamp": store.fmt_ts(p.ts), "text": store.render_pair(p)} for pid, p in pairs.items()}
        all_requests = requests + synth_requests(pairs, 120)
        req_cases = []
        ident_seen = 0
        for r in all_requests:
            c = {
                "request": r,
                "asks_holder": ownership.asks_holder(r),
                "asks_current_holder": ownership.asks_current_holder(r),
                "lower_grams": sorted(ownership.lower_grams(r)),
                "tokens": ownership.tokens(r),
            }
            req_cases.append(c)
            if IDENT_RE.search(r):
                ident_seen += 1
                if ident_seen % 40 != 1 or ident_seen > 40 * IDENT_SAMPLE:
                    continue
            phrase, chain = idx.best_chain(r)
            c["phrase"] = phrase
            c["chain"] = chain_list(chain)
            if chain:
                c["sources"] = idx.sources(phrase)
                c["occurs"] = idx.occurs(phrase)
                c["candidates"] = idx.candidates(phrase)
                c["holder_now"] = ownership.holder_now(chain, idx.orgs)
                c["first_named_holder"] = ownership.first_named_holder(chain)
        own_cases, fired = ownership_cases(pairs, idx, all_requests, full)
        # fuzzy_in / thread_like / is_person over record material.
        fuzzy = []
        toks_pool = [ownership.tokens(p.prompt) for p in pairs.values()]
        for _ in range(400):
            tk = rng.choice(toks_pool)
            if len(tk) < 3:
                continue
            n = rng.randrange(1, 4)
            i = rng.randrange(max(1, len(tk) - n + 1))
            phrase = " ".join(typo(w) if rng.random() < 0.3 else w for w in tk[i : i + n])
            other = (rng.choice(toks_pool) if rng.random() < 0.3 else tk)[: max(i + n + 20, 60)]
            ratio = rng.choice([None, None, 0.0, 0.5, 0.95])
            fuzzy.append([phrase, other, ratio, ownership.fuzzy_in(phrase, other, ratio)])
        words = sorted({w for tk in toks_pool for w in tk})
        thread_words = [typo(b) for b in ownership.BOUNDARY_WORDS for _ in range(30)] + rng.sample(words, 300)
        thread_words += list(ownership.BOUNDARY_WORDS) + ["thr", "sag", "projects", "situations", "busyness", "tread"]
        thread = [[w, ownership.thread_like(w)] for w in thread_words]
        names = sorted({m.group(1) for p in pairs.values() for m in ownership.PERSON.finditer(p.prompt)})
        persons = [[n, ownership.is_person(n)] for n in names]
        cand_phrases = [c["phrase"] for c in req_cases if c.get("phrase")] + rng.sample(words, 50)
        cand_phrases += [" ".join(rng.sample(words, 2)) for _ in range(50)]
        index_cases = [
            {"phrase": ph, "candidates": idx.candidates(ph), "occurs": idx.occurs(ph), "chain": chain_list(idx.chain(ph)),
             "sources": idx.sources(ph)}
            for ph in cand_phrases
        ]
        sets.append(
            {
                "dataset": os.path.basename(path),
                "records": [[pid, store.fmt_ts(p.ts), p.prompt, p.response] for pid, p in pairs.items()],
                "index_order": idx.pids,
                "orgs": sorted(idx.orgs),
                "requests": req_cases,
                "with_ownership": own_cases,
                "index_cases": index_cases,
                "fuzzy_in": fuzzy,
                "thread_like": thread,
                "is_person": persons,
                "holder_chains": holder_chains(pairs, idx),
            }
        )
        sm_all += sm_cases(pairs, requests)
        totals["requests"] += len(req_cases)
        totals["fired"] += sum(1 for c in req_cases if c.get("chain"))
        totals["best_chain_run"] = totals.get("best_chain_run", 0) + sum(1 for c in req_cases if "chain" in c)
        totals["ownership_cases"] += len(own_cases)
        totals["ownership_changed"] += sum(1 for c in own_cases if not c["same"])
    extra_holder = [
        {"request": r, "asks_holder": ownership.asks_holder(r), "asks_current_holder": ownership.asks_current_holder(r),
         "lower_grams": sorted(ownership.lower_grams(r))}
        for r in holder_requests()
    ]
    sent_texts = [
        "One. Two; three! Four? five", "a … b … c", "a…b", "Done.\n\nNext … then. ", "x. … y", "x.… y", "  lead. ",
        "Tab.\tnext", "nbsp. next", "sep.\x1cnext \x1f… z", "q.\n\n\n", "...  ok", "end.", "", "\n", "a … b."
        "Mr. Smith took over. Then Ana Lima did!", "Hand-off: Jo Park to Li Wen; done.",
    ]
    sent_cases = [[t, ownership.SENT.split(t)] for t in sent_texts]
    fixture = {
        "prefix": PREFIX,
        "datasets": sets,
        "holder_requests": extra_holder,
        "sequence_ratio": sm_all,
        "sentences": sent_cases,
    }
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(fixture, f, ensure_ascii=False)
    totals["sequence_ratio"] = len(sm_all)
    totals["holder_requests"] = len(extra_holder)
    totals["holder_requests_true"] = sum(1 for c in extra_holder if c["asks_holder"])
    print(json.dumps(totals))


if __name__ == "__main__":
    main()
