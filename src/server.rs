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
