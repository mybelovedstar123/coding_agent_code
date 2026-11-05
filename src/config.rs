//! Runtime configuration, read from the environment on every lookup.
//!
//! On-chain the scorer injects the inference relay (per case via `inference_base_url`, or process-wide via
//! `DITTOBENCH_INFERENCE_BASE_URL`) and an Ollama-compatible embedding gateway via `OLLAMA_BASE_URL`. Locally the
//! same code talks to OpenRouter and a local Ollama.

pub const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const EMBED_MODEL: &str = "embeddinggemma";
pub const EMBED_DIM: usize = 768;
/// PROTOCOL.md (/run and /seed): user_id is optional and the documented default memory graph is "miner". A request
/// without user_id therefore belongs to that single default principal (policy v13 W15: "a documented default user
/// represents one default principal"); requests that carry a user_id are always scoped to it.
pub const DEFAULT_USER: &str = "miner";
pub const MEMORY_CONTEXT_PREFIX: &str = "Relevant memory context for this turn:";

/// Minimal .env loader for local practice; never overrides the real environment.
pub fn load_dotenv(path: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').expect("contains '='");
        let key = key.trim();
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value.trim());
        }
    }
}

pub fn env(name: &str, default: &str) -> String {
    let value = std::env::var(name).unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    }
}

pub fn model_name() -> String {
    env("DITTOBENCH_MODEL", DEFAULT_MODEL)
}

pub fn ollama_url() -> String {
    env("OLLAMA_BASE_URL", DEFAULT_OLLAMA_URL)
        .trim_end_matches('/')
        .to_string()
}

/// (base_url, api_key) for chat completions.
pub fn inference_target(case_base_url: Option<&str>) -> (String, String) {
    if let Some(base) = case_base_url.map(str::trim).filter(|b| !b.is_empty()) {
        return (base.trim_end_matches('/').to_string(), "ticket".into());
    }
    let provider = env("DITTOBENCH_PROVIDER", "openrouter").to_lowercase();
    if provider == "platform" || provider == "chutes" {
        let base = Some(env("DITTOBENCH_INFERENCE_BASE_URL", ""))
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| env("CHUTES_BASE_URL", ""));
        if !base.is_empty() {
            return (base.trim_end_matches('/').to_string(), env("CHUTES_API_KEY", "ticket"));
        }
    }
    if provider == "ollama" {
        return (format!("{}/v1", ollama_url()), "ollama".into());
    }
    let base = env("OPENROUTER_BASE_URL", OPENROUTER_URL)
        .trim_end_matches('/')
        .to_string();
    (base, env("OPENROUTER_API_KEY", "ticket"))
}
