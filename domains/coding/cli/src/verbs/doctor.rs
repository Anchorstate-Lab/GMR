use std::path::Path;

use gmr::Runtime;
use gmr_transport::shell::Artifacts;

use crate::error::CliError;
use crate::probes::store_dir;

/// Anchors whose probe is not installed on this machine. This is the fresh
/// clone failure: the declaration travels, the artifact does not, and without
/// this check it shows up as N identical Attempt entries instead of one answer.
fn unresolvable(root: &Path, views: &[&gmr::AnchorView]) -> Vec<String> {
    let artifacts = Artifacts::new(store_dir(root));
    views
        .iter()
        .filter(|v| artifacts.resolve(&v.anchor.probe.name).is_err())
        .map(|v| v.key.to_string())
        .collect()
}

/// git is how notes are versioned here; outside a repository `bind` still works
/// but fetching a note back at the version it was bound at does not.
fn versioning_is_broken(root: &Path) -> bool {
    !root.join(".git").exists()
}

pub async fn run(rt: &Runtime, root: &Path, json: bool) -> Result<i32, CliError> {
    let views = rt.read_all().await?;
    let live: Vec<_> = views.iter().filter(|v| !v.closed).collect();

    let unseen: Vec<&str> = live
        .iter()
        .filter(|v| v.attempts > 0)
        .map(|v| v.key.as_str())
        .collect();
    let absent: Vec<&str> = live
        .iter()
        .filter(|v| v.sighting == gmr::Sighting::Absent)
        .map(|v| v.key.as_str())
        .collect();
    // Barren comes from corpus_health, not a second `memories.is_empty()` scan
    // of these same views — one definition of "unbound" instead of two that
    // could drift apart.
    let corpus = rt.corpus_health().await?;
    let barren: Vec<&str> = corpus.barren_anchors.iter().map(|k| k.as_str()).collect();
    let stranded = unresolvable(root, &live);
    let no_git = versioning_is_broken(root);
    let states: Vec<String> = live
        .iter()
        .filter_map(|v| v.status.as_ref().map(|s| s.to_string()))
        .collect();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "anchors": views.len(), "live": live.len(),
                "absent": absent, "unseen": unseen, "barren": barren,
                "stranded": stranded, "content_versioning": !no_git,
            })
        );
        return Ok(0);
    }

    println!("anchors   {} (live {})", views.len(), live.len());
    if !states.is_empty() {
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for s in &states {
            *counts.entry(s.as_str()).or_default() += 1;
        }
        let line: Vec<String> = counts.iter().map(|(s, n)| format!("{s}x{n}")).collect();
        println!("status    {}", line.join("  "));
    }
    if !absent.is_empty() {
        println!(
            "absent    {}\n          <- the probe has not seen anything yet; this is normal when criteria are written before implementation",
            absent.join(", ")
        );
    }
    if !unseen.is_empty() {
        println!(
            "unseen    {}  <- fix the probe or credentials",
            unseen.join(", ")
        );
    }
    if !barren.is_empty() {
        println!(
            "barren    {}\n          <- observing a position where nobody has written a memory",
            barren.join(", ")
        );
    }
    if !stranded.is_empty() {
        println!(
            "stranded  {}\n          <- no artifact is installed for the declared probe; run `probes build`",
            stranded.join(", ")
        );
    }
    if no_git {
        println!(
            "provider  this is not a git repository\n          \
             <- binding works, but fetching a note back at the version it was bound at does not"
        );
    }
    Ok(if stranded.is_empty() { 0 } else { 1 })
}
