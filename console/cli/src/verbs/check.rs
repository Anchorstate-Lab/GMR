use std::path::Path;

use gmr::{AnchorKey, AnchorView, Looked, Observed, Runtime, State};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::{self, Audit, Context, DEFAULT_FILE, read_declared};

fn criteria(
    root: &Path,
    catalog: Catalog,
    views: &[AnchorView],
    bound: &sync::Bound,
    among: Option<&std::collections::BTreeSet<String>>,
) -> Result<Audit, CliError> {
    let declared = read_declared(root, DEFAULT_FILE)?;
    let scanned = match among {
        None => crate::memories::scan(root, &catalog)?,
        Some(rels) => crate::memories::scan_among(root, &catalog, rels)?,
    };
    let decls = sync::merged(&declared, &scanned.notes);
    let ctx = Context { catalog };
    sync::audit(views, bound, &decls, &scanned, &ctx)
}

fn settled(observed: &Observed, before: &State) -> Option<(bool, State)> {
    match observed {
        Observed::Attempt { .. } | Observed::Closed | Observed::Contended => None,
        Observed::Transitioned { to, .. } => Some((true, to.clone())),
        Observed::Unchanged { state } => Some((false, state.clone())),
        Observed::Still => Some((false, before.clone())),
    }
}

#[derive(Default)]
struct Wrong {
    snagged: bool,
    handed: bool,
    unclaimed: bool,
    unseen: bool,
    drifted: bool,
    unreadable: bool,
    undeclared: bool,
    unwatchable: bool,
    swapped: bool,
}

