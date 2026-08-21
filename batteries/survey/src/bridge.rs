use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Mutex;

use gmr_probe::Budget;

use crate::corpus::{self, Corpus, Halt};
use crate::index::{Generation, Index, IndexError, Indexed, Row as IndexRow, Snapshot};
use crate::matching::{Fragment, Want};
use crate::recipe::Recipe;

fn index_halt(e: IndexError) -> Halt {
    Halt::Refused(format!("{:?}: {e}", e.fault))
}

fn located_to_fragments(snapshot: Option<Snapshot>) -> Vec<Fragment> {
    snapshot
        .map(|s| {
            s.rows
                .into_iter()
                .map(|l| Fragment {
                    coord: l.row.coord,
                    facts: l.row.facts,
                    id: l.row.id,
                })
                .collect()
        })
        .unwrap_or_default()
}

thread_local! {
    static FALLBACK: tokio::runtime::Runtime = tokio::runtime::Runtime::new()
        .expect("gmr-survey: no ambient tokio runtime and a fallback one would not start");
}

pub fn run_blocking<F: Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(fut),
        Err(_) => FALLBACK.with(|rt| rt.block_on(fut)),
    }
}

const WALK_POISONED: &str = "gmr-survey: a prior walk panicked while holding the memo";

pub struct Bridge<I> {
    tree: PathBuf,
    index: I,
    walked: Option<Mutex<BTreeMap<Generation, Result<(), Halt>>>>,
}

impl<I: Index> Bridge<I> {
    pub async fn open<F, Fut>(tree: impl Into<PathBuf>, open: F) -> Result<Self, IndexError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<I, IndexError>>,
    {
        Ok(Self {
            tree: tree.into(),
            index: open().await?,
            walked: None,
        })
    }

    pub fn over_a_still_tree(mut self) -> Self {
        self.walked = Some(Mutex::new(BTreeMap::new()));
        self
    }

    pub async fn retain(&self, keep: &[Generation]) -> Result<Vec<Generation>, IndexError> {
        let mut dropped = Vec::new();
        for (which, _) in self.index.generations().await? {
            if !keep.contains(&which) {
                self.index.discard(&which).await?;
                dropped.push(which);
            }
        }
        Ok(dropped)
    }
}

impl<I: Index> Bridge<I> {
    fn walk(&self, recipe: &Recipe, of: &Generation, budget: &Budget) -> Result<(), Halt> {
        let known = run_blocking(self.index.known(of)).map_err(index_halt)?;
        let scan = corpus::rescan(&self.tree, recipe, &known, budget)?;

        let files: Vec<Indexed> = scan
            .fresh
            .into_iter()
            .map(|fresh| Indexed {
                rel: fresh.rel,
                hash: fresh.hash,
                sort: fresh.sort,
                stamp: fresh.stamp,
                rows: fresh
                    .fragments
                    .into_iter()
                    .enumerate()
                    .map(|(ord, frag)| IndexRow {
                        ord: ord as u32,
                        id: frag.id,
                        coord: frag.coord,
                        facts: frag.facts,
                    })
                    .collect(),
            })
            .collect();
        run_blocking(self.index.write(of, &files)).map_err(index_halt)?;

        if !scan.restamped.is_empty() {
            run_blocking(self.index.restamp(of, &scan.restamped)).map_err(index_halt)?;
        }
        if !scan.gone.is_empty() {
            run_blocking(self.index.forget(of, &scan.gone)).map_err(index_halt)?;
        }
        Ok(())
    }
}

impl<I: Index> Corpus for Bridge<I> {
    fn refresh(&self, recipe: &Recipe, budget: &Budget) -> Result<(), Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let Some(memo) = &self.walked else {
            return self.walk(recipe, &of, budget);
        };
        if let Some(walked) = memo.lock().expect(WALK_POISONED).get(&of) {
            return walked.clone();
        }
        let walked = self.walk(recipe, &of, budget);
        memo.lock().expect(WALK_POISONED).insert(of, walked.clone());
        walked
    }

    fn populated(&self, recipe: &Recipe, root: &str) -> Result<bool, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let known = run_blocking(self.index.known(&of)).map_err(index_halt)?;
        Ok(known.keys().any(|rel| crate::index::under(rel, root)))
    }

    fn whole(&self, recipe: &Recipe, root: &str) -> Result<Vec<Fragment>, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let snapshot = run_blocking(self.index.rows(&of, root)).map_err(index_halt)?;
        Ok(located_to_fragments(snapshot))
    }

    fn touching(&self, recipe: &Recipe, root: &str, want: &Want) -> Result<Vec<Fragment>, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let snapshot = run_blocking(self.index.union(&of, root, want)).map_err(index_halt)?;
        Ok(located_to_fragments(snapshot))
    }
}
