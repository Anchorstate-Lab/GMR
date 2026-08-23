use gmr::{AnchorKey, Runtime};

use crate::error::CliError;
use crate::verbs::sealed;

async fn standing(rt: &Runtime, key: &AnchorKey) -> Result<Vec<String>, CliError> {
    let view = rt.read(key).await?;
    Ok(crate::delivery::axes_set(&view.state).unwrap_or_default())
}

pub async fn run(
    rt: &Runtime,
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

    let mut refused = Vec::new();
    let mut recapturing = Vec::new();
    for key in keys {
        match standing(rt, &key).await? {
            axes if axes.is_empty() => recapturing.push(key),
            axes => refused.push((key, axes)),
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
                "refused": refused.iter().map(|(k, axes)| serde_json::json!({
                    "anchor": k, "standing": axes
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
        for (key, axes) in &refused {
            println!("  ! {key}   {}", axes.join(" · "));
        }
        println!(
            "\nRead each one and say what you decided — `gmr accept <key> --why \"...\"` \
             seals it, `gmr close <key> --why \"...\"` retires it — then rebase again."
        );
    }
    Ok(i32::from(!refused.is_empty()))
}
