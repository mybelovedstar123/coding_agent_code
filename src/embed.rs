//! Ollama-compatible embedding client (local Ollama or the scorer's gateway).

use crate::config;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::Instant;

pub const BATCH: usize = 32;
/// The scored gateway forwards to a provider with a per-request token limit (observed: 120k tokens). Long records
/// (~16k chars each) must not be packed 32 to a request; cap each request by total characters as well.
pub const BATCH_MAX_CHARS: usize = 100_000;
const ATTEMPTS: usize = 8;
const RETRY_STATUS: [u16; 6] = [408, 429, 500, 502, 503, 504];

#[derive(Debug, Clone)]
pub enum EmbedError {
    /// The gateway rejected the request's size (or failed for a multi-text batch): retry with smaller batches.
    TooLarge(String),
    Failed(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::TooLarge(m) | EmbedError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for EmbedError {}

/// Unit vectors for texts; `deadline` bounds the whole call.
pub trait Embed: Send + Sync {
    fn embed<'a>(
        &'a self,
        texts: Vec<String>,
        deadline: Option<Instant>,
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, EmbedError>>;
}

pub struct Embedder {
    http: reqwest::Client,
    /// Bound concurrent embedding requests so a large seed wave cannot starve /run queries of gateway capacity.
    gate: Arc<Semaphore>,
}

impl Embedder {
    pub fn new(http: reqwest::Client) -> Self {
        Embedder {
            http,
            gate: Arc::new(Semaphore::new(4)),
        }
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let url = format!("{}/api/embed", config::ollama_url());
        let body = serde_json::json!({"model": config::EMBED_MODEL, "input": texts});
        let mut delay = 1.0f64;
        let mut last = String::new();
        for _ in 0..ATTEMPTS {
            let attempt = async {
                let _permit = self.gate.acquire().await.expect("semaphore open");
                let resp = self
                    .http
                    .post(&url)
                    .json(&body)
                    .timeout(Duration::from_secs(120))
                    .send()
                    .await?;
                let status = resp.status().as_u16();
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from);
                let text = resp.text().await?;
                Ok::<_, reqwest::Error>((status, retry_after, text))
            };
            match attempt.await {
                Ok((200, _, text)) => {
                    let data: serde_json::Value = serde_json::from_str(&text)
                        .map_err(|e| EmbedError::Failed(format!("bad embedding body: {e}")))?;
                    let vecs = parse_vectors(&data["embeddings"]);
                    if vecs.len() != texts.len() || vecs.iter().any(|v| v.len() != config::EMBED_DIM) {
                        return Err(EmbedError::Failed(format!(
                            "unexpected embedding shape ({}, {})",
                            vecs.len(),
                            vecs.first().map(|v| v.len()).unwrap_or(0)
                        )));
                    }
                    return Ok(vecs.into_iter().map(normalize).collect());
                }
                Ok((status, retry_after, text)) => {
                    last = format!("{status}: {}", crate::py::head(&text, 200));
                    if status == 400 || status == 413 {
                        return Err(EmbedError::TooLarge(last));
                    }
                    if !RETRY_STATUS.contains(&status) {
                        break;
                    }
                    if let Some(ra) = retry_after.and_then(|v| v.trim().parse::<f64>().ok()) {
                        delay = delay.max(ra);
                    }
                }
                Err(err) => last = format!("{err:?}"),
            }
            tokio::time::sleep(Duration::from_secs_f64(delay)).await;
            delay = (delay * 2.0).min(15.0);
        }
        // A gateway that hides the provider's size error behind a 5xx still gets one chance to succeed with smaller
        // requests.
        if texts.len() > 1 {
            return Err(EmbedError::TooLarge(format!("embed failed after retries: {last}")));
        }
        Err(EmbedError::Failed(format!("embed failed: {last}")))
    }

    fn embed_adaptive<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Vec<Vec<f32>>, EmbedError>> {
        Box::pin(async move {
            match self.embed_batch(texts).await {
                Err(EmbedError::TooLarge(m)) => {
                    if texts.len() == 1 {
                        return Err(EmbedError::TooLarge(m));
                    }
                    let mid = texts.len() / 2;
                    let (left, right) =
                        futures::try_join!(self.embed_adaptive(&texts[..mid]), self.embed_adaptive(&texts[mid..]))?;
                    Ok(left.into_iter().chain(right).collect())
                }
                other => other,
            }
        })
    }
}

impl Embed for Embedder {
    fn embed<'a>(
        &'a self,
        texts: Vec<String>,
        deadline: Option<Instant>,
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, EmbedError>> {
        Box::pin(async move {
            if texts.is_empty() {
                return Ok(Vec::new());
            }
            // Empty strings make some embedders fail; embed a placeholder instead.
            let clean: Vec<String> = texts
                .into_iter()
                .map(|t| {
                    if crate::py::strip(&t).is_empty() {
                        "(empty)".into()
                    } else {
                        t
                    }
                })
                .collect();
            let batches = batches(&clean);
            let work = futures::future::try_join_all(batches.iter().map(|b| self.embed_adaptive(b)));
            let parts = match deadline {
                None => work.await?,
                Some(d) => {
                    let left = d
                        .saturating_duration_since(Instant::now())
                        .max(Duration::from_millis(500));
                    tokio::time::timeout(left, work)
                        .await
                        .map_err(|_| EmbedError::Failed("embedding did not finish within the case budget".into()))??
                }
            };
            Ok(parts.into_iter().flatten().collect())
        })
    }
}

/// Group texts in order: at most BATCH texts and BATCH_MAX_CHARS characters each.
pub fn batches(texts: &[String]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut size = 0usize;
    for t in texts {
        let tl = crate::py::len(t);
        if !cur.is_empty() && (cur.len() >= BATCH || size + tl > BATCH_MAX_CHARS) {
            out.push(std::mem::take(&mut cur));
            size = 0;
        }
        cur.push(t.clone());
        size += tl;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_vectors(v: &serde_json::Value) -> Vec<Vec<f32>> {
    v.as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    r.as_array()
                        .map(|xs| xs.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Unit length in float32, as numpy does (`vecs / norm`, a zero norm left at 1).
pub fn normalize(v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm = if norm == 0.0 { 1.0 } else { norm };
    v.into_iter().map(|x| x / norm).collect()
}
