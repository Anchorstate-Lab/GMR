use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gmr_probe::{Budget, Spent};
use serde::{Deserialize, Serialize};

use crate::matching::Candidate;
use crate::walk::{hash, visit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    Spent(Spent),
    Refused(String),
}

impl From<String> for Halt {
    fn from(why: String) -> Self {
        Self::Refused(why)
    }
}

impl std::fmt::Display for Halt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spent(spent) => f.write_str(spent.as_str()),
            Self::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Halt {}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    hash: String,
    candidates: Vec<Candidate>,
}

type ProbeEntries = HashMap<String, Entry>;
type Scoped = HashMap<String, ProbeEntries>;

#[derive(Deserialize)]
#[serde(transparent)]
struct OnDisk(Scoped);

#[derive(Serialize)]
#[serde(transparent)]
struct OnDiskRef<'a>(&'a Scoped);

type Scanned = Result<Arc<Vec<Candidate>>, Halt>;

#[derive(Default)]
struct Flight {
    settled: Mutex<Option<Scanned>>,
    folded: Mutex<Option<Scanned>>,
}

pub struct Cache {
    file: Option<PathBuf>,
    fault: Option<String>,
    stamps: HashMap<String, String>,
    entries: Mutex<Scoped>,
    dirty: AtomicBool,
    writes: AtomicUsize,
    flights: Mutex<HashMap<String, Arc<Flight>>>,
}

impl Cache {
    pub fn load(file: &Path, stamps: HashMap<String, String>) -> Self {
        let (entries, fault) = match std::fs::read_to_string(file) {
            Ok(text) => match serde_json::from_str::<OnDisk>(&text) {
                Ok(on_disk) => (on_disk.0, None),
                Err(e) => (Scoped::new(), Some(unreadable(file, &e.to_string()))),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Scoped::new(), None),
            Err(e) => (Scoped::new(), Some(unreadable(file, &e.to_string()))),
        };
        let entries = entries
            .into_iter()
            .filter(|(scope, _)| current(scope, &stamps))
            .collect();
        Self {
            fault,
            ..Self::held(Some(file.to_owned()), entries, stamps)
        }
    }

