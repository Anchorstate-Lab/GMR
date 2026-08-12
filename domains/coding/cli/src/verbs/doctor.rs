use std::path::Path;

use gmr::Runtime;

use crate::error::CliError;

fn unresolvable(rt: &Runtime, views: &[&gmr::AnchorView]) -> Vec<String> {
    views
        .iter()
        .filter(|v| rt.instrument(&v.anchor.probe).is_err())
        .map(|v| v.key.to_string())
        .collect()
}

fn versioning_is_broken(root: &Path) -> bool {
    !root.join(".git").exists()
}

pub fn undeclared(
    root: &Path,
    live: &[&gmr::AnchorView],
    notes: &[crate::memories::Note],
) -> Result<Vec<String>, CliError> {
    use crate::verbs::sync::{DEFAULT_FILE, merged, read_declared};
    let declared = read_declared(root, DEFAULT_FILE)?;
    let decls = merged(&declared, notes);
    Ok(live
        .iter()
        .filter(|v| !v.memories.is_empty())
        .filter(|v| !decls.iter().any(|d| d.key == v.key.as_str()))
        .map(|v| v.key.to_string())
        .collect())
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    cache_fault: Option<&str>,
    json: bool,
) -> Result<i32, CliError> {
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
    let corpus = rt.corpus_health().await?;
    let barren: Vec<&str> = corpus.barren_anchors.iter().map(|k| k.as_str()).collect();
    let stranded = unresolvable(rt, &live);
    let no_git = versioning_is_broken(root);
    let provider_warnings = rt.memory().provider_warnings();
    let catalog = crate::probes::Catalog::load(root)?;
    let (_, watch) = crate::delivery::Subscriptions::load(root, &catalog)?;
    let scanned = crate::memories::scan(root, &catalog)?;
    let undeclared = undeclared(root, &live, &scanned.notes)?;
    let mut faults = scanned.faults;
    faults.extend(watch);
    faults.sort_by(|a, b| (b.weight, &a.note, a.code).cmp(&(a.weight, &b.note, b.code)));
    let (breaking, advisory): (Vec<_>, Vec<_>) = faults.iter().partition(|f| f.breaks());
    let exit_code = if stranded.is_empty()
        && provider_warnings.is_empty()
        && breaking.is_empty()
        && undeclared.is_empty()
    {
        0
    } else {
        1
    };
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
                "stranded": stranded, "undeclared": undeclared,
                "content_versioning": !no_git,
                "provider_warnings": provider_warnings, "cache_fault": cache_fault,
                "notes": faults.iter().map(|f| serde_json::json!({
                    "note": f.note, "key": f.key, "code": f.code, "detail": f.detail,
                    "breaks": f.breaks(), "blocks": f.blocks(),
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(exit_code);
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
    if !undeclared.is_empty() {
        println!(
            "undeclared {}\n           <- a memory is bound to this anchor and no note declares it any more, so nothing compares its criteria against anything. The note was deleted or its coordinate edited away",
            undeclared.join(", ")
        );
    }
    for f in &breaking {
        println!(
            "note      {}  {}\n          <- {}",
            f.note, f.code, f.detail
        );
    }
    for f in &advisory {
        println!("{:9} {}\n          <- {}", f.code, f.note, f.detail);
    }
    if !stranded.is_empty() {
        println!(
            "stranded  {}\n          <- no transport here can resolve the declared probe; run `probes build`",
            stranded.join(", ")
        );
    }
    if no_git {
        println!(
            "provider  this is not a git repository\n          \
             <- binding works, but fetching a note back at the version it was bound at does not"
        );
    }
    for w in provider_warnings {
        println!(
            "provider  {} unavailable: {}\n          \
             <- bindings through it will fail with \"no content provider could version\"",
            w.provider, w.message
        );
    }
    if let Some(fault) = cache_fault {
        println!(
            "cache     {fault}\n          \
             <- advisory, not broken: the next verb that probes writes a fresh one"
        );
    }
    Ok(exit_code)
}
