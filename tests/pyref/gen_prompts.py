"""Writes src/prompts.rs: every prompt and note text of the Python reference as a Rust literal, byte-identical.

Run from WSL: python3 tests/pyref/gen_prompts.py (reads submit/build-agent/harness)."""

import sys
from pathlib import Path

REF = "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent"
sys.path.insert(0, REF)
from harness import agent, policy, router  # noqa: E402

TEXTS = [
    ("MEMORY_GUIDANCE", policy.MEMORY_GUIDANCE, "Guidance for requests handled from memory (and general conversation)."),
    ("TOOL_GUIDANCE", policy.TOOL_GUIDANCE, "Guidance for requests that act through the app tools."),
    ("ROUTER_PROMPT", router.ROUTER_PROMPT, "The router's system prompt."),
    ("WRAP_UP_NOTE", agent.WRAP_UP_NOTE, "Added once when the case budget is nearly spent (advisory; tools stay offered)."),
    ("FINAL_CHECK_NOTE", agent.FINAL_CHECK_NOTE, "Stage 3b note with one draft."),
    ("SECOND_DRAFT_NOTE", agent.SECOND_DRAFT_NOTE, "Stage 3a note: an independent second draft."),
    (
        "FINAL_CHECK_TWO_DRAFTS_NOTE",
        agent.FINAL_CHECK_TWO_DRAFTS_NOTE,
        "Stage 3b note with two drafts ({a} and {b} are filled in).",
    ),
    ("FINAL_ANSWER_NOTE", agent.FINAL_ANSWER_NOTE, "The last resort's note (4c)."),
    ("MODEL_FAILURE_NOTE", agent.MODEL_FAILURE_NOTE, "After a failed model call (4d)."),
    ("PROGRAM_RECORDS_NOTE", agent.PROGRAM_RECORDS_NOTE, "Long identifier event histories: how to read them with run_python."),
    ("REPEAT_NOTE", agent.REPEAT_NOTE, "A repeated call to a tool that already returned a result ({name} is filled in)."),
]

BACKSLASH = chr(92)


def lit(s: str) -> str:
    out = []
    for ch in s:
        if ch == BACKSLASH:
            out.append(BACKSLASH * 2)
        elif ch == '"':
            out.append(BACKSLASH + '"')
        elif ch == "\n":
            out.append(BACKSLASH + "n")
        elif ord(ch) < 0x20 or ord(ch) > 0x7E:
            out.append(BACKSLASH + "u{%x}" % ord(ch))
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


lines = [
    "//! Every prompt and note the agent sends, as literal constants (rule R8: one module, no assembly by replacement).",
    "//!",
    "//! Generated from the Python reference by `tests/pyref/gen_prompts.py`; regenerate rather than edit, so the texts",
    "//! stay byte-identical to the reference.",
    "",
]