    pub fn disabled() -> Self {
        Self::held(None, Scoped::new(), HashMap::new())
    }

    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    fn held(file: Option<PathBuf>, entries: Scoped, stamps: HashMap<String, String>) -> Self {
        Self {
            file,
            fault: None,
            stamps,
            entries: Mutex::new(entries),
            dirty: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
            flights: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, probe: &str, rel: &str, want_hash: &str) -> Option<Vec<Candidate>> {
        let entries = guard(&self.entries);
        let entry = entries.get(probe)?.get(rel)?;
        (entry.hash == want_hash).then(|| entry.candidates.clone())
    }

    fn put(&self, probe: &str, rel: &str, file_hash: &str, candidates: &[Candidate]) {
        let mut entries = guard(&self.entries);
        entries.entry(probe.to_owned()).or_default().insert(
            rel.to_owned(),
            Entry {
                hash: file_hash.to_owned(),
                candidates: candidates.to_vec(),
            },
        );
        self.dirty.store(true, Ordering::SeqCst);
    }

    fn retain(&self, probe: &str, seen: &HashSet<String>) {
        let mut entries = guard(&self.entries);
        let Some(scope) = entries.get_mut(probe) else {
            return;
        };
        let before = scope.len();
        scope.retain(|rel, _| seen.contains(rel));
        if scope.len() != before {
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn persist(&self) -> Result<(), String> {
        let Some(path) = &self.file else {
            return Ok(());
        };
        if !self.dirty.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        self.replace(path).inspect_err(|_| {
            self.dirty.store(true, Ordering::SeqCst);
        })
    }

    fn replace(&self, path: &Path) -> Result<(), String> {
        let json = {
            let entries = guard(&self.entries);
            serde_json::to_string(&OnDiskRef(&entries))
                .map_err(|e| format!("cannot serialise the cache: {e}"))?
        };
        let dir = path
            .parent()
            .ok_or_else(|| format!("{} has no directory to write into", path.display()))?;
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("cache");
        let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
        std::fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("cannot replace {}: {e}", path.display())
        })?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn flight(&self, scope: &str) -> Option<Arc<Flight>> {
        self.file.as_ref()?;
        let mut flights = guard(&self.flights);
        Some(Arc::clone(flights.entry(scope.to_owned()).or_default()))
    }
}

fn unreadable(file: &Path, why: &str) -> String {
    format!(
        "{} could not be read ({why}); it will be rebuilt from scratch, so the next probe pays \
         a full scan and the ones after it will not. Rebuilding without saying so hides the cost",
        file.display()
    )
}

fn current(scope: &str, stamps: &HashMap<String, String>) -> bool {
    let mut parts = scope.splitn(3, '@');
    let (Some(probe), Some(stamp)) = (parts.next(), parts.next()) else {
        return false;
    };
    stamps.get(probe).map(String::as_str) == Some(stamp)
}

pub fn visit_cached(
    root: &Path,
    cache: &Cache,
    probe: &str,
    budget: &Budget,
    collect: impl FnMut(&Path, &str, &mut Vec<Candidate>) -> Result<(), String>,
) -> Scanned {
    let scope = scope_of(cache, probe, root);
    let Some(flight) = cache.flight(&scope) else {
        return scan(root, cache, &scope, budget, collect);
    };
    let mut settled = guard(&flight.settled);
    if let Some(done) = settled.as_ref() {
        return done.clone();
    }
    let scanned = scan(root, cache, &scope, budget, collect);
    *settled = worth_remembering(&scanned);
    scanned
}

pub fn visit_folded(
    root: &Path,
    cache: &Cache,
    probe: &str,
    budget: &Budget,
    collect: impl FnMut(&Path, &str, &mut Vec<Candidate>) -> Result<(), String>,
    fold: impl FnOnce(&[Candidate]) -> Result<Vec<Candidate>, String>,
) -> Scanned {
    let scope = scope_of(cache, probe, root);
    let Some(flight) = cache.flight(&scope) else {
        let fragments = visit_cached(root, cache, probe, budget, collect)?;
        return Ok(Arc::new(fold(&fragments)?));
    };
    let mut folded = guard(&flight.folded);
    if let Some(done) = folded.as_ref() {
        return done.clone();
    }
    let done = visit_cached(root, cache, probe, budget, collect)
        .and_then(|fragments| Ok(fold(&fragments)?))
        .map(Arc::new);
    *folded = worth_remembering(&done);
    done
}

fn worth_remembering(outcome: &Scanned) -> Option<Scanned> {
    match outcome {
        Err(Halt::Spent(_)) => None,
        _ => Some(outcome.clone()),
    }
}

fn scope_of(cache: &Cache, probe: &str, root: &Path) -> String {
    let stamp = cache.stamps.get(probe).map(String::as_str).unwrap_or("");
    format!("{probe}@{stamp}@{}", root.display())
}

fn scan(
    root: &Path,
    cache: &Cache,
    scope: &str,
    budget: &Budget,
    mut collect: impl FnMut(&Path, &str, &mut Vec<Candidate>) -> Result<(), String>,
) -> Scanned {
    let mut cands = Vec::new();
    let mut seen = HashSet::new();
    let mut halted = None;
    visit(root, &mut |p, rel| {
        if let Err(spent) = budget.checkpoint() {
            halted = Some(spent);
            return Err(String::new());
        }
        let Ok(bytes) = std::fs::read(p) else {
            return Ok(());
        };
        seen.insert(rel.to_owned());
        let file_hash = hash(&String::from_utf8_lossy(&bytes));
        if let Some(cached) = cache.get(scope, rel, &file_hash) {
            cands.extend(cached);
            return Ok(());
        }
        let mut fragment = Vec::new();
        collect(p, rel, &mut fragment)?;
        cache.put(scope, rel, &file_hash, &fragment);
        cands.extend(fragment);
        Ok(())
    })
    .map_err(|why| match halted {
        Some(spent) => Halt::Spent(spent),
        None => Halt::Refused(why),
    })?;
    cache.retain(scope, &seen);
    cache.persist()?;
    Ok(Arc::new(cands))
}

fn guard<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roomy() -> Budget {
        Budget::within(std::time::Duration::from_secs(600), 1 << 24)
    }
    use std::collections::BTreeMap;
    use std::sync::atomic::AtomicUsize;

    fn stamped() -> HashMap<String, String> {
        ["p", "ast-map", "addr-map"]
            .into_iter()
            .map(|n| (n.to_owned(), "v1".to_owned()))
            .collect()
    }

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (path, body) in files {
            let p = d.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        d
    }

    fn counting_collect(
        calls: &AtomicUsize,
    ) -> impl FnMut(&Path, &str, &mut Vec<Candidate>) -> Result<(), String> + '_ {
        move |_p, rel, out| {
            calls.fetch_add(1, Ordering::SeqCst);
            out.push(Candidate::new(
                rel.to_owned(),
                BTreeMap::from([("file".to_owned(), rel.to_owned())]),
                serde_json::json!({}),
            ));
            Ok(())
        }
    }

