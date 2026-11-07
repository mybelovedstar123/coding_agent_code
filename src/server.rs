//! HTTP server: GET /health, POST /seed, POST /run on port 8080.

use crate::agent::Agent;
use crate::config;
use crate::embed::{Embed, Embedder};
use crate::llm::{ChatClient, HttpChat};
use crate::store::{truthy, Store};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct App {
    pub store: Arc<Store>,
    pub agent: Arc<Agent>,
}

impl App {
    /// The production wiring: the embedding gateway and the inference relay over one HTTP client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(128)
            .build()
            .expect("http client");
        let embedder: Arc<dyn Embed> = Arc::new(Embedder::new(http.clone()));
        let chat: Arc<dyn ChatClient> = Arc::new(HttpChat::new(http.clone()));
        Self::with(embedder, chat, http)
    }

    /// Wiring with injected dependencies (tests and the equivalence harness use fakes).
    pub fn with(embedder: Arc<dyn Embed>, chat: Arc<dyn ChatClient>, http: reqwest::Client) -> Self {
        let store = Arc::new(Store::new(embedder));
        let agent = Arc::new(Agent::new(store.clone(), chat, http));
        App { store, agent }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn error(status: StatusCode, message: String) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

/// The request body as a JSON object, or the reference's 400 response.
fn object_body(raw: &[u8]) -> Result<serde_json::Map<String, Value>, Box<Response>> {
    match serde_json::from_slice::<Value>(raw) {
        Err(_) => Err(Box::new(error(StatusCode::BAD_REQUEST, "invalid JSON".into()))),
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(Box::new(error(
            StatusCode::BAD_REQUEST,
            "the body must be a JSON object".into(),
        ))),
    }
}

/// `len(x)` of the seed collections as the reference reports them.
fn py_len(v: &Value) -> usize {
    match v {
        Value::Array(a) => a.len(),
        Value::Object(o) => o.len(),
        Value::String(s) => crate::py::len(s),
        _ => 0,
    }
}

async fn health() -> Response {
    Json(json!({"status": "ok"})).into_response()
}

async fn seed(State(app): State<App>, body: axum::body::Bytes) -> Response {
    let body = match object_body(&body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    let field = |k: &str| body.get(k).filter(|v| truthy(v)).cloned().unwrap_or(json!([]));
    let (pairs, subjects, links) = (field("pairs"), field("subjects"), field("links"));
    let user_id = body
        .get("user_id")
        .filter(|v| truthy(v))
        .map(crate::py::str_value)
        .unwrap_or_else(|| config::DEFAULT_USER.to_string());
    if let Err(err) = app.store.seed(&user_id, &pairs, &subjects, &links).await {
        // The scorer retries a failed wave; report why.
        tracing::error!("seed failed: {err}");
        return error(StatusCode::SERVICE_UNAVAILABLE, format!("seed failed: {err}"));
    }
    Json(json!({"pairs": py_len(&pairs), "subjects": py_len(&subjects), "links": py_len(&links)})).into_response()
}

async fn run(State(app): State<App>, body: axum::body::Bytes) -> Response {
    let body = match object_body(&body) {
        Ok(b) => b,
        Err(resp) => return *resp,
    };
    let case_id = body.get("case_id").map(crate::py::str_value).unwrap_or_default();
    // A failure inside the case is contained here: the case answers 500 with the reason, the server keeps serving.
    match tokio::spawn(app.agent.clone().run(body)).await {
        Ok(result) => Json(result).into_response(),
        Err(err) => {
            tracing::error!("run {case_id} failed: {err}");
            error(StatusCode::INTERNAL_SERVER_ERROR, format!("run failed: {err}"))
        }
    }
}
