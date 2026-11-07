"""Differential fixtures for src/tool_hints.rs from harness/tool_hints.py.

Requests: every tool_cases[].prompt and memory_cases[].question of each v13-full dataset, the request fixtures of
build-agent/tests (send / exact-value), and synthetic requests that reach each trigger's branches (typos, Unicode word
boundaries, numbers). Catalogs: each dataset's own tool_defs (built as agent.py builds them from `catalog`, with
pytool.DEFINITION appended), an empty catalog, and a synthetic catalog that offers every tool a rule names (with
descriptions that exercise the strip / rstrip(".") / [:200] formatting). Writes tests/fixtures/tool_hints.json."""

import glob
import json
import os
import random
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import pytool, tool_hints  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "tool_hints.json")
REF = "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent"
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))

WANTS = [
    "wants_schedules",
    "wants_image_edit",
    "wants_arithmetic",
    "wants_background_job",
    "wants_calendar_entry",
    "wants_sandbox_run",
    "wants_job_figure",
    "wants_note_fix",
    "wants_research_send",
    "wants_usual_effort",
    "asks_own_preference",
    "wants_send_on_record",
    "wants_exact_value",
]
CATALOG_NOTES = ["research_send_note", "effort_note", "tool_hint_note", "send_on_record_note"]
PLAIN_NOTES = ["exact_value_note", "preference_note"]


def tool_defs_for(catalog):
    wire = [t for t in catalog if isinstance(t, dict) and t.get("name")]
    defs = [
        {
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t.get("description", ""),
                "parameters": t.get("parameters") or {"type": "object", "properties": {}},
            },
        }
        for t in wire
    ]
    if pytool.TOOL_NAME not in {t["name"] for t in wire}:
        defs.append(pytool.DEFINITION)
    return defs


requests, seen = [], set()


def add_request(text, source):
    if text in seen:
        return
    seen.add(text)
    requests.append({"text": text, "source": source})


catalogs = []
for path in DATASETS:
    d = json.load(open(path, encoding="utf-8"))
    catalogs.append({"name": os.path.basename(path), "tool_defs": tool_defs_for(d["catalog"])})
    for c in d["tool_cases"]:
        add_request(c["prompt"], "tool_case")
    for c in d["memory_cases"]:
        add_request(c["question"], "memory_case")
for name in ("send_requests.json", "exact_value_requests.json"):
    fx = json.load(open(os.path.join(REF, "tests", "fixtures", name), encoding="utf-8"))
    for group in (v for v in fx.values() if isinstance(v, list)):
        for c in group:
            add_request(c["prompt"], "ref_fixture")

