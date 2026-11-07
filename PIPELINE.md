# Pipeline declaration (Thief harness)

## Implementation (Rust)

This agent is a Rust port of the Python reference (`submit/build-agent`, tag v14). Each `harness/<module>.py` is
`src/<module>.rs`. Every model request it sends is meant to be identical to the reference's for the same case;
`PORTING.md` holds the contract.

- `src/py.rs` and `src/pyre.rs` carry the Python semantics that guarantee identical requests: code-point string
  lengths and slicing, CPython `repr`, float and `json.dumps` output, and `re` behaviour. `src/difflib.rs` is the
  one port of `difflib.SequenceMatcher(...).ratio()`, shared by every module that fuzzy-matches words.
- All prompt and note texts are generated from the reference into `src/prompts.rs`.
- **The image:** the binary runs on `python:3.12-slim`. CPython 3.12 is used only by the `run_python` program tool:
  the model writes Python, and the program is validated by CPython's own `ast` and run in a resource-limited child,
  exactly as before.
- **Equivalence is proven in three ways:**
  - per-module differential tests against the reference (`tests/`);
  - an end-to-end harness that drives both agents with the same deterministic fake model, embedder and tool
    endpoint, then compares every model request (`tests/equiv/equiv.py`);
  - a cached replay of full runs with zero cache misses.


