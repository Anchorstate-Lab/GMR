use std::path::Path;

use gmr::{AnchorKey, Looked, Observed, Runtime, State};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::memories::Names;
use crate::probes::Catalog;

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &Names,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match key {
        Some(k) => super::resolve(rt, &k).await?,
        None => rt.anchors().await?,
    };
    let (subs, _) = Subscriptions::load(root, &Catalog::load(root)?, names)?;

    let mut moved = 0;
    let mut handed = 0;
    let mut unclaimed = Vec::new();
    let mut report = Vec::new();
    for key in &keys {
        let Looked { before, observed } = rt.look(key).await?;
        let (word, detail) = match &observed {
            Observed::Unchanged { .. } => ("settled", None),
            Observed::Transitioned { to, .. } => {
                moved += 1;
                ("moved", to.status().map(|s| s.to_string()))
            }
            Observed::Still => ("still", None),
            Observed::Attempt { code, message, .. } => {
                ("unseen", Some(format!("{code:?}: {message}")))
            }
            Observed::Closed => ("closed", None),
            Observed::Contended => (
                "contended",
                Some("another writer recorded first; nothing was written".to_owned()),
            ),
        };

        let memories = match &observed {
            Observed::Transitioned { to, .. } => {
                let shape = crate::shapes::of(&before.anchor.transitions);
                delivered(rt, &subs, key, shape, to, true, &mut unclaimed).await?
            }
            _ => Vec::new(),
        };
        handed += memories.len();

        if json {
            let state = match &observed {
                Observed::Transitioned { to, .. } => Some(to.as_value()),
                _ => None,
            };
            report.push(serde_json::json!({
                "anchor": key, "observed": word, "detail": detail,
                "state": state, "memories": addressed_all(&memories),
            }));
        } else if word != "still" {
            match &detail {
                Some(d) => println!("{key}  {word}  {d}"),
                None => println!("{key}  {word}"),
            }
            for m in &memories {
                println!("    → {}", names.of(m));
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "observed {} anchors, {moved} moved, {handed} handed back",
            keys.len()
        );
        report_unclaimed(&unclaimed);
    }
    Ok(if moved > 0 { 1 } else { 0 })
}

pub(crate) type Snag = (AnchorKey, gmr::Ref, String);

pub(crate) async fn delivered(
    rt: &Runtime,
    subs: &Subscriptions,
    key: &AnchorKey,
    shape: Option<&crate::shapes::Shape>,
    to: &State,
    moved: bool,
    unclaimed: &mut Vec<AnchorKey>,
) -> Result<Vec<gmr::Ref>, CliError> {
    let mut snags = Vec::new();
    let out = settled(rt, subs, key, shape, to, moved, unclaimed, &mut snags).await?;
    report_snags(&snags);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn settled(
    rt: &Runtime,
    subs: &Subscriptions,
    key: &AnchorKey,
    shape: Option<&crate::shapes::Shape>,
    to: &State,
    moved: bool,
    unclaimed: &mut Vec<AnchorKey>,
    snags: &mut Vec<Snag>,
) -> Result<Vec<gmr::Ref>, CliError> {
    let bound = super::memories_on(rt, key).await?;
    if bound.is_empty() {
        if moved {
            unclaimed.push(key.clone());
        }
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for m in bound {
        match subs.delivers(key.as_str(), shape, &m, to) {
            Ok(true) => out.push(m),
            Ok(false) => {}
            Err(why) => snags.push((key.clone(), m, why)),
        }
    }
    Ok(out)
}

pub(crate) fn report_snags(snags: &[Snag]) {
    if snags.is_empty() {
        return;
    }
    println!(
        "\n{} memories could not be answered for — whether to hand them over has no answer:",
        snags.len()
    );
    for (key, reference, why) in snags {
        println!("  ? {key}  {}  {why}", reference.external_id);
    }
}

pub(crate) fn report_unclaimed(unclaimed: &[AnchorKey]) {
    if unclaimed.is_empty() {
        return;
    }
    println!("\n{} moved with no note bound to them:", unclaimed.len());
    for k in unclaimed {
        println!("  ? {k}");
    }
}

pub(crate) fn addressed_all(refs: &[gmr::Ref]) -> Vec<String> {
    refs.iter().map(crate::memories::addressed).collect()
}
