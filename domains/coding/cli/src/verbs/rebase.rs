use std::path::Path;

use gmr::{AnchorKey, Runtime};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::memories::Names;
use crate::probes::Catalog;
use crate::verbs::sealed;

async fn standing(
    rt: &Runtime,
    subs: &Subscriptions,
    key: &AnchorKey,
) -> Result<Vec<gmr::Ref>, CliError> {
    let view = rt.read(key).await?;
    let shape = crate::shapes::of(&view.anchor.transitions);
    let mut unclaimed = Vec::new();
    super::observe::delivered(rt, subs, key, shape, &view.state, false, &mut unclaimed).await
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &Names,
    keys: Vec<String>,
    all: bool,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let keys: Vec<AnchorKey> = match all {
        true => {
            let mut views = Vec::new();
            for key in rt.anchors().await? {
                views.push(rt.read(&key).await?);
            }
            crate::verbs::swapped(rt, &views)
                .into_iter()
                .map(|(key, _)| key)
                .collect()
        }
        false => {
            let mut out = Vec::new();
            for arg in &keys {
                out.push(crate::verbs::resolve_one(rt, arg).await?);
            }
            out
        }
    };
    if keys.is_empty() {
        println!("no anchor is standing on a reading a different instrument took");
        return Ok(0);
    }

    let (subs, _) = Subscriptions::load(root, &Catalog::load(root)?, names)?;
    let mut refused = Vec::new();
    let mut recapturing = Vec::new();
    for key in keys {
        match standing(rt, &subs, &key).await? {
            owed if owed.is_empty() => recapturing.push(key),
            owed => refused.push((key, owed)),
        }
    }

    let mut done = Vec::new();
    for key in &recapturing {
        done.push((
            key.clone(),
            crate::verbs::recapture(rt, key, why.as_bytes()).await?,
        ));
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "rebased": done.iter().map(|(k, _)| k).collect::<Vec<_>>(),
                "refused": refused.iter().map(|(k, owed)| serde_json::json!({
                    "anchor": k, "owed": super::observe::addressed_all(owed)
                })).collect::<Vec<_>>(),
                "context": done.first().map(|(_, r)| &r.context),
                "rationale": done.first().map(|(_, r)| &r.rationale),
            })
        );
        return Ok(i32::from(!refused.is_empty()));
    }

    for (key, _) in &done {
        println!("{key} recaptured");
    }
    if let Some((_, revised)) = done.first() {
        sealed(&revised.context, &revised.rationale);
    }

    if !refused.is_empty() {
        println!(
            "\n{} anchors were not recaptured: a judgement is outstanding on them, and \
             recapturing pins the world as it is now, which would answer that judgement \
             without anybody having looked.",
            refused.len()
        );
        for (key, owed) in &refused {
            println!(
                "  ! {key}   {}",
                owed.iter().map(|m| names.of(m)).collect::<Vec<_>>().join(" · ")
            );
        }
        println!(
            "\nRead each one and say what you decided — `gmr accept <key> --why \"...\"` \
             seals it, `gmr close <key> --why \"...\"` retires it — then rebase again."
        );
    }
    Ok(i32::from(!refused.is_empty()))
}
