use std::path::Path;

use gmr::{AnchorKey, Footing, HoldingKind, KnowledgeKind, Runtime};

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
    unsupervised: bool,
    chain_broken: bool,
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
            || self.unsupervised
            || self.chain_broken
    }
}

fn addresses(refs: &[gmr::Ref]) -> Vec<String> {
    refs.iter().map(crate::memories::addressed).collect()
}

fn claimed(claims: &[gmr::Claim]) -> Vec<String> {
    claims.iter().map(gmr::Claim::to_string).collect()
}

fn spelled_claims(claims: &[gmr::Claim], names: &crate::memories::Names) -> String {
    claims
        .iter()
        .map(|c| match c.stored() {
            Some(reference) => names.of(reference),
            None => c.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn grounds_json(ground: &gmr::CorpusHealth) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for kind in [
        HoldingKind::Holds,
        HoldingKind::Finished,
        HoldingKind::Moved,
        HoldingKind::Incomparable,
        HoldingKind::Absent,
        HoldingKind::NeverEstablished,
        HoldingKind::Undated,
    ] {
        let mut anchors = serde_json::Map::new();
        if let Some(on) = ground.holdings.get(&kind) {
            for (anchor, refs) in on {
                anchors.insert(anchor.to_string(), serde_json::json!(addresses(refs)));
            }
        }
        let name = serde_json::to_value(kind).unwrap_or_default();
        out.insert(
            name.as_str().unwrap_or("unknown").to_owned(),
            serde_json::Value::Object(anchors),
        );
    }
    serde_json::Value::Object(out)
}

fn grounds_line(ground: &gmr::CorpusHealth, kind: HoldingKind) -> Option<String> {
    let on = ground.holdings.get(&kind)?;
    let pairs: usize = on.values().map(Vec::len).sum();
    Some(format!("{pairs} record(s) on {} anchor(s)", on.len()))
}

fn counted(records: &[(&gmr::AnchorKey, &gmr::Ref)]) -> Option<String> {
    if records.is_empty() {
        return None;
    }
    let anchors: std::collections::BTreeSet<_> = records.iter().map(|(k, _)| *k).collect();
    Some(format!(
        "{} record(s) on {} anchor(s)",
        records.len(),
        anchors.len()
    ))
}

type Standing<'a> = Vec<(&'a gmr::AnchorKey, &'a gmr::Ref)>;

fn subscribed<'a>(
    ground: &'a gmr::CorpusHealth,
    subs: &crate::delivery::Subscriptions,
    live: &[&gmr::AnchorView],
) -> (Standing<'a>, Standing<'a>) {
    let mut watched = Vec::new();
    let mut quiet = Vec::new();
    let Some(on) = ground.holdings.get(&HoldingKind::Moved) else {
        return (watched, quiet);
    };
    for (key, refs) in on {
        let Some(view) = live.iter().find(|v| &v.key == key) else {
            quiet.extend(refs.iter().map(|r| (key, r)));
            continue;
        };
        let shape = crate::shapes::of(&view.anchor.transitions);
        for note in refs {
            match subs.delivers(key.as_str(), shape, note, &view.state) {
                Ok(true) => watched.push((key, note)),
                _ => quiet.push((key, note)),
            }
        }
    }
    (watched, quiet)
}

fn spelled(refs: &[gmr::Ref], names: &crate::memories::Names) -> String {
    refs.iter()
        .map(|r| names.of(r))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn undeclared(
    root: &Path,
    catalog: Catalog,
    live: &[&gmr::AnchorView],
    bound: &sync::Bound,
    scanned: &crate::memories::Scanned,
) -> Result<Vec<AnchorKey>, CliError> {
    let declared = read_declared(root, DEFAULT_FILE)?;
    let decls = sync::merged(&declared, &scanned.notes);
    let ctx = Context { catalog };
    Ok(sync::audit(live.iter().copied(), bound, &decls, scanned, &ctx)?.undeclared)
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &crate::memories::Names,
    cache_fault: Option<&str>,
    chain_break: Option<gmr::Seq>,
    json: bool,
) -> Result<i32, CliError> {
    let declared_providers = crate::providers::declared(root)?;
    let corpus = rt.corpus().await?;
    let live = corpus.live();
    let ground = corpus.health();

    let blind = |kind: KnowledgeKind| -> Vec<&str> {
        ground
            .knowings
            .get(&kind)
            .map(|keys| keys.iter().map(|k| k.as_str()).collect())
            .unwrap_or_default()
    };
    let unseen: Vec<&str> = [
        KnowledgeKind::NeverAsked,
        KnowledgeKind::Unreachable,
        KnowledgeKind::Unusable,
        KnowledgeKind::Unevaluable,
    ]
    .into_iter()
    .flat_map(blind)
    .collect();
    let absent: Vec<&str> = live
        .iter()
        .filter(|v| v.sighting == gmr::Sighting::Absent)
        .map(|v| v.key.as_str())
        .collect();
    let barren: Vec<&str> = ground.barren_anchors.iter().map(|k| k.as_str()).collect();
    let stranded = unresolvable(rt, &live);
    let skill_stale = crate::skill::stale(root);
    let no_git = versioning_is_broken(root);
    let provider_warnings = rt.memory().provider_warnings();
    let catalog = crate::probes::Catalog::load(root)?;
    let (subs, watch) = crate::delivery::Subscriptions::load(root, &catalog, names)?;
    let (watched, quiet) = subscribed(ground, &subs, &live);
    let mut scanned = crate::memories::scan(root, &catalog)?;
    let declared = read_declared(root, DEFAULT_FILE)?;
    scanned.accounted_for(
        declared
            .anchor
            .iter()
            .map(|d| d.key.as_str())
            .chain(corpus.anchors().map(|v| v.key.as_str())),
    );
    let undeclared_keys = undeclared(root, catalog, &live, &sync::Bound::of(rt).await?, &scanned)?;
    let undeclared: Vec<&str> = undeclared_keys.iter().map(|k| k.as_str()).collect();
    let mut faults = scanned.faults;
    faults.extend(watch);
    faults.sort_by(|a, b| (b.weight, &a.note, a.code).cmp(&(a.weight, &b.note, b.code)));
    let mut kinds: std::collections::BTreeMap<(String, &'static str), usize> = Default::default();
    for (_, record) in rt.all_links().await? {
        *kinds
            .entry((record.kind.0, record.source.as_str()))
            .or_default() += 1;
    }
    let (breaking, advisory): (Vec<_>, Vec<_>) = faults.iter().partition(|f| f.breaks());
    let exit_code = i32::from(
        Verdict {
            stranded: !stranded.is_empty(),
            provider_unavailable: !provider_warnings.is_empty(),
            breaking_notes: !breaking.is_empty(),
            undeclared: !undeclared.is_empty(),
            gone: !ground.on(Footing::Gone).is_empty(),
            no_provider: !ground.on(Footing::NoProvider).is_empty(),
            skill_stale: !skill_stale.is_empty(),
            unsupervised: !ground.unsupervised.is_empty(),
            chain_broken: chain_break.is_some(),
        }
        .theirs_to_fix(),
    );
    let states: Vec<String> = live
        .iter()
        .filter_map(|v| v.status.as_ref().map(|s| s.to_string()))
        .collect();

    let spending = rt.spending().await?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "anchors": corpus.len(), "live": live.len(),
                "absent": absent, "unseen": unseen, "barren": barren,
                "unseen_unreachable": blind(KnowledgeKind::Unreachable),
                "unseen_unusable": blind(KnowledgeKind::Unusable),
                "unseen_unevaluable": blind(KnowledgeKind::Unevaluable),
                "unseen_never_asked": blind(KnowledgeKind::NeverAsked),
                "grounds": grounds_json(ground),
                "stranded": stranded, "undeclared": undeclared,
                "gone": addresses(ground.on(Footing::Gone)),
                "no_provider": addresses(ground.on(Footing::NoProvider)),
                "unreachable": addresses(ground.on(Footing::Unreachable)),
                "never_asked": addresses(ground.on(Footing::NeverAsked)),
                "bound": ground.grounded_records(),
                "no_before": addresses(ground.on(Footing::NoBefore)),
                "unverified": addresses(ground.on(Footing::Unverified)),
                "unsupervised": claimed(&ground.unsupervised),
                "edges": kinds.iter().map(|((kind, source), count)| serde_json::json!({
                    "kind": kind, "source": source, "count": count,
                })).collect::<Vec<_>>(),
                "skill_stale": skill_stale.iter().map(|s| &s.path).collect::<Vec<_>>(),
                "content_versioning": !no_git,
                "chain_break": chain_break,
                "provider_warnings": provider_warnings, "cache_fault": cache_fault,
                "declared_providers": declared_providers.iter().map(|(name, decl)| serde_json::json!({
                    "provider": name, "can": decl.can(), "caveat": decl.caveat(),
                })).collect::<Vec<_>>(),
                "notes": faults.iter().map(|f| serde_json::json!({
                    "note": f.note, "key": f.key, "code": f.code, "detail": f.detail,
                    "breaks": f.breaks(), "blocks": f.blocks(),
                })).collect::<Vec<_>>(),
                "spending": spending.iter().map(|row| serde_json::json!({
                    "session": row.session, "verb": row.verb,
                    "calls": row.calls, "bytes": row.bytes,
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(exit_code);
    }

    if let Some(seq) = chain_break {
        println!(
            "journal   BROKEN at seq {seq} — this entry's link does not cover it. \
             The log is append-only by trigger; something got past that, or the file \
             was edited underneath. Do not trust readings at or after this point"
        );
    }
    println!("anchors   {} (live {})", corpus.len(), live.len());
    for (name, decl) in &declared_providers {
        println!("provider  {name}   {}", decl.can().join(" · "));
        if let Some(caveat) = decl.caveat() {
            println!("          <- {caveat}");
        }
    }
    if !states.is_empty() {
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for s in &states {
            *counts.entry(s.as_str()).or_default() += 1;
        }
        let line: Vec<String> = counts.iter().map(|(s, n)| format!("{s}x{n}")).collect();
        println!("status    {}", line.join("  "));
    }
    if !kinds.is_empty() {
        let line: Vec<String> = kinds
            .iter()
            .map(|((kind, source), n)| format!("{kind}x{n} ({source})"))
            .collect();
        println!("edges     {}", line.join("  "));
    }
    if !absent.is_empty() {
        println!(
            "absent    {}\n          <- the probe has not seen anything yet; this is normal when criteria are written before implementation",
            absent.join(", ")
        );
    }
    for (kind, label, whose) in [
        (
            KnowledgeKind::Unreachable,
            "unreachable",
            "the probe could not be reached — somebody else's service or your credentials",
        ),
        (
            KnowledgeKind::Unusable,
            "unusable  ",
            "the probe answered and the answer cannot be used — whoever writes the probe",
        ),
        (
            KnowledgeKind::Unevaluable,
            "unevaluable",
            "the rules cannot be evaluated against what came back — whoever wrote the rules",
        ),
        (
            KnowledgeKind::NeverAsked,
            "never asked",
            "our clock ran out before their turn, not anybody's outage",
        ),
    ] {
        let keys = blind(kind);
        if !keys.is_empty() {
            println!("{label} {}\n          <- {whose}", keys.join(", "));
        }
    }
    if let Some(line) = counted(&watched) {
        println!(
            "moved      {line}\n          <- the ground under these records moved after they \
             were bound, on an axis their note subscribes to. `gmr check` hands them back"
        );
    }
    if let Some(line) = counted(&quiet) {
        println!(
            "quiet      {line}\n          <- moved on axes their note's `watch:` does not name. \
             `check` will not hand these back and is not meant to; `gmr status` shows them"
        );
    }
    for (kind, label, whose) in [
        (
            HoldingKind::Finished,
            "finished  ",
            "the anchor under these has finished; its journal is frozen, so this warrant can never change again",
        ),
        (
            HoldingKind::Incomparable,
            "incomparable",
            "a different extractor took the reading these are dated against, so a diff would answer `the instrument changed shape`, not `the world moved`",
        ),
        (
            HoldingKind::Absent,
            "absent gnd",
            "the coordinate these records are about is not there any more",
        ),
        (
            HoldingKind::NeverEstablished,
            "no ground ",
            "bound at a seq that predates the anchor's first entry — there was no ground yet",
        ),
        (
            HoldingKind::Undated,
            "undated   ",
            "written before bindings carried a seq, so they cannot be compared against the log at all",
        ),
    ] {
        if let Some(line) = grounds_line(ground, kind) {
            println!("{label} {line}\n          <- {whose}");
        }
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
    if !ground.unsupervised.is_empty() {
        println!(
            "unsupervised {}\n             <- every anchor these are bound to has finished, or was never opened. The record still claims something about the code and nothing observes it any more — which is the state this tool exists to make visible. Supersede the anchor into a new generation, point the note somewhere still watched, or unbind it",
            spelled_claims(&ground.unsupervised, names)
        );
    }
    if !ground.on(Footing::Gone).is_empty() {
        println!(
            "gone      {}\n          <- the provider says these records no longer exist. Restore them or detach the binding; until then these anchors are watched on behalf of nothing",
            spelled(ground.on(Footing::Gone), names)
        );
    }
    if !ground.on(Footing::NoProvider).is_empty() {
        println!(
            "no provider {}\n            <- bound through a provider this binary does not have. Rebuild with that feature, or the binding cannot be read here at all",
            spelled(ground.on(Footing::NoProvider), names)
        );
    }
    if !ground.on(Footing::Unreachable).is_empty() {
        println!(
            "unreachable {} record(s) could not be reached this run\n            <- somebody else's service, not something to fix here. Reported, never counted against the exit code",
            ground.on(Footing::Unreachable).len()
        );
    }
    if !ground.on(Footing::NeverAsked).is_empty() {
        println!(
            "unasked   {} of {} bound record(s) were never asked about — the total content budget ran out first\n          <- what is printed above is that partial view, not the whole repository. Raise --content-total-ms to see the rest",
            ground.on(Footing::NeverAsked).len(),
            ground.grounded_records()
        );
    }
    if !ground.on(Footing::Unverified).is_empty() {
        println!(
            "unverified {} record(s) have never been compared against what the store holds\n          <- they were asserted when the store could not answer, so there is no baseline to move away from. A `gmr bind` or `gmr reaffirm` that reaches the store establishes one",
            ground.on(Footing::Unverified).len()
        );
    }
    if !ground.on(Footing::NoBefore).is_empty() {
        println!(
            "no before {} rewritten record(s) cannot show what they said at binding time\n          <- the provider keeps no history, or did not keep that version. You are still told they moved; you just have to re-read the whole thing instead of a diff",
            ground.on(Footing::NoBefore).len()
        );
    }
    for s in &skill_stale {
        println!(
            "skill     {}\n          <- this copy is not the SKILL.md in this binary. `gmr init` only ever writes it when absent, so an upgraded binary leaves the old text in place and agents keep reading contracts this build no longer honours. Delete it and re-run `{}`",
            s.path, s.refresh
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
    if !spending.is_empty() {
        let sessions: std::collections::BTreeSet<&str> =
            spending.iter().map(|row| row.session.as_str()).collect();
        let calls: u64 = spending.iter().map(|row| row.calls).sum();
        let bytes: u64 = spending.iter().map(|row| row.bytes).sum();
        println!(
            "ledger    {} session(s), {calls} call(s), {bytes} envelope byte(s) served \
             through the doors",
            sessions.len()
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
    fn a_record_nobody_subscribed_to_is_counted_apart_from_one_check_will_hand_back() {
        let key = AnchorKey::new("a");
        let note = gmr::Ref::new("git", "memories/n.md");
        let watched = [(&key, &note)];
        let none: [(&AnchorKey, &gmr::Ref); 0] = [];

        assert_eq!(
            counted(&watched).as_deref(),
            Some("1 record(s) on 1 anchor(s)")
        );
        assert_eq!(
            counted(&none),
            None,
            "an empty bucket prints no line at all, so a reader is never told to go \
             read something back that `check` was built to stay silent about"
        );
    }

    #[test]
    fn two_records_on_one_anchor_are_two_records_and_one_anchor() {
        let key = AnchorKey::new("a");
        let (one, two) = (
            gmr::Ref::new("git", "memories/one.md"),
            gmr::Ref::new("git", "memories/two.md"),
        );
        assert_eq!(
            counted(&[(&key, &one), (&key, &two)]).as_deref(),
            Some("2 record(s) on 1 anchor(s)"),
            "the count a reader acts on is records; the anchors are how many places \
             to look, and collapsing them would make one busy anchor read as many"
        );
    }

    #[test]
    fn every_condition_this_repositorys_owner_can_act_on_turns_it_red() {
        let each: [fn(&mut Verdict); 9] = [
            |v| v.stranded = true,
            |v| v.provider_unavailable = true,
            |v| v.breaking_notes = true,
            |v| v.undeclared = true,
            |v| v.gone = true,
            |v| v.no_provider = true,
            |v| v.skill_stale = true,
            |v| v.unsupervised = true,
            |v| v.chain_broken = true,
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
            9,
            "Verdict is one bool per condition that makes this run red, and a store being \
             unreachable is deliberately not one of them: nobody holding this repository can \
             fix somebody else's service, and a build that fails on it fails for a reason its \
             owner cannot act on. Adding a field here means claiming otherwise. \
             `chain_broken` is a claim of exactly that kind and it holds: the journal is \
             this repository's own file, append-only by trigger, and a link that no longer \
             covers its row means something got past that or edited it underneath -- \
             whoever holds the repository is the only one who can go and look"
        );
    }
}
