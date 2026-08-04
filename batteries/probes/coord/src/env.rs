use std::process::ExitCode;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub use gmr_probe::{PARAMS_ENV, POSITION_ENV};

pub fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub fn position() -> Result<Value, String> {
    let raw = std::env::var(POSITION_ENV).unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|e| format!("{POSITION_ENV} is not JSON: {e}"))
}

pub fn params() -> Result<Value, String> {
    let raw = std::env::var(PARAMS_ENV).unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|e| format!("{PARAMS_ENV} is not JSON: {e}"))
}

/// The portion the probe should inspect. This comes from params, not argv:
/// params enter the declaration hash, argv is locked by the manifest, and
/// neither is chosen by the probe at runtime.
pub fn root(params: &Value) -> String {
    params
        .get("root")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .to_owned()
}

/// The exit-code half of the probe's contract: stdout carries the JSON,
/// the process exit status carries our-failure-vs-answer. Returning an
/// `ExitCode` from `main` (instead of calling `process::exit` in here)
/// keeps this a library function a caller could still use for something
/// other than terminating a process.
pub fn emit(result: Result<Value, String>) -> ExitCode {
    match result {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
