use gmr::{AnchorKey, Change, Runtime, State};

use crate::error::CliError;
use crate::verbs::sealed;

/// Recapture against the instrument that is in the build now.
///
/// A swapped derivation makes the stored baseline incomparable, and that is a
/// change of criteria — the substrate will not make it silently. One rationale
/// covers the whole batch, because one upgrade is one decision.
pub async fn run(
    rt: &Runtime,
    keys: Vec<String>,
    all: bool,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let keys = match all {
        true => swapped(rt).await?,
        false => keys.into_iter().map(AnchorKey::new).collect(),
    };
    if keys.is_empty() {
        println!("no anchor is standing on a reading a different instrument took");
        return Ok(0);
    }

    let mut done = Vec::new();
    for key in &keys {
        let view = rt.read(key).await?;
        // Exactly what `open` seeds, so this is a replay of opening, not a new
        // path the rules have never been down.
        let blank = State::new(serde_json::json!({ "position": view.state.position() }));
        let revised = rt
            .revise(key, Change::Restate { state: blank }, why.as_bytes())
            .await?;
        rt.observe(key).await?;
        done.push((key.clone(), revised));
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "rebased": done.iter().map(|(k, _)| k).collect::<Vec<_>>(),
                "context": done.first().map(|(_, r)| &r.context),
                "rationale": done.first().map(|(_, r)| &r.rationale),
            })
        );
        return Ok(0);
    }

    for (key, _) in &done {
        println!("{key} recaptured");
    }
    if let Some((_, revised)) = done.first() {
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}

/// Anchors whose last reading was taken by a rule this build no longer has.
async fn swapped(rt: &Runtime) -> Result<Vec<AnchorKey>, CliError> {
    let mut out = Vec::new();
    for view in rt.read_all().await? {
        if view.closed {
            continue;
        }
        let (Some(was), Ok(now)) = (&view.derivation, rt.instrument(&view.anchor.probe)) else {
            continue;
        };
        if was.version != now.version {
            out.push(view.key.clone());
        }
    }
    Ok(out)
}
