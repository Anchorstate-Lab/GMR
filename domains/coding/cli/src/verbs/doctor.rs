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
    let notes = crate::memories::scan(root, &catalog)?.lint;
    let (malformed, advisory): (Vec<_>, Vec<_>) = notes.iter().partition(|l| l.breaks);
    let (_, unwatchable) = crate::delivery::Subscriptions::load(root, &catalog)?;
    let exit_code = if stranded.is_empty()
        && provider_warnings.is_empty()
        && malformed.is_empty()
        && unwatchable.is_empty()
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
                "stranded": stranded, "content_versioning": !no_git,
                "provider_warnings": provider_warnings, "cache_fault": cache_fault,
                "notes": notes.iter().map(|l| serde_json::json!({
                    "note": l.note, "code": l.code, "detail": l.detail, "breaks": l.breaks,
                })).collect::<Vec<_>>(),
                "watch_invalid": unwatchable.iter().map(|u| serde_json::json!({
                    "note": u.note, "reason": u.reason,
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
    for l in &malformed {
        println!(
            "note      {}  {}\n          <- {}",
            l.note, l.code, l.detail
        );
    }
    for l in &advisory {
        println!("{:9} {}\n          <- {}", l.code, l.note, l.detail);
    }
    for u in &unwatchable {
        println!(
            "note      {}  watch-invalid\n          <- {}",
            u.note, u.reason
        );
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
