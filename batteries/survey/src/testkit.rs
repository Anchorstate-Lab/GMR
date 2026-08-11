//! The reference every backend is measured against: the index with nothing
//! underneath it but a map.
//!
//! Not a deployment option — it does not survive the process. Its job is to be
//! obviously right, so that a conformance run disagreeing with it is evidence
//! about the backend and not about this.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::index::{Built, Generation, Index, IndexError, Indexed, Located, Row, touched, under};
use crate::matching::Want;

struct Kept {
    hash: String,
    sort: String,
    rows: Vec<Row>,
}

#[derive(Default)]
struct Held {
    opened: BTreeMap<Generation, Option<DateTime<Utc>>>,
    files: BTreeMap<(Generation, String), Kept>,
}

#[derive(Default)]
pub struct Remembered {
    held: Mutex<Held>,
}

impl Remembered {
    pub fn new() -> Self {
        Self::default()
    }
}

fn guard(held: &Mutex<Held>) -> std::sync::MutexGuard<'_, Held> {
    held.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ordered(held: &Held, of: &Generation, root: &str) -> Vec<(String, String, Row)> {
    let mut out: Vec<(String, String, Row)> = held
        .files
        .iter()
        .filter(|((which, rel), _)| which == of && under(rel, root))
        .flat_map(|((_, rel), kept)| {
            kept.rows
                .iter()
                .map(|r| (kept.sort.clone(), rel.clone(), r.clone()))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.ord.cmp(&b.2.ord)));
    out
}

fn located(rows: Vec<(String, String, Row)>) -> Vec<Located> {
    rows.into_iter()
        .map(|(_, rel, row)| Located { rel, row })
        .collect()
}

#[async_trait]
impl Index for Remembered {
    async fn built(&self, of: &Generation) -> Result<Option<Built>, IndexError> {
        let held = guard(&self.held);
        let Some(sealed_at) = held.opened.get(of) else {
            return Ok(None);
        };
        let mine = || held.files.iter().filter(|((which, _), _)| which == of);
        Ok(Some(Built {
            files: mine().count() as u64,
            rows: mine().map(|(_, kept)| kept.rows.len() as u64).sum(),
            sealed_at: *sealed_at,
        }))
    }

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, String>, IndexError> {
        let held = guard(&self.held);
        Ok(held
            .files
            .iter()
            .filter(|((which, _), _)| which == of)
            .map(|((_, rel), kept)| (rel.clone(), kept.hash.clone()))
            .collect())
    }

    async fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError> {
        let mut held = guard(&self.held);
        held.opened.insert(of.clone(), None);
        for file in files {
            held.files.insert(
                (of.clone(), file.rel.clone()),
                Kept {
                    hash: file.hash.clone(),
                    sort: file.sort.clone(),
                    rows: file.rows.clone(),
                },
            );
        }
        Ok(())
    }

    async fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError> {
        let mut held = guard(&self.held);
        for rel in gone {
            held.files.remove(&(of.clone(), rel.clone()));
        }
        Ok(())
    }

    async fn seal(&self, of: &Generation, at: DateTime<Utc>) -> Result<(), IndexError> {
        let mut held = guard(&self.held);
        match held.opened.get_mut(of) {
            Some(sealed_at) => {
                *sealed_at = Some(at);
                Ok(())
            }
            None => Err(IndexError::unopened(of)),
        }
    }

    async fn generations(&self) -> Result<Vec<(Generation, Built)>, IndexError> {
        let known: Vec<Generation> = guard(&self.held).opened.keys().cloned().collect();
        let mut out = Vec::new();
        for which in known {
            if let Some(built) = self.built(&which).await? {
                out.push((which, built));
            }
        }
        Ok(out)
    }

    async fn discard(&self, of: &Generation) -> Result<(), IndexError> {
        let mut held = guard(&self.held);
        held.opened.remove(of);
        held.files.retain(|(which, _), _| which != of);
        Ok(())
    }

    async fn rows(&self, of: &Generation, root: &str) -> Result<Vec<Located>, IndexError> {
        let held = guard(&self.held);
        Ok(located(ordered(&held, of, root)))
    }

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Vec<Located>, IndexError> {
        let held = guard(&self.held);
        let kept: Vec<_> = ordered(&held, of, root)
            .into_iter()
            .filter(|(_, _, row)| touched(row, want))
            .collect();
        Ok(located(kept))
    }
}