synthetic = [
    # schedules
    "Which workflows are due to run this week?",
    "How many runs has the nightly backup workflow logged?",
    "Set up a workflow that runs every day",
    "Put a weekly workflow on repeat for me",
    "what do i have on repeat?",
    "Show me the recurrng runs of my schedules",
    "Which automations go on their own on a timer?",
    "Remind me which workflows run automatically",
    "Start the focus timer",
    "Which scheduled workflows run automaically each night?",
    "Every day my workflow runs on its own",
    # image edit
    "Make the image from earlier brighter",
    "Take the picture you just generated and add a hat",
    "Create a new image of a cat, unlike the one from earlier",
    "Edit the one you just made: more blue",
    "Could you tweak that latest render so it's darker?",
    "Draw me a new picture like the previous pic",
    "Change the imagee from before to black and white",
    "Generate a photo from earlier ideas",
    # arithmetic
    "What is the average of 12, 15.5 and 20?",
    "Compute 3,5 minus 1.25",
    "the difference between -4 and 7 please",
    "total for 10 and x20 and 30x",
    "What's 12.5 times 3?",
    "ratio of 1.2.3 and 4",
    "Sum 5 and 6٣",
    "the spread between 10² and 3",
    "percentage of 40 in 50_0 and 9",
    # background jobs
    "Launch a background job to reindex my notes",
    "Kick off an agent to summarise the thread",
    "What's the status of my background job?",
    "Spin up an asynchronous task to crawl the site",
    "delegate a task to a worker while i keep working",
    "Queue up a job for the import",
    "How is my recent backgrund job doing?",
    # calendar
    "Put lunch with Sam on my calendar",
    "Add the dentist to my calender tomorrow",
    "Block out Friday afternoon for me",
    'Add "Project Alpha" review to the calendar',
    "Add case-0042abcd review to the calendar",
    "pencil in a call with Jo for me",
    "get it on the calndar",
    "Slot the 1:1 into my calendar",
    "Add the “retro” to my calendar",
    # sandbox
    "Run the checksum routine in the sandbox and tell me what it prints",
    "In code mode, run the in-process routine",
    "Use the sandbx for code",
    "sandbox please",
    # job figure
    "How many items did the backfill job process?",
    "What count does the job list report for the crawl job?",
    "How many items has the background work processed? Start it if needed",
    "Number of records the agent jobs i dispatched went through",
    "How many did the backgroud tasks process",
    # note fix
    "Fix the note: the handoff is Thursday now",
    "Update the note in my scratchpad: moved to Monday",
    "The handoff is friday, scratch that",
    "Take a note about lunch",
    # research send
    "Look up the current EUR rate and send it to Maria",
    "Check the latest reading and email it over to them",
    "For Dana — find the figure and pass it on to Dana",
    "Which email do I have for Tom?",
    "for kim contact lookup: which address belongs to kim",
    "Send an update to Joe about the launch",
    "What number is it sitting at right now? Forward it to Lee",
    "Find what it's at today and shoot it to thm",
    "pull the value and get it over to Ana",
    # usual effort
    "Set my reasoning effort to the usual",
    "Set thinking depth to what I normally use",
    "Set reasoning effort to high",
    "How hard do you think, as usual?",
    "use my normal resoning levl",
    "Thinking effort: my usual, deeply",
    # preference
    "Which accent colour is right for my workspace?",
    "What font should my own Ditto use?",
    "Set my accent colour to the usual",
    "Which mode suits my interface? Please confirm",
    "suppose my workspace needed a typeface, which one",
    "My workspace accent?",
    "What colour is right for my personal look?",
    "Which acent is set on my workspce?",
    "if you picked the mode for my app what would it be",
    # send on record
    "Send them the notes from Tuesday",
    "Forward the invoice to Priya",
    "Can you send it to them?",
    "Get the deck over to them",
    "get them the slides",
    "Reply to her with the summary",
    "Shoot him the draft",
    "pass it along to the team",
    "Get the signed Q3 contract over to thm",
    "sned the file to Mark",
    "forwrd the report to him",
    # exact value
    "Give me the exact value it returns",
    "What precisely is the figure?",
    "the exactly reading",
    "Report the precise amount",
    "exxact numbr",
    "I need the exakt code",
    "precise",
    # Unicode word boundaries, whitespace classes, empty
    "",
    "   ",
    "Set my reasoning effort to the usual",
    "Set my reasoning\x1ceffort to the usual",
    "Which workflows are due to ruń this week?",
    "Which workflows are due to run‍ this week?",
    "Send theḿ the notes",
    "Send thém the notes to Émile",
    "Add lunch to my calendar‍",
    "Look up the current rate and send it to Zoë",
    "Look up the current rate and send it to Élise",
    "the average of 3́ and 4",
    "the sum of ² 3 and 4",
    "note: is tuesdaý now, update the note",
    "exact́ value",
    "Run the checksum in the saßbox",
    "İ want the exact value",
]
for t in synthetic:
    add_request(t, "synthetic")

long_desc = "  Lists the saved schedules. " + "x" * 230 + ".. "
synthetic_catalog = [
    {"type": "function", "function": {"name": n, "description": desc}}
    for n, desc in [
        ("list_schedules", long_desc),
        ("edit_image", "Edit an image the user already has... "),
        ("run_code", " Run code in a sandbox.\n"),
        ("execute_agent_job", None),
        ("calendar_create_event", ""),
        ("list_agent_jobs", "List background jobs ."),
        ("update_memory", "Update a stored memory. {desc} {tool}"),
        ("gmail_send", "Send an email."),
        ("search_web", "Search the web."),
        ("set_reasoning_effort", "Set the reasoning effort."),
        ("list_schedules", "Second definition replaces the first."),
    ]
] + [{"type": "function", "function": None}, {"type": "function"}, {"type": "function", "function": {"name": None}}]
catalogs.append({"name": "synthetic", "tool_defs": synthetic_catalog})
