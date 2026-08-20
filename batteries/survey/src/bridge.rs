use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use gmr_probe::Budget;
use tokio::runtime::Runtime;

use crate::corpus::{self, Corpus, Halt};
use crate::index::{Generation, Index, IndexError, Indexed, Row as IndexRow, Snapshot};
use crate::matching::{Fragment, Want};
use crate::recipe::Recipe;
use crate::walk::{Held, Stamp};

type Job<I> = Box<dyn FnOnce(&I, &Runtime) + Send>;

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

/// Bridges `Corpus` (sync — what `look()` calls) to `Index` (async — sqlx and
/// friends) with a dedicated background thread that owns its own runtime,
/// rather than reaching into whatever runtime the caller happens to be on.
/// A caller might be a one-shot CLI invocation today, but this is a battery:
/// it must not assume it is always called from inside a multi-thread tokio
/// runtime, or from inside any runtime at all.
///
/// Jobs run through the background thread's own owned `Runtime`, not a
/// cloned `Handle` — `sqlx`'s pool spawns a background maintenance task on
/// `connect`, and only the owning `Runtime::block_on` reliably drives a
/// current-thread runtime's previously-spawned tasks forward. A cloned
/// `Handle::block_on` does not: the first query after connecting hangs
/// forever, waiting on a connection permit only that maintenance task would
/// release. Found by writing a minimal reproduction once the real bridge
/// hung on its first `known()` call — not a hypothetical.
pub struct Bridge<I> {
    tree: PathBuf,
    tx: mpsc::Sender<Job<I>>,
}

impl<I: Index + Send + 'static> Bridge<I> {
    /// `open` runs entirely on the background thread's own runtime — it is
    /// never awaited from the caller's context, so it does not need `Fut:
    /// Send`. `spawn` itself blocks once, synchronously, waiting for that
    /// open to finish; it is a one-time construction cost; unlike a bridge
    /// call, callers do not pay it per anchor.
    pub fn spawn<F, Fut>(tree: impl Into<PathBuf>, open: F) -> Result<Self, IndexError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<I, IndexError>>,
    {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), IndexError>>();
        let (tx, rx) = mpsc::channel::<Job<I>>();
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("gmr-survey: the bridge's own runtime failed to start");
            let index = match rt.block_on(open()) {
                Ok(index) => {
                    let _ = ready_tx.send(Ok(()));
                    index
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            while let Ok(job) = rx.recv() {
                job(&index, &rt);
            }
        });
        ready_rx
            .recv()
            .expect("gmr-survey: the bridge's background thread died before answering")?;
        Ok(Self {
            tree: tree.into(),
            tx,
        })
    }

    fn run<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&I, &Runtime) -> T + Send + 'static,
        T: Send + 'static,
    {
        let (result_tx, result_rx) = mpsc::channel();
        let job: Job<I> = Box::new(move |index, rt| {
            let _ = result_tx.send(f(index, rt));
        });
        self.tx
            .send(job)
            .expect("gmr-survey: the bridge's background thread is gone");
        result_rx
            .recv()
            .expect("gmr-survey: the bridge's background thread dropped a job without answering")
    }

    fn known(&self, of: &Generation) -> Result<BTreeMap<String, Held>, IndexError> {
        let of = of.clone();
        self.run(move |index, rt| rt.block_on(index.known(&of)))
    }

    fn write(&self, of: &Generation, files: &[Indexed]) -> Result<(), IndexError> {
        let of = of.clone();
        let files = files.to_vec();
        self.run(move |index, rt| rt.block_on(index.write(&of, &files)))
    }

    fn restamp(&self, of: &Generation, restamped: &[(String, Option<Stamp>)]) -> Result<(), IndexError> {
        let of = of.clone();
        let restamped = restamped.to_vec();
        self.run(move |index, rt| rt.block_on(index.restamp(&of, &restamped)))
    }

    fn forget(&self, of: &Generation, gone: &[String]) -> Result<(), IndexError> {
        let of = of.clone();
        let gone = gone.to_vec();
        self.run(move |index, rt| rt.block_on(index.forget(&of, &gone)))
    }

    fn rows(&self, of: &Generation, root: &str) -> Result<Option<Snapshot>, IndexError> {
        let of = of.clone();
        let root = root.to_owned();
        self.run(move |index, rt| rt.block_on(index.rows(&of, &root)))
    }

    fn union(&self, of: &Generation, root: &str, want: &Want) -> Result<Option<Snapshot>, IndexError> {
        let of = of.clone();
        let root = root.to_owned();
        let want = want.clone();
        self.run(move |index, rt| rt.block_on(index.union(&of, &root, &want)))
    }
}

impl<I: Index + Send + 'static> Corpus for Bridge<I> {
    fn refresh(&self, recipe: &Recipe, budget: &Budget) -> Result<(), Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let known = self.known(&of).map_err(index_halt)?;
        let scan = corpus::rescan(&self.tree, recipe, &known, budget)?;

        // Always write, even with nothing fresh: this is what opens the
        // generation in the SQLite backend (INSERT INTO generation ...),
        // and a generation that never opens because a directory happens to
        // be empty or wholly ineligible would leave `rows`/`union` reading
        // `None` forever. Still one round trip regardless of file count —
        // the quadratic per-file write survey-cache-write.md measured on
        // the old Cache is exactly what batching into `Indexed` avoids.
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
        self.write(&of, &files).map_err(index_halt)?;

        if !scan.restamped.is_empty() {
            self.restamp(&of, &scan.restamped).map_err(index_halt)?;
        }
        if !scan.gone.is_empty() {
            self.forget(&of, &scan.gone).map_err(index_halt)?;
        }
        Ok(())
    }

    fn populated(&self, recipe: &Recipe, root: &str) -> Result<bool, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let known = self.known(&of).map_err(index_halt)?;
        Ok(known.keys().any(|rel| crate::index::under(rel, root)))
    }

    fn whole(&self, recipe: &Recipe, root: &str) -> Result<Vec<Fragment>, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let snapshot = self.rows(&of, root).map_err(index_halt)?;
        Ok(located_to_fragments(snapshot))
    }

    fn touching(&self, recipe: &Recipe, root: &str, want: &Want) -> Result<Vec<Fragment>, Halt> {
        let of = Generation::of(recipe.name, recipe.version);
        let snapshot = self.union(&of, root, want).map_err(index_halt)?;
        Ok(located_to_fragments(snapshot))
    }
}
