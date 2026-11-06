//! Shared text utilities.

use crate::pyre::Re;
use std::sync::LazyLock;

static TOKEN_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z0-9]+(?:[.'@_-][a-z0-9]+)*"));
