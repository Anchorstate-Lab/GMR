use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::matching::Candidate;
use crate::walk::{hash, visit};

#[derive(Clone, Default, Serialize, Deserialize)]
struct Entry {
    hash: String,
    candidates: Vec<Candidate>,
}

type ProbeEntries = HashMap<String, Entry>;

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
struct OnDisk(HashMap<String, ProbeEntries>);

pub struct Cache {
    file: Option<PathBuf>,
    entries: Mutex<HashMap<String, ProbeEntries>>,
    scans: Mutex<HashMap<String, Arc<Vec<Candidate>>>>,
}

impl Cache {
    pub fn load(file: &Path) -> Self {
        let on_disk = std::fs::read_to_string(file)
            .ok()
            .and_then(|s| serde_json::from_str::<OnDisk>(&s).ok())
            .unwrap_or_default();
        Self {
            file: Some(file.to_owned()),
            entries: Mutex::new(on_disk.0),
            scans: Mutex::new(HashMap::new()),
        }
    }

    pub fn disabled() -> Self {
        Self {
            file: None,
            entries: Mutex::new(HashMap::new()),
            scans: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, probe: &str, rel: &str, want_hash: &str) -> Option<Vec<Candidate>> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(probe)?.get(rel)?;
        (entry.hash == want_hash).then(|| entry.candidates.clone())
    }

    fn put(&self, probe: &str, rel: &str, file_hash: &str, candidates: &[Candidate]) {
        let mut entries = self.entries.lock().unwrap();
        entries.entry(probe.to_owned()).or_default().insert(
            rel.to_owned(),
            Entry {
                hash: file_hash.to_owned(),
                candidates: candidates.to_vec(),
            },
        );
        if let Some(path) = &self.file {
            let on_disk = OnDisk(entries.clone());
            drop(entries);
            if let Ok(json) = serde_json::to_string(&on_disk)
                && let Some(dir) = path.parent()
            {
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn scanned(&self, probe: &str) -> Option<Arc<Vec<Candidate>>> {
        self.file.as_ref()?;
        self.scans.lock().unwrap().get(probe).cloned()
    }

    fn scan_done(&self, probe: &str, cands: Arc<Vec<Candidate>>) {
        if self.file.is_some() {
            self.scans.lock().unwrap().insert(probe.to_owned(), cands);
        }
    }
}

pub fn visit_cached(
    root: &Path,
    cache: &Cache,
    probe: &str,
    mut collect: impl FnMut(&Path, &str, &mut Vec<Candidate>) -> Result<(), String>,
) -> Result<Arc<Vec<Candidate>>, String> {
    if let Some(cands) = cache.scanned(probe) {
        return Ok(cands);
    }
    let mut cands = Vec::new();
    visit(root, &mut |p, rel| {
        let Ok(bytes) = std::fs::read(p) else {
            return Ok(());
        };
        let file_hash = hash(&String::from_utf8_lossy(&bytes));
        if let Some(cached) = cache.get(probe, rel, &file_hash) {
            cands.extend(cached);
            return Ok(());
        }
        let mut fragment = Vec::new();
        collect(p, rel, &mut fragment)?;
        cache.put(probe, rel, &file_hash, &fragment);
        cands.extend(fragment);
        Ok(())
    })?;
    let cands = Arc::new(cands);
    cache.scan_done(probe, Arc::clone(&cands));
    Ok(cands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

        let first = visit_cached(d.path(), &cache, "p", counting_collect(&calls)).unwrap();
        let second = visit_cached(d.path(), &cache, "p", counting_collect(&calls)).unwrap();

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

        let first = Cache::load(&cache_file);
        visit_cached(src.path(), &first, "p", counting_collect(&calls)).unwrap();

        std::fs::write(src.path().join("a.rs"), "two").unwrap();
        let second = Cache::load(&cache_file);
        visit_cached(src.path(), &second, "p", counting_collect(&calls)).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_second_lookup_in_the_same_process_does_not_rewalk() {
        let d = tree(&[("a.rs", "one"), ("b.rs", "two")]);
        let cache = Cache::disabled();
        let calls = AtomicUsize::new(0);

        let first = visit_cached(d.path(), &cache, "p", counting_collect(&calls)).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "one collect per file");
        assert_eq!(first.len(), 2);

        let second = visit_cached(d.path(), &cache, "p", counting_collect(&calls)).unwrap();
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

        visit_cached(d.path(), &cache, "ast-map", counting_collect(&calls)).unwrap();
        visit_cached(d.path(), &cache, "addr-map", counting_collect(&calls)).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_warm_cache_survives_a_reload_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let src = tree(&[("a.rs", "one")]);
        let cache_file = dir.path().join("cache.json");
        let calls = AtomicUsize::new(0);

        let warm = Cache::load(&cache_file);
        visit_cached(src.path(), &warm, "p", counting_collect(&calls)).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let reloaded = Cache::load(&cache_file);
        visit_cached(src.path(), &reloaded, "p", counting_collect(&calls)).unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a fresh process should still hit"
        );
    }
}
