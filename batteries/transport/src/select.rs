use gmr_core::{Facts, Outcome};
use serde_json::Value;

pub const VALUE: &str = "value";

pub fn pointer(select: &str) -> String {
    let path = select.trim_start_matches('$').trim_start_matches('.');
    match path.starts_with('/') {
        true => path.to_owned(),
        false => path
            .split('.')
            .filter(|s| !s.is_empty())
            .map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1")))
            .collect(),
    }
}

pub fn pick(body: &Value, select: Option<&str>) -> Outcome {
    let Some(select) = select else {
        return found(body.clone());
    };
    match body.pointer(&pointer(select)) {
        Some(picked) => found(picked.clone()),
        None => Outcome::NotFound,
    }
}

fn found(value: Value) -> Outcome {
    match value.is_null() {
        true => Outcome::NotFound,
        false => Outcome::Found {
            facts: Facts::new(serde_json::json!({ VALUE: value })),
        },
    }
}