impl Wrong {
    fn any(&self) -> bool {
        self.snagged
            || self.handed
            || self.unclaimed
            || self.unseen
            || self.drifted
            || self.unreadable
            || self.undeclared
            || self.unwatchable
            || self.swapped
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &crate::memories::Names,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let named = key.is_some();
    let keys = match key {
        Some(k) => super::resolve(rt, &k).await?,
        None => rt.anchors().await?,
    };
    let catalog = Catalog::load(root)?;
    let (bound, among) = match named {
        false => (sync::Bound::of(rt).await?, None),
        true => {
            let records = rt.memory().all().await?;
            let mut rels = std::collections::BTreeSet::new();
            let mut held = Vec::new();
            for record in &records {
                if !record.binding.anchors.iter().any(|a| keys.contains(a)) {
                    continue;
                }
                held.extend(record.binding.anchors.iter().cloned());
                if let Some(reference) = record.binding.claim.stored()
                    && reference.provider.as_str() == "git"
                {
                    rels.insert(reference.external_id.to_string());
                }
            }
            (sync::Bound::among(held), Some(rels))
        }
    };
    let (subs, unwatchable) = match &among {
        None => Subscriptions::load(root, &catalog, names)?,
        Some(rels) => Subscriptions::load_among(root, &catalog, names, rels)?,
    };

    let (views, looks): (Vec<AnchorView>, Vec<Observed>) = rt
        .look_all(&keys)
        .await?
        .into_iter()
        .map(|Looked { before, observed }| (before, observed))
        .unzip();

    let Audit {
        drifted,
        unreadable,
        undeclared,
    } = criteria(root, catalog, &views, &bound, among.as_ref())?;
    let swapped = super::swapped(rt, &views);

    let mut handed: Vec<(AnchorKey, String, Option<String>, Vec<gmr::Ref>)> = Vec::new();
    let mut unclaimed = Vec::new();
    let mut unseen = Vec::new();
    let mut snags: Vec<super::observe::Snag> = Vec::new();
    let mut quiet = 0;

    for (before, observed) in views.iter().zip(&looks) {
        let key = &before.key;
        if let Observed::Attempt { code, message, .. } = observed {
            unseen.push((key.clone(), format!("{code:?}: {message}")));
            continue;
        }
        let Some((moved, state)) = settled(observed, &before.state) else {
            continue;
        };

        let shape = crate::shapes::of(&before.anchor.transitions);
        let memories = super::observe::settled(
            rt,
            &subs,
            key,
            shape,
            &state,
            moved,
            &mut unclaimed,
            &mut snags,
        )
        .await?;
        if memories.is_empty() {
            quiet += usize::from(moved);
            continue;
        }
        let after = rt.read(key).await?;
        handed.push((
            key.clone(),
            after
                .state
                .status()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            crate::render::diagnosis(after.facts.as_ref()),
            memories,
        ));
    }

    let wrong = Wrong {
        handed: !handed.is_empty(),
        unclaimed: !unclaimed.is_empty(),
        unseen: !unseen.is_empty(),
        snagged: !snags.is_empty(),
        drifted: !drifted.is_empty(),
        unreadable: !unreadable.is_empty(),
        undeclared: !undeclared.is_empty(),
        unwatchable: !unwatchable.is_empty(),
        swapped: !swapped.is_empty(),
    };

    if json {
        println!(
            "{}",
            serde_json::json!({
                "observed": keys.len(),
                "handed_back": handed.iter().map(|(k, s, d, m)| serde_json::json!({
                    "anchor": k, "status": s, "diagnosis": d,
                    "memories": super::observe::addressed_all(m)
                })).collect::<Vec<_>>(),
                "moved_unwatched": quiet,
                "unclaimed": unclaimed,
                "unseen": unseen.iter().map(|(k, m)| serde_json::json!({
                    "anchor": k, "detail": m
                })).collect::<Vec<_>>(),
                "criteria_drifted": drifted.iter().map(|(k, f)| serde_json::json!({
                    "anchor": k, "facets": f
                })).collect::<Vec<_>>(),
                "criteria_unreadable": unreadable.iter().map(|(k, r)| serde_json::json!({
                    "anchor": k, "reason": r
                })).collect::<Vec<_>>(),
                "watch_unevaluable": snags.iter().map(|(k, m, why)| serde_json::json!({
                    "anchor": k, "memory": crate::memories::addressed(m), "detail": why
                })).collect::<Vec<_>>(),
                "criteria_undeclared": undeclared,
                "watch_invalid": unwatchable.iter().map(|f| serde_json::json!({
                    "note": f.note, "key": f.key, "code": f.code, "detail": f.detail
                })).collect::<Vec<_>>(),
                "instrument_swapped": swapped.iter().map(|(k, v)| serde_json::json!({
                    "anchor": k, "versions": v
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(i32::from(wrong.any()));
    }

    for (key, status, diagnosis, memories) in &handed {
        println!("{key}   {status}");
        if let Some(d) = diagnosis {
            println!("  {d}");
        }
        for m in memories {
            println!("  → {}", names.of(m));
        }
    }
    if !unseen.is_empty() {
        println!("\n{} could not be looked at:", unseen.len());
        for (key, detail) in &unseen {
            println!("  ! {key}  {detail}");
        }
    }
    super::observe::report_unclaimed(&unclaimed);
    super::observe::report_snags(&snags);

    match (handed.len(), quiet) {
        (0, 0) if !wrong.any() => println!("{} anchors, nothing moved.", keys.len()),
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

    if !unreadable.is_empty() {
        println!(
            "\n{} of {} stand on a declaration this run could not read.\n\
             A memory named these as its coordinate, and something about that coordinate\n\
             — a broken frontmatter, a probe nothing here can route to — kept it from\n\
             becoming a declaration this run could compare against. These are not known\n\
             to have drifted; they are unwatched until the coordinate is fixed.",
            unreadable.len(),
            keys.len()
        );
        for (key, reason) in &unreadable {
            println!("  ?! {key}  ({reason})");
        }
        println!("\n     gmr sync   shows the same reason against the note that named it");
    }

    if !undeclared.is_empty() {
        println!(
            "\n{} of {} are supervised by no note this build can read.\n\
             A memory is bound to each, so they were declared once — and no note in\n\
             `memories/` declares them now. They are still observed, but their criteria\n\
             are compared against nothing, so a declaration that drifted away from them\n\
             cannot be reported: deleting the note is how an anchor stops being watched\n\
             without anybody closing it.",
            undeclared.len(),
            keys.len()
        );
        for key in &undeclared {
            println!("  ?? {key}");
        }
        println!("\n     gmr close   if it has served its purpose, or write the note again");
    }

    if !unwatchable.is_empty() {
        println!(
            "\n{} notes declare a `watch:` this run cannot make sense of:",
            unwatchable.len()
        );
        for f in &unwatchable {
            println!("  ?! {}  ({})", f.note, f.detail);
        }
        println!(
            "\nEach of these is unwatched until fixed — not known to have drifted, just \
             never checked."
        );
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
    Ok(i32::from(wrong.any()))
}

#[cfg(test)]
mod tests {
    use super::Wrong;

    #[test]
    fn a_quiet_run_is_green() {
        assert!(!Wrong::default().any());
    }

    #[test]
    fn nothing_a_provider_answers_can_move_this_exit_code() {
        assert_eq!(
            std::mem::size_of::<Wrong>(),
            9,
            "every field here is something this repository's owner can act on: a memory handed \
             back, a note claiming nothing, an anchor nobody could look at, criteria that \
             drifted. Whether a store answered is deliberately not among them — D6 puts that \
             in the bucket that never turns red, and `read` reports it instead. A ninth field \
             means somebody let a network failure decide whether CI passes, and the damage is \
             that a repository with an unreachable store can no longer be checked at all"
        );
    }
}
