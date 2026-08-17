use std::path::Path;

use gmr::{AnchorKey, Runtime};

use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::{self, Context, DEFAULT_FILE, read_declared};

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

#[derive(Default)]
struct Verdict {
    stranded: bool,
    provider_unavailable: bool,
    breaking_notes: bool,
    undeclared: bool,
    gone: bool,
    no_provider: bool,
    skill_stale: bool,
}

impl Verdict {
    fn theirs_to_fix(&self) -> bool {
        self.stranded
            || self.provider_unavailable
            || self.breaking_notes
            || self.undeclared
            || self.gone
            || self.no_provider
            || self.skill_stale
    }
}

#[derive(Default)]
struct Grounds {
    gone: Vec<String>,
    no_provider: Vec<String>,
    unreachable: Vec<String>,
    no_before: Vec<String>,
}

fn grounds(live: &[&gmr::AnchorView]) -> Grounds {
    let mut out = Grounds::default();
    for m in live.iter().flat_map(|v| &v.memories) {
        let id = m.reference.external_id.to_string();
        match &m.grounding {
            gmr::Grounding::Gone => out.gone.push(id),
            gmr::Grounding::NoProvider { .. } => out.no_provider.push(id),
            gmr::Grounding::Unreachable { .. } => out.unreachable.push(id),
            gmr::Grounding::Rewritten { before, .. }
                if !matches!(before, gmr::Before::Retrieved { .. }) =>
            {
                out.no_before.push(id);
            }
            _ => {}
        }
    }
    for list in [
        &mut out.gone,
        &mut out.no_provider,
        &mut out.unreachable,
        &mut out.no_before,
    ] {
        list.sort();
        list.dedup();
    }
    out
}

pub fn undeclared(
    root: &Path,
    catalog: Catalog,
    live: &[&gmr::AnchorView],
    scanned: &crate::memories::Scanned,
) -> Result<Vec<AnchorKey>, CliError> {
    let declared = read_declared(root, DEFAULT_FILE)?;
    let decls = sync::merged(&declared, &scanned.notes);
    let ctx = Context { catalog };
    Ok(sync::audit(live.iter().copied(), &decls, scanned, &ctx)?.undeclared)
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
    let ground = grounds(&live);
    let skill_stale = crate::skill::stale(root);
    let no_git = versioning_is_broken(root);
    let provider_warnings = rt.memory().provider_warnings();
    let catalog = crate::probes::Catalog::load(root)?;
    let (_, watch) = crate::delivery::Subscriptions::load(root, &catalog)?;
    let mut scanned = crate::memories::scan(root, &catalog)?;
    scanned.accounted_for(
        read_declared(root, DEFAULT_FILE)?
            .anchor
            .iter()
            .map(|d| d.key.as_str())
            .chain(views.iter().map(|v| v.key.as_str()))
            .collect::<Vec<_>>()
            .into_iter(),
    );
    let undeclared_keys = undeclared(root, catalog, &live, &scanned)?;
    let undeclared: Vec<&str> = undeclared_keys.iter().map(|k| k.as_str()).collect();
    let mut faults = scanned.faults;
    faults.extend(watch);
    faults.sort_by(|a, b| (b.weight, &a.note, a.code).cmp(&(a.weight, &b.note, b.code)));
    let (breaking, advisory): (Vec<_>, Vec<_>) = faults.iter().partition(|f| f.breaks());
    let exit_code = i32::from(
        Verdict {
            stranded: !stranded.is_empty(),
            provider_unavailable: !provider_warnings.is_empty(),
            breaking_notes: !breaking.is_empty(),
            undeclared: !undeclared.is_empty(),
            gone: !ground.gone.is_empty(),
            no_provider: !ground.no_provider.is_empty(),
            skill_stale: !skill_stale.is_empty(),
        }
        .theirs_to_fix(),
    );
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
                "gone": ground.gone, "no_provider": ground.no_provider,
                "unreachable": ground.unreachable, "no_before": ground.no_before,
                "skill_stale": skill_stale,
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
    if !ground.gone.is_empty() {
        println!(
            "gone      {}\n          <- the provider says these records no longer exist. Restore them or detach the binding; until then these anchors are watched on behalf of nothing",
            ground.gone.join(", ")
        );
    }
    if !ground.no_provider.is_empty() {
        println!(
            "no provider {}\n            <- bound through a provider this binary does not have. Rebuild with that feature, or the binding cannot be read here at all",
            ground.no_provider.join(", ")
        );
    }
    if !ground.unreachable.is_empty() {
        println!(
            "unreachable {} record(s) could not be reached this run\n            <- somebody else's service or a spent budget, not something to fix here. Reported, never counted against the exit code",
            ground.unreachable.len()
        );
    }
    if !ground.no_before.is_empty() {
        println!(
            "no before {} rewritten record(s) cannot show what they said at binding time\n          <- the provider keeps no history, or did not keep that version. You are still told they moved; you just have to re-read the whole thing instead of a diff",
            ground.no_before.len()
        );
    }
    if !skill_stale.is_empty() {
        println!(
            "skill     {}\n          <- installed SKILL.md differs from this binary's. `gmr init` only ever writes it when absent, so an upgraded binary leaves the old text in place and agents keep reading contracts this build no longer honours. Delete the file and re-run `gmr init`",
            skill_stale.join(", ")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_run_is_green() {
        assert!(!Verdict::default().theirs_to_fix());
    }

    #[test]
    fn every_condition_this_repositorys_owner_can_act_on_turns_it_red() {
        let each: [fn(&mut Verdict); 7] = [
            |v| v.stranded = true,
            |v| v.provider_unavailable = true,
            |v| v.breaking_notes = true,
            |v| v.undeclared = true,
            |v| v.gone = true,
            |v| v.no_provider = true,
            |v| v.skill_stale = true,
        ];
        for set in each {
            let mut v = Verdict::default();
            set(&mut v);
            assert!(v.theirs_to_fix());
        }
    }

    #[test]
    fn a_store_that_would_not_answer_is_not_among_them() {
        assert_eq!(
            std::mem::size_of::<Verdict>(),
            7,
            "Verdict is one bool per condition that makes this run red, and a store being \
             unreachable is deliberately not one of them: nobody holding this repository can \
             fix somebody else's service, and a build that fails on it fails for a reason its \
             owner cannot act on. Adding a field here means claiming otherwise"
        );
    }
}
