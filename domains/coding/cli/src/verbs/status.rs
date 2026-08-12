use std::path::Path;

use gmr::{AnchorView, Runtime};

use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::{Context, DEFAULT_FILE, differs, merged, read_declared};

fn axes_line(view: &AnchorView) -> Option<String> {
    let v = view.state.as_value().get("v")?.as_object()?;
    Some(
        v.iter()
            .map(|(k, on)| format!("{k} {}", u8::from(on.as_bool().unwrap_or(false))))
            .collect::<Vec<_>>()
            .join("  "),
    )
}

fn unwritten(root: &Path, note: &str) -> bool {
    match std::fs::read_to_string(root.join(note)) {
        Ok(text) => match text.split_once("\n---") {
            Some((_, body)) => {
                let body = body.trim_start_matches('-').trim();
                body.is_empty() || body == super::anchor::UNWRITTEN
            }
            None => false,
        },
        Err(_) => false,
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    key: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let views = match &key {
        Some(k) => {
            let mut out = Vec::new();
            for key in super::resolve(rt, k).await? {
                out.push(rt.read(&key).await?);
            }
            out
        }
        None => rt.read_all().await?,
    };
    let live: Vec<&AnchorView> = views.iter().filter(|v| !v.closed).collect();

    let ctx = Context {
        catalog: Catalog::load(root)?,
    };
    let declared = read_declared(root, DEFAULT_FILE)?;
    let crate::memories::Scanned { notes, broken, .. } = crate::memories::scan(root, &ctx.catalog)?;
    let decls = merged(&declared, &notes);

    let mut rows = Vec::new();
    let mut drifted = Vec::new();
    let mut unreadable = Vec::new();
    for view in &live {
        let shape = crate::shapes::name_of(&view.anchor.transitions).unwrap_or("custom");
        match decls.iter().find(|d| d.key == view.key.as_str()) {
            Some(decl) => {
                let facets = differs(&view.anchor, decl, &ctx)?;
                if !facets.is_empty() {
                    drifted.push((view.key.to_string(), facets.join(" · ")));
                }
            }
            None => {
                if let Some(b) = broken
                    .iter()
                    .find(|b| b.key.as_deref() == Some(view.key.as_str()))
                {
                    unreadable.push((view.key.to_string(), b.reason.clone()));
                }
            }
        }
        let memories: Vec<(String, bool)> = view
            .memories
            .iter()
            .map(|m| {
                let path = m.reference.external_id.as_str().to_owned();
                let blank = unwritten(root, &path);
                (path, blank)
            })
            .collect();
        rows.push((view, shape, axes_line(view), memories));
    }

    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|(v, shape, _, memories)| {
                serde_json::json!({
                    "anchor": v.key, "shape": shape,
                    "status": v.status.as_ref().map(|s| s.to_string()),
                    "state": v.state,
                    "memories": memories.iter().map(|(p, blank)| serde_json::json!({
                        "note": p, "unwritten": blank
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "anchors": out, "criteria_drifted": drifted,
                "criteria_unreadable": unreadable.iter().map(|(k, r)| serde_json::json!({
                    "anchor": k, "reason": r
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(0);
    }

    let bound = rows.iter().filter(|(_, _, _, m)| !m.is_empty()).count();
    let blank = rows
        .iter()
        .filter(|(_, _, _, m)| m.iter().any(|(_, b)| *b))
        .count();
    println!(
        "{} anchors · {bound} with memories{}",
        live.len(),
        match blank {
            0 => String::new(),
            n => format!(" · {n} unwritten"),
        }
    );

    for (view, shape, axes, memories) in &rows {
        let status = view
            .status
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unseen".to_owned());
        println!("\n  {}   {shape}   {status}", view.key);
        if let Some(axes) = axes {
            println!("    v  {axes}");
        }
        if let Some(d) = crate::render::diagnosis(view.facts.as_ref()) {
            println!("    ?  {d}");
        }
        for (note, blank) in memories {
            match blank {
                true => println!("    ! {note}   unwritten"),
                false => println!("    → {note}"),
            }
        }
        if memories.is_empty() {
            println!("    ? no memory is bound to this anchor");
        }
    }

    if !drifted.is_empty() {
        println!(
            "\n{} anchors whose declaration no longer matches their criteria:",
            drifted.len()
        );
        for (key, facets) in &drifted {
            println!("  != {key}  ({facets})");
            println!("     gmr accept {key} --criteria --why '...'");
        }
    }
    if !unreadable.is_empty() {
        println!(
            "\n{} anchors whose declaration this run could not read:",
            unreadable.len()
        );
        for (key, reason) in &unreadable {
            println!("  ?! {key}  ({reason})");
        }
    }
    Ok(0)
}
