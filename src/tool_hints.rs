//! Tool hints: a request whose wording matches what one offered tool is specifically for (when automations run and
//! how often they have run; an edit to an earlier image; arithmetic on numbers the request gives) gets a one-line note
//! naming that tool and its own description. The trigger comes from the request and the offered catalog only; the
//! note is advisory, names no value, and the model still chooses every call.
//!
//! Patterns are `event_state::PyRe`, so `\s`, `\w` and `\b` keep Python's classes.

use crate::difflib::sequence_ratio;
use crate::event_state::PyRe;
use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::Value;
use std::sync::LazyLock;

pub const TOOL_HINT_NOTE: &str = "For this request, the offered tool {tool} is the one made for it ({desc}). Call it \
once with what the request gives and answer from its result.";
pub static NUMBER_RE: LazyLock<PyRe> = LazyLock::new(|| re(r"(?<![\w.])-?\d+(?:[.,]\d+)?(?![\w])"));

fn re(p: &str) -> PyRe {
    PyRe::new(p)
}

static WORDS_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z]+"));

pub fn _words(text: &str) -> Vec<String> {
    WORDS_RE.findall(&text.to_lowercase())
}

// ------------------------------------------------------------------------------------------------------- triggers

pub fn _fuzzy(w: &str, target: &str, ratio: f64) -> bool {
    w == target
        || (py::len(w) >= 4
            && w.chars().next() == target.chars().next()
            && py::len(w).abs_diff(py::len(target)) <= 2
            && sequence_ratio(w, target) >= ratio)
}

pub fn _any(words: &[String], targets: &[&str], ratio: f64) -> bool {
    words.iter().any(|w| targets.iter().any(|t| _fuzzy(w, t, ratio)))
}

fn has_word(words: &[String], word: &str) -> bool {
    words.iter().any(|w| w == word)
}

static SCHED_COUNTING: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(how m\w{1,3} r\w{1,4}|logged|run count|pull up|lay out|show me|what|which)\b"));
static SCHED_SET_UP: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(put|set up|make|have|turn)\b[^.?!]*\b(every|each|daily|weekly|nightly)\b"));
static SCHED_EVERY: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(every|each|daily|weekly|nightly)\b"));
static SCHED_ABOUT_RUNS: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b((?:due|q\w{3,6}d|lined up|set|scheduled) to run|timed runs?|on a c\w{3,5}e|",
        r"on th\w{2,3} own|on (?:their|its) own|on a t\w{3}r|how m\w{1,3} r\w{0,3}s|run count|",
        r"logged|on repeat|automatic runs?|automatically|automaically|go(?:es)? on their own)\b",
    ))
});

/// When saved workflows run, which run on their own or on a timer, or how many runs a scheduled one logged (not
/// a calendar request, not a timer to start, not how to set up a reminder).
pub fn wants_schedules(text: &str) -> bool {
    let w = _words(text);
    let low = text.to_lowercase();
    if ["reminder", "reminders", "start", "focus"]
        .iter()
        .any(|x| has_word(&w, x))
    {
        return false;
    }
    let counting = SCHED_COUNTING.is_match(&low);
    if !counting && (SCHED_SET_UP.is_match(&low) || (SCHED_EVERY.is_match(&low) && !text.contains('?'))) {
        return false;
    }
    let about_runs = SCHED_ABOUT_RUNS.is_match(&low)
        || (_any(&w, &["recurring", "upcoming"], 0.7) && _any(&w, &["runs", "run", "ruuns"], 0.7));
    let about_automation = _any(
        &w,
        &[
            "workflow",
            "workflows",
            "schedules",
            "scheduled",
            "recurring",
            "automatic",
            "automatically",
            "timer",
            "upcoming",
        ],
        0.7,
    ) || low.contains("on repeat")
        || low.contains("what do i have");
    about_runs && about_automation
}