Declared model stages, their inputs, selection rules and recovery rules (policy
v13 W7). Every stage is a call to the locked relay model; there is no
trained classifier, lookup table, miner-trained component or expected-answer
comparison in the harness (the router below is itself a declared model call).
One deterministic evidence selector exists: identifiers in the user's message
(a token of at least 8 characters mixing letters and digits) pre-load the
records that carry them (see Context construction). Retrieval uses one hosted inference component: the text
embedding endpoint the validator provides (Ollama-compatible API at
`OLLAMA_BASE_URL`, model name `embeddinggemma`, 768 dimensions; on-chain the
broker serves the platform's locked embedding route). Together with BM25 it
selects which records are retrieved (top-k), so it is an evidence selector; it
never writes or chooses an answer. Every chat call goes to the locked relay
model (`DITTOBENCH_MODEL`, default `openai/gpt-oss-20b`) with temperature 0 and
seed 42; the router's first call and the last resort (4c) request low reasoning effort, all
other calls leave the relay's default effort. Token usage of every call below is added to the reported
`prompt_tokens` / `output_tokens` (W12). The request's system prompt is at the
head of every deciding, draft, check and recovery call and takes precedence over
the miner guidance (W2); the router receives it after its dispatcher prompt, as
host instructions the assistant follows.

## Stages

| # | Stage | Role | Input | Output used for |
|---|---|---|---|---|
| 0 | Spelling reading (`src/spelling.rs`) | advisory | the user's message only (no records, no tools) | one low-effort call, run concurrently with the router, returns the message with its spelling mistakes corrected and nothing else changed. Its output is used only if every digit-bearing token (identifiers, numbers, dates) is unchanged and its length is within 30 % of the original; then the deciding stages receive it as a reading note appended to the system prompt, which says that the user's message itself is what they sent and the note is only for reading misspelled words. The request itself is never changed. Failure, timeout or an unusable output adds no note. Usage is reported. |
| 1 | Router (`src/router.rs`) | advisory | request system prompt, user message, live catalog names with descriptions (each description shortened to its first 220 characters) | choosing which miner guidance text accompanies the request (no tool hint is added) and therefore whether the final check (3b) runs. The full live catalog is always offered; nothing is pinned, forced, removed or reordered. The router is asked at low reasoning effort; only when that yields no valid decision (an empty or unparseable reply) it is asked once more at the relay's default effort. If neither yields a valid decision, the memory-guidance path is used. Usage of both calls is reported. A "tool" decision that names no app tool of the live catalog (memory read tools do not count) is treated as a "memory" decision. The router has a 40 s budget. |
| 2 | Deciding loop (`src/agent.rs`) | deciding | request system prompt + miner guidance, pre-loaded memory records, user message, full live catalog (+ local `run_python`) | the model's tool calls and its reply. `tool_choice` is never set. A reply without tool calls ends the loop; it is the final result unless stage 3b checks it as its draft. |
| 3 | (not used) | — | — | There is no post-tool synthesis stage: on the tool path the deciding loop's own reply (full catalog offered on every turn) is the final result. |
| 3a | Memory-path second draft | advisory | the complete transcript and the same evidence as 3b, without the loop's reply | when 3b is about to run with at least 60 s left, one independent no-tool call writes a second draft reply from the same records. An empty, failed, provider-error or artefact output is discarded. If it is non-empty and differs from the loop's reply after trimming leading and trailing whitespace (a shape test only, never a comparison of meaning), 3b receives both drafts (A = the loop's reply, B = this draft); otherwise 3b runs on the loop's reply alone. The host never chooses between the drafts: the 3b model checks both against the records and writes the final reply (best-of-N selection by a declared deciding model, W7). |
| 3b | Memory-path final check | deciding (final) | the complete transcript and the loop's reply as its draft (plus draft B from 3a when present) | runs for every non-empty reply of the deciding loop on the memory path with at least 30 s left; no request class is exempt. Replies written by the last resort (4c) are not re-checked. No tools are offered, so it also receives the complete text of every pre-loaded record that the deciding turn saw only as an excerpt. Its note asks the model to keep a correct draft unchanged (with two drafts: either draft unchanged) or correct a wrong or incomplete one, and restates the publicly documented answer forms (PROTOCOL.md: a grounded decline that names the closest related records found and why they do not answer, without presenting another record's value as the answer; a clarifying question that names the missing slot and cites what was found), plus, for a request that assumes something the records do not show, saying there is no record of it and what the records do show. The model checks its draft(s) against the records and writes the final reply, which may be a draft unchanged (including a refusal or a question to the user). Its non-empty output is the final result; empty, failed, provider-error or a transport artefact (a `<|start|>`-style channel token): the draft is kept (with two drafts, draft A, the loop's reply). For the request classes listed under Context construction, the request-class note is appended to its note (decided from the request and the user's records only). |
| 4 | Recovery | recovery | same transcript | (a) once per request, a turn that ended with provider `finish_reason: "error"` (transport failure) whose tool calls are all malformed (or that has none) is discarded, including any text it carried, and asked once more, its calls recorded as blocked; well-formed calls in such a turn are executed; (b) a turn with no text and no calls is asked to reply, at most twice; (c) if the loop would return nothing, one final no-tool call at low reasoning effort; (d) once per request, if a model call itself fails (the client's retries are exhausted), the model is told the previous call failed and may answer or continue with tools (the catalog stays offered); a second failed call ends the loop and the final stages answer from the transcript. Apart from (a), whose provider error marks the turn as a transport failure, none of these fire on a non-empty reply, a refusal, or a no-call decision. |

Selection rule: the first valid (non-empty) output of the last stage that ran
is shipped. Apart from the whitespace-trimmed identity test in 3a, which only decides whether 3b sees one or two drafts, the host never compares candidate answers, never checks them
against any expected value, never re-asks a valid reply because of its meaning,
and never continues generation until a preferred answer, vocabulary, action or
refusal state appears.

## Limits (declared)

- Time: each request has a 270 s budget (validator deadline 300 s). Ordinary
  calls keep a 60 s reserve for the answer. With 90 s left (120 s when the
  transcript exceeds 150,000 characters) the model is told that time is nearly
  up and asked to answer from the results it has; this is advisory only: the
  full catalog stays offered and any call it makes is executed. Tools are never
  withdrawn from a deciding turn. The wrap-up note and the model-failure note
  (4d) share one flag: at most one of them is added per request, and once either
  was added a further failed model call ends the loop. When only the 60 s answer
  reserve is left, the deciding loop ends and a final stage writes the answer
  from the transcript: when no reply exists yet, recovery (c).
- Turns: the deciding loop runs at most 12 model turns; after that the stages
  above (final check, recovery) write the reply from the transcript.
- An identical call runs once per request (`REPEAT_NOTE`, `call_key`,
  `repeat_exempt`): a call with the same tool name and the same arguments
  (object keys sorted) as a call that already returned a result in this
  request is not executed; the model is told so and to use the result it has
  or call a different tool. The call is recorded as blocked
  (`blocked_tool_calls`), not reported as executed. A call with different
  arguments always runs. When the user asks for something to be done again
  (again, another, once more, repeat), no call is blocked. A call that returned
  an error does not count, so it can be retried. The memory read tools and
  tools named `read_*` or `fetch_*` are exempt.
- Call count note (`REQUERY_NOTE`, advisory): after a turn in which any
  non-reader tool has run more than once in this request, one user-role line
  lists those tools with their call counts and says that each call is an
  action taken for the user: reply if the results answer the request, call
  again only for something they do not contain. Nothing is blocked, the full
  catalog stays offered, and a request to do something again gets no note.
- Model client: each call has a 75 s timeout within the request budget and is
  retried (up to 8 attempts, with backoff) only on transport failures, HTTP
  408/409/425/429/5xx-gateway statuses, or an HTTP 200 body that carries an
  `error` field or no usable choice; it never retries a valid completion.
- Tool calls: no call starts with less than 15 s left in the request budget
  (the model is told the call was not executed and to answer now), and a tool
  endpoint wait never extends past the request budget.
- Failures: a request never fails because a dependency did. Embedding calls are
  bounded by the request budget. When retrieval fails, the context keeps the
  identifier and handle-channel records, which need no embedding; when an
  evidence add-on fails, the request is answered without the memory context; a
  failed memory tool returns an error result to the model. A malformed request
  body is answered with HTTP 400.
- Context sizes: up to 10 retrieved records are pre-loaded; the first 4 as
  excerpts of up to 1,400 characters, the rest up to 600; identifier records
  (see Context construction) always in full, with no size cap.
- Memory tools: search previews are up to 900 characters per record; `topK`
  defaults to 8 and is clamped to 1-25.
- `run_python`: programs up to 8,000 characters, output truncated at 6,000
  characters, 15 s wall time, 8 s CPU, 768 MB address space, 3 open files, no
  child processes.
- Embeddings: batches of at most 32 texts / 100,000 characters, at most 4
  concurrent requests.

## Tool execution (W11, W12)

- Executor guard: only a tool offered in the current turn can run. Calls to
  unknown or unoffered tools, calls with arguments that are not a JSON object,
  and memory read calls with record ids that neither exist in this user's store
  nor were named exactly by an id field (`id`, `pair_id`, `memory_id` and their
  camel-case forms) of an earlier executed tool result of the same request
  (error-only results do not count; empty ids never pass) are blocked before
  execution: the model receives an error and nothing is sent. An accepted id the
  store does not hold is simply not found.
- Every call receives one W12 state. Reported in `tool_calls`: `completed`
  (the endpoint returned a result, or the call ran in the harness) and
  `delivered_failed` calls the tool itself answered with an error (for non-memory
  tools the endpoint's error is passed to the model as an error, never as an
  empty result). Not
  reported, listed in the advisory `blocked_tool_calls`: `blocked_before_execution`,
  `not_delivered` (connection failure before sending), `delivered_failed` refused
  by the endpoint with HTTP 4xx before the tool ran, and `delivery_unknown`
  (timeout, HTTP 5xx or an unreadable body after sending). Only a
  connection that failed before sending is retried; a delivery-unknown call is
  never replayed. The validator's endpoint ledger is authoritative.
- `hop` is the call's position: it advances for every call that was reported
  or may have crossed the endpoint boundary (not for not-delivered or blocked
  calls), and is the value sent to the endpoint; reported hops are unique.
- A turn that ended with provider `finish_reason: "error"` is discarded and asked
  again only when none of its tool calls is well formed; well-formed calls are
  executed.
- The no-tool final stages (memory final check, last resort)
  offer no tools; any call such a stage still emits is listed in
  `blocked_tool_calls` as `blocked_before_execution`.
- No other host-side de-duplication: apart from the identical-call rule above,
  every allowed call the model issues is executed.
- Every executed call is reported in order with its model-authored name and
  arguments.
- Memory read tools (as named in the request catalog) answer from this user's
  seeded store held by the harness (local execution, always reported as
  completed). Each call is also sent to the tool endpoint so the validator
  observes the model's complete trajectory, including its memory reads; the
  endpoint does not serve memory tools and answers with a "not available"
  error (PROTOCOL.md), which is not shown to the model. The model receives only
  the local store's result. `save_memory` is an endpoint-served action: it is
  applied to the local store only when the endpoint returned a result (the
  saved note's id derives from its content and it is filed one second after
  the user's latest record, so no clock or random value reaches the model); it is
  reported when the endpoint executed it (result or the tool's own error).
  When the request supplies no tool endpoint, memory tools (including
  `save_memory`) run only against the local store and are reported as
  completed local executions; any other catalog call is blocked before
  execution.
  `fetch_memories` returns each record in full up to 60,000 characters per side
  of the exchange; a longer side keeps its head and tail and is marked as
  shortened (its complete text is available to `run_python`). The harness adds its own memory
  tool definitions only when the request carries no catalog at all.
- `run_python` (when the request's catalog does not offer a tool of that name;
  otherwise the catalog's tool is used and sent to the endpoint) is a
  harness-local, resource-bounded interpreter for
  model-authored programs over this user's records (W6): separate process,
  clean environment, CPU/memory/file/process limits, import allowlist; it is
  local execution, never sent to the endpoint, and reported in order with the
  other executed calls (it does not consume an endpoint hop position).

## Output normalization (W8)

`src/typography.rs` (version 2): a fixed, request-independent
normalization applied to `final_text` only. It maps typographic hyphen and
space variants (U+2010, U+2011, U+00A0, U+2007, U+2009, U+202F) to their plain
equivalents, and it removes markdown strong-emphasis markers from paired spans
on one line and set off from the surrounding words (`**17 days**` becomes
`17 days`), keeping the span's text as written. Other asterisks (operators,
code) and all underscores (identifiers) are left alone. Nothing else is changed: no digit, sign,
word, unit or identifier letter.

## Context construction

Memory records are retrieved by embedding similarity and a typo-tolerant BM25
index. When the request names an identifier (for example a case or order
number), every record carrying it is pre-loaded in full, oldest first, with no
size cap (never an excerpt or an id-only pointer), and other retrieval hits are
not added. Other retrieved records are
shown as excerpts; the context note calls all pre-loaded records excerpts,
including identifier records shown in full. An excerpt's full text is one
`fetch_memories` call away.

Alias-aware search: a record that writes exactly one quoted single name next to
exactly one full name declares that name as the person's alias; searches that
mention one form also search the other. With several identifiers in a request,
identifier retrieval keeps the records of the most specific one (the
identifier matching the fewest records; equal counts go to the identifier the
request names first), so records that only share a broader identifier (the
same entity in another case file) are not pre-loaded. Every note that works
from the request's identifier uses that same identifier.

Companion records (`src/companions.rs`): a plan's later notes often name
only the person it was made with ("When Sparky and I first mapped that trip
out …"), not the plan itself. An identifier record or retrieval hit that shares
a rare phrase with the request (two adjacent content words found in at most 2
of the user's records) is an anchor; for each person the anchor names who also
appears in a first-person joint mention ("<Name> and I"), the records carrying
that joint mention and sharing a content word with the request are added in
full, at most 4 per request, with the other pre-loaded records (oldest first).
The index is built from the records once per user and rebuilt after any
record changes. It reads only the request and the user's records.

Ordering: every ranking and list that reaches the model is a total order over
record content. Equal scores, and records filed at the same time, are ordered
by their text; never by an id (ids are opaque and differ between runs), by the
order records arrived in, or by set iteration. A record without a usable
timestamp sorts as the oldest.

Event histories: when the request's identifier records hold an event history
(at least 10 events, given as JSON/CSV/markdown tables with entity, field,
value, effective step and operation, or as sentences quoting the entity, field
and value with the step), the context also carries `event_state`: a host-side reading of the state the
history describes at the step the request names (the last step when it names
none), replayed under the rules the records themselves state (by effective step,
not document order; an assignment replaces the value; add/remove change one set
member; state persists until changed). It is labelled as an unverified
candidate, not an answer, that can be wrong; the records decide. The records
stay in the context unchanged; the model reads both and writes the reply.

Change notes: when the request's identifier records are short notes (up to
15,000 characters) and one of them says that something changed on the
request's topic (an update, a revision, a take-over, a rescheduling, a
replacement of an earlier value; copies with typos are grouped by text
similarity), the context also carries `current_statements`: that note with the
earlier plain notes of the same topic it supersedes, because refiled copies'
filing dates do not show which note came last. Asides ("Separately, …",
"Unrelated: …") are never listed. The note also asks the model to name the
current decision with the statement's own words and verb form (not rephrased into
another tense). The records stay unchanged; the model reads them and writes the
reply.

Tool hints (tool path only): when the request's wording matches what one offered
tool is specifically for — when saved workflows run, which run on their own or
how many runs one logged (`list_schedules`); an edit to an image from earlier
(`edit_image`); arithmetic or statistics on at least two numbers the request
gives (`run_code`); an explicit request for a worker or background job
(`execute_agent_job`); a plain request to put something on the calendar that
names no case identifier (`calendar_create_event`) — and that tool is in the
live catalog, the system prompt gets one line naming the tool and quoting its
own catalog description (for the image edit and the calendar entry also how to
pass what the request gives). The trigger comes from the request and the
catalog only; the model chooses every call.

More request-specific notes (tool path, same form as the tool hints above, advisory, no values):
code-sandbox / Code Mode requests point to `run_code`; a figure a background job reports
(items processed, a count) points to `list_agent_jobs`; correcting a stored handoff or
scratchpad note points to `update_memory`, updating the same note in place;
feedback meant for the people who build Ditto ("let the team know", "report to
the devs", "whoever builds this") points to `file_feedback_for_team`, passing
the user's own words without asking for an address; a change to which kinds of
tools the assistant may use in chat ("turn off agent jobs", "allow only memory
tools", "change my chat tools so …") points to `set_chat_tool_preferences`; a
question about whether or where Ditto can do something ("is there a way to …",
"where do I find the option for …") points to `discover_capabilities`, answering
from what it reports without changing the setting; a piece of software work the
user wants done rather than explained ("go ahead and refactor …", "write a
script to … — start it", "actually scrape …", "don't just tell me how") points
to `execute_agent_job`. A request
for the user's usual reasoning effort that names no level gets one line: set the default
level the records state, or, when the records say none was chosen, ask which level,
naming the options they describe. A request to start something the way it was
agreed earlier ("the way we already agreed", "per what we landed on"; the
agreement word typo-tolerant), when the catalog offers `list_workflows` and
`run_workflow`, gets one line: the records say what was agreed; for a saved
workflow, list the saved workflows first and run the one that matches, or
create it under its agreed name if it is not saved yet; for a one-off job,
dispatch it and do not create or run a workflow. A request to have a quoted,
named item on the user's calendar ("get the "…" review onto my calendar"),
when the catalog offers `calendar_create_event`, gets one line: if the records
say it is already on the calendar, do not create another event but find it
with the calendar search and say it is there; otherwise create it as asked. A
request to apply an appearance option the user spells only approximately, with
a lookup of the configured options first ("look the list up first, I may have
botched the name"), when the catalog offers `discover_capabilities`, gets one
line: after the lookup, apply the listed option closest to what they wrote
(swapped, doubled or missing letters; a variant word belongs to the option),
and do not stop because the exact spelling is not listed.

Exact-value requests (tool path, in addition to the notes above): when the request asks for
the exact or precise value, figure, number, reading, amount, result or code (a typo-tolerant
word match on the request only), the system prompt gets one line: a tool's result can mention
more than one item, each with its own value; report the value the result gives for the item the
user named, copied exactly as written, and leave out the other items and their values. It names
no item or value; the model reads the result and writes the reply.

Send requests (both paths): when the request is an instruction, not a question, to send, forward or reply
something to a person it points to by pronoun or name, and `gmail_send` is offered, the system prompt gets
one line: find in the request or the records who receives it; when they name the recipient and their
address, send it with `gmail_send` to that address (if the records do not hold the content to send, write a
short message from what the request and the records say about it and send that, rather than asking the user
for the content); when the records say no recipient has been decided, send nothing and ask the user who
should receive it, naming the people the records mention. Requests to look a
figure up first get the look-up-and-send note below instead. Advisory; the model makes every call.