    #[test]
    fn an_unchanged_file_is_not_recollected() {
        let d = tree(&[("a.rs", "one")]);
        let cache = Cache::disabled();
        let calls = AtomicUsize::new(0);

        let first =
            visit_cached(d.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();
        let second =
            visit_cached(d.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn a_changed_file_is_recollected_by_the_next_process() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let first = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &first, "p", &roomy(), counting_collect(&calls)).unwrap();

        std::fs::write(src.path().join("a.rs"), "two").unwrap();
        let second = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &second, "p", &roomy(), counting_collect(&calls)).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_second_lookup_in_the_same_process_does_not_rewalk() {
        let dir = tempfile::tempdir().unwrap();
        let d = tree(&[("a.rs", "one"), ("b.rs", "two")]);
        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        let calls = AtomicUsize::new(0);

        let first =
            visit_cached(d.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "one collect per file");
        assert_eq!(first.len(), 2);

        let second =
            visit_cached(d.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the second lookup reused the first scan, not even a per-file cache check"
        );
        assert_eq!(second.len(), 2);
    }

    #[test]
    fn a_second_probe_does_not_see_the_first_probes_entries() {
        let d = tree(&[("a.rs", "one")]);
        let cache = Cache::disabled();
        let calls = AtomicUsize::new(0);

        visit_cached(
            d.path(),
            &cache,
            "ast-map",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();
        visit_cached(
            d.path(),
            &cache,
            "addr-map",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_same_probe_at_two_different_roots_does_not_cross_contaminate() {
        let a = tree(&[("lib.rs", "one")]);
        let b = tree(&[("lib.rs", "two")]);
        let cache = Cache::disabled();
        let calls = AtomicUsize::new(0);

        let from_a = visit_cached(
            a.path(),
            &cache,
            "ast-map",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();
        let from_b = visit_cached(
            b.path(),
            &cache,
            "ast-map",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "a narrower root (e.g. a layer:: anchor's params.root) must not be served \
             the other root's cached scan just because the probe name matches"
        );
        assert_eq!(from_a.len(), 1);
        assert_eq!(from_b.len(), 1);
    }

    #[test]
    fn a_warm_cache_survives_a_reload_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let warm = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &warm, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let reloaded = Cache::load(&cache_file, stamped());
        visit_cached(
            src.path(),
            &reloaded,
            "p",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a fresh process should still hit"
        );
    }

    fn many(n: usize) -> Vec<(String, String)> {
        (0..n)
            .map(|i| (format!("src/f{i}.rs"), format!("fn f{i}() {{}}")))
            .collect()
    }

    #[test]
    fn a_scan_writes_the_cache_file_once_not_once_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let owned = many(50);
        let files: Vec<(&str, &str)> = owned
            .iter()
            .map(|(p, b)| (p.as_str(), b.as_str()))
            .collect();
        let src = tree(&files);
        let calls = AtomicUsize::new(0);

        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        visit_cached(src.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 50, "one collect per file");
        assert_eq!(
            cache.writes.load(Ordering::SeqCst),
            1,
            "the cache is written once per scan; once per file is the quadratic that \
             made a 800-file repository take 41 seconds"
        );
    }

    #[test]
    fn a_scan_that_changes_nothing_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one"), ("b.rs", "two")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let warm = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &warm, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(warm.writes.load(Ordering::SeqCst), 1);

        let reloaded = Cache::load(&cache_file, stamped());
        visit_cached(
            src.path(),
            &reloaded,
            "p",
            &roomy(),
            counting_collect(&calls),
        )
        .unwrap();
        assert_eq!(
            reloaded.writes.load(Ordering::SeqCst),
            0,
            "nothing changed, so there is nothing to write down"
        );
    }

    #[test]
    fn a_failed_scan_is_not_retried_by_the_next_caller() {
        let dir = tempfile::tempdir().unwrap();
        let d = tree(&[("a.rs", "1"), ("b.rs", "2"), ("c.rs", "3"), ("d.rs", "4")]);
        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        let calls = AtomicUsize::new(0);

        let mut refuse = |_p: &Path, rel: &str, _o: &mut Vec<Candidate>| {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            match n {
                2 => Err(format!("no: {rel}")),
                _ => Ok(()),
            }
        };

        assert!(visit_cached(d.path(), &cache, "p", &roomy(), &mut refuse).is_err());
        let after_first = calls.load(Ordering::SeqCst);

        assert!(visit_cached(d.path(), &cache, "p", &roomy(), &mut refuse).is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            after_first,
            "a scan that already failed is not run again by the next anchor in the same pass; \
             retrying it once per anchor is what pegged a core for six minutes"
        );
    }

    #[test]
    fn a_corrupt_cache_file_is_reported_and_then_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, "{ this is not json").unwrap();

        let cache = Cache::load(&path, stamped());
        let fault = cache
            .fault()
            .expect("a cache file that is not JSON has to be reported, not discarded");
        assert!(fault.contains("cache.json"), "{fault}");
        assert!(
            fault.contains("full scan"),
            "rebuilding without saying so hides what this run is paying: {fault}"
        );
    }