static IMG_THE_ONE: LazyLock<PyRe> = LazyLock::new(|| re(r"\bthe one you (?:just )?\w+"));
static IMG_EARLIER: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(from earlier|from before|you (?:just )?(?:g\w{4,9}d|m\w{1,3}de|cr\w{3,6}d)|the one you (?:just )?\w+|",
        r"(?:that|the) (?:latest|last) \w+)\b",
    ))
});
static IMG_POINTED: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(you (?:just )?(?:g\w{4,9}d|m\w{1,3}de|cr\w{3,6}d)|from earlier|from before|",
        r"(?:the one|it) you (?:just )?\w+)\b",
    ))
});
static IMG_NEW: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(make|create|generate|draw)\s+(?:a|an|me a|me an)\s+(?:new\s+)?(?:image|picture|photo)\b"));

/// A change to an image the user already has (from earlier, the one generated, the latest render), not a new
/// one.
pub fn wants_image_edit(text: &str) -> bool {
    let w = _words(text);
    let low = text.to_lowercase();
    let image =
        _any(&w, &["image", "picture", "photo", "render"], 0.8) || has_word(&w, "pic") || IMG_THE_ONE.is_match(&low);
    let earlier = _any(&w, &["earlier", "previous"], 0.8) || IMG_EARLIER.is_match(&low);
    let pointed = IMG_POINTED.is_match(&low);
    let new = (_any(&w, &["create", "generate", "draw"], 0.7) && !pointed) || IMG_NEW.is_match(&low);
    image && earlier && !new
}

/// Arithmetic or statistics on at least two numbers the request gives.
pub fn wants_arithmetic(text: &str) -> bool {
    let w = _words(text);
    let nums = NUMBER_RE.findall(text);
    let stat = _any(
        &w,
        &[
            "mean",
            "average",
            "total",
            "difference",
            "median",
            "crunch",
            "apart",
            "percent",
            "percentage",
            "range",
            "ratio",
            "normalize",
            "fractions",
            "calculation",
            "compute",
            "times",
            "minus",
            "plus",
            "divided",
            "multiply",
            "multiplied",
            "sum",
            "spread",
        ],
        0.7,
    );
    nums.len() >= 2 && stat
}

static JOB_STATUS: LazyLock<PyRe> = LazyLock::new(|| {
    re(r"\b(status|how's|how is|where do things stand|options|my (?:recent |running |current )?b\w{2,4}k\w*nd)\b")
});
static JOB_START: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(launch|start|kick off|queue up|fire off|spin up|run|delegate|send|have|do)\b"));
static JOB_AGENT: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(a worker|delegate a task|send an? a\w{2,4}t off|kick off an? a\w{2,4}t|",
        r"spin up an? a\w{2,4}t|launch an? (?:a\w{2,4}t|job)|queue up an? (?:task|job)|",
        r"while i keep working)\b",
    ))
});

/// The user instructs a worker, an agent or a background job to do something (not a question about jobs that
/// already exist).
pub fn wants_background_job(text: &str) -> bool {
    let w = _words(text);
    let low = text.to_lowercase();
    if JOB_STATUS.is_match(&low) {
        return false;
    }
    let start = JOB_START.is_match(&low);
    let background = (_any(&w, &["background"], 0.7) && _any(&w, &["job", "task"], 0.7))
        || _any(&w, &["asynchronously", "asynchronous"], 0.7);
    let agent = JOB_AGENT.is_match(&low);
    (background && start) || agent
}

static CAL_IDENT: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(?=[a-z0-9-]*\d)(?=[a-z0-9-]*[a-z])[a-z0-9][a-z0-9-]{7,}\b"));
static CAL_BLOCK: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(block|blpck|bluck|slot|solt)\b"));
static CAL_FOR_ME: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(pencil in|bl\w{2}k out|slot in|book)\b.*\bfor me\b"));

/// A plain request to put something on the calendar (no quoted project name, no case identifier).
pub fn wants_calendar_entry(text: &str) -> bool {
    let low = text.to_lowercase();
    if text.contains('"') || text.contains('\u{201c}') || CAL_IDENT.is_match(&low) {
        return false;
    }
    let w = _words(text);
    let calendar = _any(&w, &["calendar"], 0.6);
    let put = _any(&w, &["put", "add", "schedule", "pencil", "book", "get"], 0.7) || CAL_BLOCK.is_match(&low);
    let for_me = CAL_FOR_ME.is_match(&low);
    (calendar && put) || for_me
}

