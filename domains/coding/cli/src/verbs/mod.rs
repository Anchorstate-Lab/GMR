pub mod accept;
pub mod anchor;
pub mod atlas;
pub mod bind;
pub mod check;
pub mod close;
pub mod cobound;
pub mod doctor;
pub mod edges;
pub mod export;
pub mod health;
pub mod import;
pub mod init;
pub mod link;
pub mod memories;
pub mod observe;
pub mod open;
pub mod pass;
pub mod probes;
pub mod publish;
pub mod read;
pub mod reaffirm;
pub mod rebase;
pub mod requeue;
pub mod revise;
pub mod status;
pub mod sync;

use gmr::{AnchorKey, AnchorView, Change, ContentHash, Revised, Runtime, State};

use crate::error::CliError;

fn shared_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

fn nearest(all: &[AnchorKey], arg: &str) -> String {
    let mut ranked: Vec<&AnchorKey> = all.iter().collect();
    ranked.sort_by_key(|k| {
        (
            std::cmp::Reverse(shared_prefix(k.as_str(), arg)),
            k.as_str(),
        )
    });
    let lines: Vec<String> = ranked.iter().take(3).map(|k| format!("    {k}")).collect();
    match lines.is_empty() {
        true => "Nothing is anchored yet; `gmr anchor <coordinate>` opens the first one.".into(),
        false => format!("Nearest:\n{}", lines.join("\n")),
    }
}

fn pick(all: &[AnchorKey], arg: &str) -> Vec<AnchorKey> {
    match all.iter().find(|k| k.as_str() == arg) {
        Some(hit) => vec![hit.clone()],
        None => all
            .iter()
            .filter(|k| k.as_str().starts_with(arg))
            .cloned()
            .collect(),
    }
}

pub(crate) async fn resolve(rt: &Runtime, arg: &str) -> Result<Vec<AnchorKey>, CliError> {
    let all = rt.anchors().await?;
    let hits = pick(&all, arg);
    match hits.is_empty() {
        true => Err(CliError(format!(
            "no anchor matches `{arg}`.\n{}",
            nearest(&all, arg)
        ))),
        false => Ok(hits),
    }
}

pub(crate) async fn resolve_one(rt: &Runtime, arg: &str) -> Result<AnchorKey, CliError> {
    let mut hits = resolve(rt, arg).await?;
    if hits.len() > 1 {
        let list: Vec<String> = hits.iter().map(|k| format!("    {k}")).collect();
        return Err(CliError(format!(
            "`{arg}` covers {} anchors, and this changes one:\n{}\n\
             Name the one you mean. Each of these is a separate judgment.",
            hits.len(),
            list.join("\n")
        )));
    }
    Ok(hits.remove(0))
}

pub(crate) async fn recapture(
    rt: &Runtime,
    key: &AnchorKey,
    why: &[u8],
) -> Result<Revised, CliError> {
    let view = rt.read(key).await?;
    let blank = State::new(serde_json::json!({ "position": view.state.position() }));
    let revised = rt
        .revise(key, Change::Restate { state: blank }, why)
        .await?;
    rt.observe(key).await?;
    Ok(revised)
}

pub(crate) async fn owed(
    rt: &Runtime,
    subs: &crate::delivery::Subscriptions,
    key: &AnchorKey,
) -> Result<Vec<gmr::Ref>, CliError> {
    let view = rt.read(key).await?;
    let shape = crate::shapes::of(&view.anchor.transitions);
    let bound = memories_on(rt, key).await?;
    if bound.is_empty() {
        return Ok(Vec::new());
    }

    let entries = rt.log().entries(key, 0).await?;
    let sealed = entries
        .iter()
        .filter(|(_, e)| matches!(e, gmr::Entry::Revise { .. }))
        .map(|(seq, _)| *seq)
        .next_back()
        .unwrap_or(0);

    let mut out = Vec::new();
    for m in bound {
        let raised = entries
            .iter()
            .filter(|(seq, _)| *seq >= sealed)
            .any(|(_, e)| {
                let state = match e {
                    gmr::Entry::Open { state, .. } | gmr::Entry::Transition { state, .. } => state,
                    _ => return false,
                };
                subs.delivers(key.as_str(), shape, &m, state)
                    .unwrap_or(false)
            });
        if raised {
            out.push(m);
        }
    }
    Ok(out)
}

pub(crate) async fn memories_on(rt: &Runtime, key: &AnchorKey) -> Result<Vec<gmr::Ref>, CliError> {
    Ok(rt
        .memory()
        .bindings_on(rt.log(), key)
        .await?
        .iter()
        .filter_map(|b| b.stored().cloned())
        .collect())
}

pub(crate) fn swapped(rt: &Runtime, views: &[AnchorView]) -> Vec<(AnchorKey, String)> {
    let mut out = Vec::new();
    for view in views {
        if view.closed {
            continue;
        }
        let (Some(was), Ok(now)) = (&view.derivation, rt.instrument(&view.anchor.probe)) else {
            continue;
        };
        if was.version != now.version {
            out.push((
                view.key.clone(),
                format!(
                    "{} -> {}",
                    &was.version.as_str()[..12],
                    &now.version.as_str()[..12]
                ),
            ));
        }
    }
    out
}

pub(crate) fn sealed(context: &ContentHash, rationale: &ContentHash) {
    println!(
        "  context   {} (captured by substrate, cannot be forged)",
        context.short()
    );
    println!(
        "  rationale {} (written by you; substrate only preserves tamper evidence)",
        rationale.short()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<AnchorKey> {
        ["a.rs#f", "a.rs#g", "b.rs#f", "doctrine::rules"]
            .into_iter()
            .map(AnchorKey::new)
            .collect()
    }

    #[test]
    fn an_exact_key_wins_over_the_prefix_it_also_is() {
        let all = [AnchorKey::new("a.rs#f"), AnchorKey::new("a.rs#foo")];
        assert_eq!(pick(&all, "a.rs#f"), [AnchorKey::new("a.rs#f")]);
    }

    #[test]
    fn a_file_names_every_anchor_under_it() {
        assert_eq!(
            pick(&keys(), "a.rs"),
            [AnchorKey::new("a.rs#f"), AnchorKey::new("a.rs#g")]
        );
    }

    #[test]
    fn a_typo_is_told_what_it_nearly_said() {
        assert!(pick(&keys(), "a.rs#ff").is_empty());
        let said = nearest(&keys(), "a.rs#ff");
        assert!(
            said.lines().nth(1).is_some_and(|l| l.contains("a.rs#f")),
            "the closest key must come first, got:\n{said}"
        );
    }

    #[test]
    fn nothing_anchored_says_so_instead_of_listing_nothing() {
        assert!(nearest(&[], "x").contains("gmr anchor"));
    }
}
