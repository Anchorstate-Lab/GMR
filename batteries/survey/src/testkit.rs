use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::index::{Built, Generation, Index, IndexError, Indexed, Located, Row, Snapshot, under};
use crate::matching::Want;
use crate::narrow::touches;
use gmr_probe as _;

struct Kept {
    hash: String,
    sort: String,
    stamp: Option<crate::walk::Stamp>,
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

    async fn known(&self, of: &Generation) -> Result<BTreeMap<String, crate::walk::Held>, IndexError> {
        let held = guard(&self.held);
        Ok(held
            .files
            .iter()
            .filter(|((which, _), _)| which == of)
            .map(|((_, rel), kept)| {
                (
                    rel.clone(),
                    crate::walk::Held {
                        hash: kept.hash.clone(),
                        stamp: kept.stamp,
                    },
                )
            })
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
                    stamp: file.stamp,
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

    async fn rows(&self, of: &Generation, root: &str) -> Result<Option<Snapshot>, IndexError> {
        let held = guard(&self.held);
        Ok(held.opened.get(of).map(|sealed_at| Snapshot {
            sealed_at: *sealed_at,
            rows: located(ordered(&held, of, root)),
        }))
    }

    async fn union(
        &self,
        of: &Generation,
        root: &str,
        want: &Want,
    ) -> Result<Option<Snapshot>, IndexError> {
        let held = guard(&self.held);
        Ok(held.opened.get(of).map(|sealed_at| Snapshot {
            sealed_at: *sealed_at,
            rows: located(
                ordered(&held, of, root)
                    .into_iter()
                    .filter(|(_, _, row)| touches(&row.coord, want))
                    .collect(),
            ),
        }))
    }
}

#[derive(Default)]
pub struct Surveyed {
    tree: std::path::PathBuf,
    held: Mutex<BTreeMap<String, (crate::walk::Held, Vec<crate::matching::Fragment>)>>,
}

impl Surveyed {
    pub fn over(tree: impl Into<std::path::PathBuf>) -> Self {
        Self {
            tree: tree.into(),
            held: Mutex::new(BTreeMap::new()),
        }
    }
}

impl crate::corpus::Corpus for Surveyed {
    fn refresh(
        &self,
        recipe: &crate::recipe::Recipe,
        budget: &gmr_probe::Budget,
    ) -> Result<(), crate::corpus::Halt> {
        let known: BTreeMap<String, crate::walk::Held> = {
            let held = self.held.lock().unwrap();
            held.iter().map(|(k, (h, _))| (k.clone(), h.clone())).collect()
        };
        let scan = crate::corpus::rescan(&self.tree, recipe, &known, budget)?;
        let mut held = self.held.lock().unwrap();
        for rel in scan.gone {
            held.remove(&rel);
        }
        for (rel, stamp) in scan.restamped {
            if let Some((h, _)) = held.get_mut(&rel) {
                h.stamp = stamp;
            }
        }
        for fresh in scan.fresh {
            held.insert(
                fresh.rel,
                (
                    crate::walk::Held {
                        hash: fresh.hash,
                        stamp: fresh.stamp,
                    },
                    fresh.fragments,
                ),
            );
        }
        Ok(())
    }

    fn populated(
        &self,
        _recipe: &crate::recipe::Recipe,
        root: &str,
    ) -> Result<bool, crate::corpus::Halt> {
        let held = self.held.lock().unwrap();
        Ok(held
            .iter()
            .any(|(rel, (_, f))| under(rel, root) && !f.is_empty()))
    }

    fn whole(
        &self,
        _recipe: &crate::recipe::Recipe,
        root: &str,
    ) -> Result<Vec<crate::matching::Fragment>, crate::corpus::Halt> {
        let held = self.held.lock().unwrap();
        let mut rows: Vec<(String, Vec<crate::matching::Fragment>)> = held
            .iter()
            .filter(|(rel, _)| under(rel, root))
            .map(|(rel, (_, f))| (crate::walk::sort_key(rel), f.clone()))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(rows.into_iter().flat_map(|(_, f)| f).collect())
    }

    fn touching(
        &self,
        recipe: &crate::recipe::Recipe,
        root: &str,
        want: &Want,
    ) -> Result<Vec<crate::matching::Fragment>, crate::corpus::Halt> {
        Ok(self
            .whole(recipe, root)?
            .into_iter()
            .filter(|f| touches(&f.coord, want))
            .collect())
    }
}