static SANDBOX: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(sa[a-z]{1,6}x|code mode)\b"));
static SANDBOX_CODE: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(code|in-?\s?pr\w{2,5}ss|checksum|routine)\b"));

/// Run a named routine in the code sandbox / Code Mode and report what it prints.
pub fn wants_sandbox_run(text: &str) -> bool {
    let low = text.to_lowercase();
    SANDBOX.is_match(&low) && SANDBOX_CODE.is_match(&low)
}

static JOBFIG_JOB: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(job list|b\w{2,4}gr?o?u?n?d (?:work|tasks?|jobs?|j\w?bs)|agent jobs?|agents i set loose|",
        r"jobs? i (?:dispatched|queued|started)|(?:crawl|backfill|reindex|import|export|sync|migration|batch|",
        r"scrape) job)\b",
    ))
});
static JOBFIG_START: LazyLock<PyRe> = LazyLock::new(|| {
    re(r"\b(start|launch|kick off|run|queue up|spin up|fire off|delegate|turn off|turn on|prefer\w*)\b")
});

/// A figure a background/agent job reports (items processed, a count), read off the job list; not starting a job,
/// not a status check.
pub fn wants_job_figure(text: &str) -> bool {
    let low = text.to_lowercase();
    let w = _words(text);
    let job = JOBFIG_JOB.is_match(&low);
    let figure = _any(
        &w,
        &[
            "items",
            "count",
            "many",
            "number",
            "processed",
            "process",
            "through",
            "reports",
        ],
        0.75,
    );
    let start = JOBFIG_START.is_match(&low);
    job && figure && !start
}

static NOTE_NOTE: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(note|scr\w{2,5}pa?d|handoff)\b"));
static NOTE_FIX: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(fix the note|correct what|bin the old|make it \w+day|moved to|shifted to|scratch that|is \w+day now|",
        r"rather than adding|instead of piling|update the note|change the note|forget \w+day|handoff is \w+day|",
        r"keep that in the)\b",
    ))
});

/// Correct a stored note in place (a handoff or scratchpad note whose fact changed).
pub fn wants_note_fix(text: &str) -> bool {
    let low = text.to_lowercase();
    NOTE_NOTE.is_match(&low) && NOTE_FIX.is_match(&low)
}

static TEAM_FEEDBACK: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(let the t\w{2,3}m know|report (this |it )?to the (d\w{1,2}vs?|developers?|t\w{2,3}m)",
        r"|to the ditto t\w{2,3}m|whoever (builds|makes) (this|it|t\w{2,3})|tell the (devs?|developers?|ditto t\w{2,3}m))\b",
    ))
});

/// Feedback meant for the people who build Ditto ("let the team know …", "report to the devs …", "send this to the
/// Ditto team", "forward this to whoever builds this").
pub fn wants_team_feedback(text: &str) -> bool {
    TEAM_FEEDBACK.is_match(&text.to_lowercase())
}

static CHAT_TOOL_PREFS: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(chat t\w{2,4}ls|(t\w{2}n (off|on)|disable|enable|allow only|switch (off|on)) (the )?",
        r"(agent j\w{2,3}s|image t\w{2,4}ls|memory t\w{2,4}ls|web (search|t\w{2,4}ls)|\w+ t\w{2,4}ls))\b",
    ))
});

/// Which kinds of tools the assistant may use in chat ("turn off agent jobs", "allow only memory tools", "change my
/// chat tools so …").
pub fn wants_chat_tool_prefs(text: &str) -> bool {
    CHAT_TOOL_PREFS.is_match(&text.to_lowercase())
}

static CAPABILITY_QUESTION: LazyLock<PyRe> = LazyLock::new(|| {
    re(
        r"\b(is th\w{2,4} a way to|where (do|can) i find the opt\w{2,4}|how (do|can) i (turn on|set up|switch|enable))\b",
    )
});

