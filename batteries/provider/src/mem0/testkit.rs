use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use gmr_content::ContentError;
use gmr_probe::Budget;

use super::{Deployment, Mem0, Scope};
use crate::http::{Answer, Http};

type Held = Arc<Mutex<BTreeMap<String, String>>>;

pub struct Memories {
    held: Held,
}

impl Default for Memories {
    fn default() -> Self {
        Self::new()
    }
}

impl Memories {
    pub fn new() -> Self {
        Self {
            held: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn holds(&self, id: &str, text: &str) {
        self.held
            .lock()
            .expect("the fake's map is never held across a panic")
            .insert(id.to_owned(), text.to_owned());
    }

    pub fn provider(&self) -> Mem0 {
        Mem0::faked(
            Box::new(Serving {
                held: Arc::clone(&self.held),
            }),
            Scope::user("u1"),
            Deployment::Platform,
        )
    }

    pub fn out_of_reach(&self) -> Mem0 {
        Mem0::faked(Box::new(Silent), Scope::user("u1"), Deployment::Platform)
    }
}

struct Serving {
    held: Held,
}

struct Silent;

fn answered(body: String) -> Answer {
    Answer { status: 200, body }
}

fn missing() -> Answer {
    Answer {
        status: 404,
        body: r#"{"detail":"not found"}"#.to_owned(),
    }
}

fn memory(id: &str, text: &str) -> String {
    serde_json::json!({ "id": id, "memory": text }).to_string()
}

#[async_trait]
impl Http for Serving {
    async fn get(&self, url: &str, _budget: &Budget) -> Result<Answer, ContentError> {
        let path = url.split_once("mem0.test").map_or(url, |(_, rest)| rest);
        let held = self
            .held
            .lock()
            .expect("the fake's map is never held across a panic");

        if path.starts_with("/v1/memories/?") {
            let all: Vec<String> = held.iter().map(|(id, text)| memory(id, text)).collect();
            return Ok(answered(format!("[{}]", all.join(","))));
        }
        let Some(rest) = path.strip_prefix("/v1/memories/") else {
            return Err(ContentError::new(format!(
                "this fake mem0 has no route for `{url}`"
            )));
        };
        if let Some(id) = rest.strip_suffix("/history/") {
            return Ok(match held.contains_key(id) {
                true => answered("[]".to_owned()),
                false => missing(),
            });
        }
        let id = rest.trim_end_matches('/');
        Ok(match held.get(id) {
            Some(text) => answered(memory(id, text)),
            None => missing(),
        })
    }
}

#[async_trait]
impl Http for Silent {
    async fn get(&self, _url: &str, _budget: &Budget) -> Result<Answer, ContentError> {
        Err(ContentError::new("connection reset by peer"))
    }
}