    #[test]
    fn a_corrupt_cache_still_scans_and_leaves_a_readable_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let src = tree(&[("a.rs", "one")]);
        let calls = AtomicUsize::new(0);

        let broken = Cache::load(&path, stamped());
        assert!(broken.fault().is_some());
        visit_cached(src.path(), &broken, "p", &roomy(), counting_collect(&calls)).unwrap();

        let healed = Cache::load(&path, stamped());
        assert_eq!(
            healed.fault(),
            None,
            "one scan replaces the unreadable file, so the cost is paid once and not every run"
        );
        visit_cached(src.path(), &healed, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the rebuilt file has to be a hit, or nothing was actually repaired"
        );
    }

    #[test]
    fn a_cache_file_that_is_not_there_yet_is_not_a_fault() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Cache::load(&dir.path().join("absent.json"), stamped()).fault(),
            None
        );
    }

    #[test]
    fn entries_for_deleted_files_do_not_survive_a_scan() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one"), ("b.rs", "two")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let first = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &first, "p", &roomy(), counting_collect(&calls)).unwrap();

        std::fs::remove_file(src.path().join("b.rs")).unwrap();
        let second = Cache::load(&cache_file, stamped());
        visit_cached(src.path(), &second, "p", &roomy(), counting_collect(&calls)).unwrap();

        let third = Cache::load(&cache_file, stamped());
        let entries = third.entries.lock().unwrap();
        let scope = entries.values().next().expect("one scope was written");
        assert!(scope.contains_key("a.rs"));
        assert!(
            !scope.contains_key("b.rs"),
            "a file that is gone from the tree must go from the cache too, or the file \
             grows for as long as the repository lives"
        );
    }

    fn stamped_as(version: &str) -> HashMap<String, String> {
        [("p".to_owned(), version.to_owned())].into_iter().collect()
    }

    #[test]
    fn a_cache_written_by_one_version_of_a_probe_is_not_served_to_another() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let before = Cache::load(&cache_file, stamped_as("v1"));
        visit_cached(src.path(), &before, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let after = Cache::load(&cache_file, stamped_as("v2"));
        visit_cached(src.path(), &after, "p", &roomy(), counting_collect(&calls)).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the file did not change but the logic that reads it did, so the candidates \
             have to be recomputed — serving the old ones is the probe reporting what a \
             version it no longer is would have said"
        );
    }

    #[test]
    fn entries_a_probe_version_can_no_longer_reach_are_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let before = Cache::load(&cache_file, stamped_as("v1"));
        visit_cached(src.path(), &before, "p", &roomy(), counting_collect(&calls)).unwrap();

        let after = Cache::load(&cache_file, stamped_as("v2"));
        visit_cached(src.path(), &after, "p", &roomy(), counting_collect(&calls)).unwrap();
        let reloaded = Cache::load(&cache_file, stamped_as("v2"));
        assert_eq!(
            reloaded.entries.lock().unwrap().len(),
            1,
            "the superseded version's entries are unreachable, so they are not kept — \
             otherwise the file grows by a whole repository at every upgrade"
        );
    }

    #[test]
    fn a_written_cache_leaves_no_temporary_behind() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let calls = AtomicUsize::new(0);

        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        visit_cached(src.path(), &cache, "p", &roomy(), counting_collect(&calls)).unwrap();

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    fn counting_fold(
        folds: &AtomicUsize,
    ) -> impl FnOnce(&[Candidate]) -> Result<Vec<Candidate>, String> + '_ {
        move |cands| {
            folds.fetch_add(1, Ordering::SeqCst);
            Ok((0..cands.len())
                .map(|i| {
                    Candidate::new(
                        format!("folded {i}"),
                        BTreeMap::new(),
                        serde_json::json!({}),
                    )
                })
                .collect())
        }
    }

    #[test]
    fn an_aggregate_is_folded_once_per_scan_not_once_per_question() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one"), ("b/c.rs", "two")]);
        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        let (calls, folds) = (AtomicUsize::new(0), AtomicUsize::new(0));

        let first = visit_folded(
            src.path(),
            &cache,
            "p",
            &roomy(),
            counting_collect(&calls),
            counting_fold(&folds),
        )
        .unwrap();
        let second = visit_folded(
            src.path(),
            &cache,
            "p",
            &roomy(),
            counting_collect(&calls),
            counting_fold(&folds),
        )
        .unwrap();

        assert!(
            Arc::ptr_eq(&first, &second),
            "the second answer is the first one"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2, "two files, read once each");
        assert_eq!(
            folds.load(Ordering::SeqCst),
            1,
            "a cross-file aggregate is a pure function of the fragments, and the fragments \
             are already settled for this scope. Folding again per question is the whole \
             cost the per-file cache does not remove"
        );
    }

    #[test]
    fn two_roots_do_not_share_a_folded_answer() {
        let dir = tempfile::tempdir().unwrap();
        let one = tree(&[("a.rs", "one")]);
        let two = tree(&[("a.rs", "one"), ("b.rs", "two")]);
        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        let (calls, folds) = (AtomicUsize::new(0), AtomicUsize::new(0));

        let a = visit_folded(
            one.path(),
            &cache,
            "p",
            &roomy(),
            counting_collect(&calls),
            counting_fold(&folds),
        )
        .unwrap();
        let b = visit_folded(
            two.path(),
            &cache,
            "p",
            &roomy(),
            counting_collect(&calls),
            counting_fold(&folds),
        )
        .unwrap();

        assert_eq!(
            (a.len(), b.len()),
            (1, 2),
            "{}",
            "the fold memo is keyed by the same scope the scan is, or narrowing a probe to \
             one directory would be served whatever the first root produced"
        );
        assert_eq!(folds.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_disabled_cache_folds_every_time_it_is_asked() {
        let src = tree(&[("a.rs", "one")]);
        let cache = Cache::disabled();
        let (calls, folds) = (AtomicUsize::new(0), AtomicUsize::new(0));

        for _ in 0..3 {
            visit_folded(
                src.path(),
                &cache,
                "p",
                &roomy(),
                counting_collect(&calls),
                counting_fold(&folds),
            )
            .unwrap();
        }
        assert_eq!(
            folds.load(Ordering::SeqCst),
            3,
            "disabled has to mean disabled: a test that rewrites a file and asks again must \
             see the new answer, and a memo it cannot invalidate would hand back the old one"
        );
    }

    #[test]
    fn a_cold_run_over_several_roots_writes_the_whole_file_once_per_root() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[
            ("a/one.rs", "one"),
            ("a/two.rs", "two"),
            ("b/three.rs", "three"),
            ("c/four.rs", "four"),
        ]);
        let cache = Cache::load(&dir.path().join("cache.json"), stamped());
        let calls = AtomicUsize::new(0);

        for root in ["", "a", "b", "c"] {
            visit_cached(
                &src.path().join(root),
                &cache,
                "p",
                &roomy(),
                counting_collect(&calls),
            )
            .unwrap();
        }

        assert_eq!(
            cache.writes.load(Ordering::SeqCst),
            4,
            "one full-file write per scanned root: the flag is per cache, but so is the \
             serialisation, so making the flag per scope changes nothing here"
        );
    }
}