/// A question about whether or where Ditto can do something ("is there a way to …", "where do I find the option
/// for …", "how do I turn on …").
pub fn asks_capability(text: &str) -> bool {
    CAPABILITY_QUESTION.is_match(&text.to_lowercase())
}

static JUST_TELL_ME: LazyLock<PyRe> = LazyLock::new(|| re(r"\bdon'?t just tell me how\b"));

/// A piece of software work the user wants done rather than explained: "go ahead and refactor …", "write a script to
/// … — start it", "actually scrape …", "don't just tell me how" (the work verbs tolerate typos).
pub fn wants_work_done(text: &str) -> bool {
    let w = _words(text);
    let low = text.to_lowercase();
    let work_verb = |x: &String| _any(std::slice::from_ref(x), &["refactor", "scrape", "build", "write"], 0.7);
    let go_ahead =
        (3..w.len()).any(|i| w[i - 3] == "go" && w[i - 2] == "ahead" && w[i - 1] == "and" && work_verb(&w[i]));
    let actually = (1..w.len()).any(|i| w[i - 1] == "actually" && work_verb(&w[i]));
    let start_it = low.contains("start it")
        && (has_word(&w, "write") || has_word(&w, "build"))
        && _any(&w, &["script", "tool", "app"], 0.7);
    go_ahead || actually || start_it || JUST_TELL_ME.is_match(&low)
}

pub type Rule = (&'static str, fn(&str) -> bool);

pub const RULES: &[Rule] = &[
    ("list_schedules", wants_schedules),
    ("edit_image", wants_image_edit),
    ("run_code", wants_arithmetic),
    ("execute_agent_job", wants_background_job),
    ("calendar_create_event", wants_calendar_entry),
    ("run_code", wants_sandbox_run),
    ("list_agent_jobs", wants_job_figure),
    ("update_memory", wants_note_fix),
    ("file_feedback_for_team", wants_team_feedback),
    ("set_chat_tool_preferences", wants_chat_tool_prefs),
    ("discover_capabilities", asks_capability),
    ("execute_agent_job", wants_work_done),
];

pub const RULE_EXTRA: &[(&str, &str)] = &[
    (
        "run_code",
        " Call it directly, without searching the records first: pass the routine the user names as the task to run \
         and report exactly the value it prints.",
    ),
    (
        "update_memory",
        " Find that note with the memory search, then update the same note in place with the corrected fact (keep the \
         rest of its text); do not add a separate note.",
    ),
    (
        "edit_image",
        " Pass the image as the user refers to it (for example \"the image from earlier\") with the edit as the \
         instruction; do not search the records for it or ask for a link.",
    ),
    (
        "file_feedback_for_team",
        " Pass the user's own words as the feedback; it reaches the team without an address, so do not ask for one.",
    ),
    (
        "discover_capabilities",
        " Answer the question from what it reports; the user asked whether or where, so do not change the setting \
         yourself.",
    ),
    (
        "calendar_create_event",
        " Use the title the user gave and the timing they gave; when they gave none, pass the timing as unspecified \
         instead of asking first.",
    ),
];

/// `(t.get("function") or {}).get(key)` of one tool definition.
fn function_field<'a>(t: &'a Value, key: &str) -> Option<&'a Value> {
    t.get("function")
        .filter(|f| crate::store::truthy(f))
        .and_then(|f| f.get(key))
}

/// The offered tool names, `{(t.get("function") or {}).get("name") for t in tool_defs}` (string names only).
fn offered_names(tool_defs: &[Value]) -> Vec<&str> {
    tool_defs
        .iter()
        .filter_map(|t| function_field(t, "name").and_then(Value::as_str))
        .collect()
}

