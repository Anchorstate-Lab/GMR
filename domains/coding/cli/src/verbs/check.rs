use std::path::Path;

use gmr::{AnchorKey, Observed, Runtime};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::{Context, DEFAULT_FILE, differs, merged, read_declared};

async fn drifted(
    rt: &Runtime,
    root: &Path,
    catalog: Catalog,
    keys: &[AnchorKey],
) -> Result<Vec<(AnchorKey, String)>, CliError> {
    let declared = read_declared(root, DEFAULT_FILE)?;
    let notes = crate::memories::scan(root, &catalog)?;
    let decls = merged(&declared, &notes);
    let ctx = Context { catalog };

    let mut out = Vec::new();
    for key in keys {
        let Some(decl) = decls.iter().find(|d| d.key == key.as_str()) else {
            continue;
        };
        let view = rt.read(key).await?;
        if view.closed {
            continue;
        }
        let facets = differs(&view.anchor, decl, &ctx)?;
        if !facets.is_empty() {
            out.push((key.clone(), facets.join(" · ")));
        }
    }
    Ok(out)
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => super::resolve(rt, &k).await?,
        None => rt.anchors().await?,
    };
    let catalog = Catalog::load(root)?;
    let subs = Subscriptions::load(root, &catalog)?;
    let drifted = drifted(rt, root, catalog, &keys).await?;
    let swapped = super::swapped(rt, &keys).await?;

    let mut handed: Vec<(AnchorKey, String, Option<String>, Vec<String>)> = Vec::new();
    let mut unclaimed = Vec::new();
    let mut unseen = Vec::new();
    let mut quiet = 0;

    for key in &keys {
        let observed = rt.observe(key).await?;
        let moved = match &observed {
            Observed::Attempt { code, message, .. } => {
                unseen.push((key.clone(), format!("{code:?}: {message}")));
                continue;
            }
            Observed::Closed => continue,
            other => matches!(other, Observed::Transitioned { .. }),
        };

        let view = rt.read(key).await?;
        let memories =
            super::observe::delivered(rt, &subs, key, &view.state, moved, &mut unclaimed).await?;
        if memories.is_empty() {
            quiet += usize::from(moved);
            continue;
        }
        let status = view
            .state
            .status()
            .map(|s| s.to_string())
            .unwrap_or_default();
        handed.push((
            key.clone(),
            status,
            crate::render::diagnosis(view.facts.as_ref()),
            memories,
        ));
    }

    let wrong = !handed.is_empty()
        || !unclaimed.is_empty()
        || !unseen.is_empty()
        || !drifted.is_empty()
        || !swapped.is_empty();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "observed": keys.len(),
                "handed_back": handed.iter().map(|(k, s, d, m)| serde_json::json!({
                    "anchor": k, "status": s, "diagnosis": d, "memories": m
                })).collect::<Vec<_>>(),
                "moved_unwatched": quiet,
                "unclaimed": unclaimed,
                "unseen": unseen.iter().map(|(k, m)| serde_json::json!({
                    "anchor": k, "detail": m
                })).collect::<Vec<_>>(),
                "criteria_drifted": drifted.iter().map(|(k, f)| serde_json::json!({
                    "anchor": k, "facets": f
                })).collect::<Vec<_>>(),
                "instrument_swapped": swapped.iter().map(|(k, v)| serde_json::json!({
                    "anchor": k, "versions": v
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(i32::from(wrong));
    }

    for (key, status, diagnosis, memories) in &handed {
        println!("{key}   {status}");
        if let Some(d) = diagnosis {
            println!("  {d}");
        }
        for m in memories {
            println!("  → {m}");
        }
    }
    if !unseen.is_empty() {
        println!("\n{} could not be looked at:", unseen.len());
        for (key, detail) in &unseen {
            println!("  ! {key}  {detail}");
        }
    }
    super::observe::report_unclaimed(&unclaimed);

    match (handed.len(), quiet) {
        (0, 0)
            if unseen.is_empty()
                && unclaimed.is_empty()
                && drifted.is_empty()
                && swapped.is_empty() =>
        {
            println!("{} anchors, nothing moved.", keys.len())
        }
        (0, n) if n > 0 => println!(
            "\n{} anchors, {n} moved on axes nobody asked about. `gmr status` shows them.",
            keys.len()
        ),
        (n, _) if n > 0 => println!(
            "\n{n} of {} handed a memory back. Re-read it: does what you wrote still hold?",
            keys.len()
        ),
        _ => {}
    }

    if !drifted.is_empty() {
        println!(
            "\n{} of {} stand on criteria their declaration no longer asks for.\n\
             Nothing above is trustworthy until these are taken: an anchor whose rules\n\
             this build cannot name has no axes, so `watch:` does not apply to it and it\n\
             falls back to reporting any transition at all.",
            drifted.len(),
            keys.len()
        );
        for (key, facets) in &drifted {
            println!("  != {key}  ({facets})");
        }
        println!("\n     gmr accept --all --criteria --why '...'");
    }

    if !swapped.is_empty() {
        println!(
            "\n{} of {} stand on a reading a different instrument took.\n\
             The stored baseline and what this build measures are not comparable, so\n\
             an axis above may be quiet because nothing moved, or loud because the\n\
             probe changed — this run cannot tell those apart.",
            swapped.len(),
            keys.len()
        );
        for (key, versions) in &swapped {
            println!("  ~= {key}  ({versions})");
        }
        println!("\n     gmr rebase --all --why '...'");
    }
    Ok(i32::from(wrong))
}
