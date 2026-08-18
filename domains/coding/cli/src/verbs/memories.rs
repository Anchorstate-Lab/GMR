use gmr::Runtime;

use crate::error::CliError;
use crate::stores::Stores;

fn excerpt(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty() && !l.starts_with("---"))
        .unwrap_or("")
        .trim();
    match line.chars().count() > 60 {
        true => format!("{}…", line.chars().take(59).collect::<String>()),
        false => line.to_owned(),
    }
}

pub async fn run(
    rt: &Runtime,
    stores: &Stores,
    provider: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let wanted = stores.listing(provider.as_deref());
    if wanted.is_empty() {
        return Err(CliError(format!(
            "no store here can list what it holds{}. Stores that can: {}",
            provider.map_or(String::new(), |p| format!(" under the name `{p}`")),
            match stores.listing(None).is_empty() {
                true => "none in this binary".to_owned(),
                false => stores
                    .listing(None)
                    .iter()
                    .map(|s| s.provider().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        )));
    }

    let budget = rt.content_budget();
    let mut rows = Vec::new();
    for store in wanted {
        let source = store.source().expect("filtered to stores that list");
        for record in source.list(&budget).await? {
            let bound = rt.memory().binding_of(&record.reference).await?;
            rows.push((
                crate::memories::addressed(&record.reference),
                bound.map(|b| {
                    b.binding
                        .anchors
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                }),
                excerpt(&record.bytes),
            ));
        }
    }
    rows.sort();

    let free = rows.iter().filter(|(_, bound, _)| bound.is_none()).count();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "records": rows.iter().map(|(reference, bound, excerpt)| serde_json::json!({
                    "reference": reference, "anchors": bound, "excerpt": excerpt,
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(0);
    }

    for (reference, bound, excerpt) in &rows {
        match bound {
            Some(anchors) if anchors.is_empty() => println!("  detached  {reference}  {excerpt}"),
            Some(anchors) => println!("  bound     {reference}  → {}", anchors.join(", ")),
            None => println!("  free      {reference}  {excerpt}"),
        }
    }
    println!(
        "\n{} record(s), {free} bound to nothing.\n\
         A listing is what a store will show, not a roster of what exists — a record missing \
         from it is not a dead reference. Bind one with `gmr bind --provider <p> <reference>`.",
        rows.len()
    );
    Ok(0)
}
