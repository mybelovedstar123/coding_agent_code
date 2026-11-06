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