/// TOOL_HINT_NOTE for the first rule whose tool is offered and whose request test holds; empty otherwise.
pub fn tool_hint_note(user_input: &str, tool_defs: &[Value]) -> String {
    // name -> description; a later definition of the same name replaces the description. A name or description
    // that is not a string never matches a rule's tool (Python would raise on a non-string description).
    let mut offered: IndexMap<&str, &str> = IndexMap::new();
    for t in tool_defs {
        let name = function_field(t, "name").and_then(Value::as_str).unwrap_or("");
        let desc = function_field(t, "description").and_then(Value::as_str).unwrap_or("");
        offered.insert(name, desc);
    }
    for (tool, test) in RULES {
        if let Some(desc) = offered.get(tool) {
            if test(user_input) {
                let desc = py::rstrip_chars(py::strip(desc), ".");
                let extra = RULE_EXTRA.iter().find(|(t, _)| t == tool).map_or("", |(_, e)| e);
                return TOOL_HINT_NOTE
                    .replace("{tool}", tool)
                    .replacen("{desc}", py::head(desc, 200), 1)
                    + extra;
            }
        }
    }
    String::new()
}

pub const RESEARCH_SEND_NOTE: &str = "This request has two parts: first look up the current figure it asks about \
({lookup} when no other offered tool reports it), then send that figure to the person it names with {send}, using \
their email from the records. Do both, and in your reply state the figure you sent and to whom.";

static RS_WHICH: LazyLock<PyRe> = LazyLock::new(|| re(r"\s*(?:for \w+ contact l\w+:\s*)?w\w{1,3}ch\b"));
static RS_BELONGS: LazyLock<PyRe> = LazyLock::new(|| re(r"b\w{1,3}ongs to|email do i have"));
static RS_FIGURE: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\bat (?:r\w{2,4}t now|today|as of now)|sit\w{0,4}ng at|set\w{0,4}ng at"));
static RS_LOOKUP: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(l\w{1,3}k up|c\w?h?e?c?k|pu\w?ll?|find)\b"));
static RS_UPDATE: LazyLock<PyRe> = LazyLock::new(|| re(r"\bupdate\b"));
static RS_SEND: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b(get (?:it|the \w+) over|s\w{1,2}nt to|s[ne]\w{0,2}t to|e-mail|shoot it|p\w{0,2}s{1,2} (?:it|the)|",
        r"email what)\b",
    ))
});
static RS_RECIPIENT: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(?:to|over to|on to|a\w{2,4}g to) (?:[A-Z][a-z]+|th\w{0,2}m|t\w{1,3}m)\b"));
static RS_FOR_NAME: LazyLock<PyRe> = LazyLock::new(|| re("\\s*For [A-Z][a-z]+ [\u{2014}-]"));

/// Get a current figure (look it up) and send it to a person, in one request (not a question about an address,
/// not sending an update about something).
pub fn wants_research_send(text: &str) -> bool {
    let low = text.to_lowercase();
    let w = _words(text);
    if RS_WHICH.match_start(&low).is_some() || RS_BELONGS.is_match(&low) {
        return false;
    }
    let figure = _any(
        &w,
        &[
            "figure",
            "number",
            "reading",
            "value",
            "latest",
            "current",
            "currently",
            "stands",
        ],
        0.7,
    ) || RS_FIGURE.is_match(&low);
    let lookup = RS_LOOKUP.is_match(&low);
    if !figure && RS_UPDATE.is_match(&low) {
        return false;
    }
    let send = _any(&w, &["send", "email", "forward", "pass"], 0.7) || RS_SEND.is_match(&low);
    let recipient = RS_RECIPIENT.is_match(text) || RS_FOR_NAME.match_start(text).is_some();
    (figure || lookup) && send && recipient
}

/// RESEARCH_SEND_NOTE when the request asks for a looked-up figure to be sent and both tools are offered.
pub fn research_send_note(user_input: &str, tool_defs: &[Value]) -> String {
    let offered = offered_names(tool_defs);
    if offered.contains(&"gmail_send") && offered.contains(&"search_web") && wants_research_send(user_input) {
        return RESEARCH_SEND_NOTE
            .replace("{lookup}", "search_web")
            .replace("{send}", "gmail_send");
    }
    String::new()
}

pub const EFFORT_NOTE: &str = "The user asks for their usual reasoning effort (how hard you think) without naming a \
level. The records for \
this request say what it is: if they state a default level, set that level with set_reasoning_effort; if they say no \
default has been chosen, do not set one: ask which level the user wants, and in the question name the two options the \
record for this request describes them going back and forth between, in that record's own words (not the tool's level \
names).";
