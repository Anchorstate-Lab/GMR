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
    let silent: Vec<String> = stores
        .silent(provider.as_deref())
        .iter()
        .map(|s| s.provider().to_string())
        .collect();
    if wanted.is_empty() && silent.is_empty() {
        return Err(CliError(format!(
            "no store here is registered{}. Stores that are: {}",
            provider.map_or(String::new(), |p| format!(" under the name `{p}`")),
            match stores.registered().is_empty() {
                true => "none in this binary".to_owned(),
                false => stores.registered().join(", "),
            }
        )));
    }

    let budget = rt.content_budget();
    let mut rows = Vec::new();
    let mut silent_now = Vec::new();
    for store in wanted {
        let source = store.source().expect("filtered to stores that list");
        let held = match source.list(&budget).await {
            Ok(held) => held,
            Err(e) => {
                silent_now.push((store.provider().to_string(), e.to_string()));
                continue;
            }
        };
        for record in held {
            let bound = rt.memory().binding_of(&record.reference).await?;
            let anchors = (!bound.is_empty()).then(|| {
                bound
                    .iter()
                    .flat_map(|b| b.binding.anchors.iter().map(|a| a.to_string()))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            });
            rows.push((
                crate::memories::addressed(&record.reference),
                anchors,
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
                "cannot_list": silent,
                "would_not_answer": silent_now.iter().map(|(provider, why)| serde_json::json!({
                    "provider": provider, "why": why,
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
    for provider in &silent {
        println!("  ?         {provider} is registered here and cannot list what it holds");
    }
    for (provider, why) in &silent_now {
        println!("  !         {provider} would not answer this run: {why}");
    }
    println!(
        "\n{} record(s), {free} bound to nothing.\n\
         A listing is what a store will show, not a roster of what exists — a record missing \
         from it is not a dead reference. Bind one with `gmr bind --provider <p> <reference>`.",
        rows.len()
    );
    if !silent.is_empty() {
        println!(
            "A store that cannot list is not empty and not broken: nothing here can enumerate \
             it, so a record in it has to be named by an address you already hold."
        );
    }
    if !silent_now.is_empty() {
        println!(
            "A store that would not answer has told you nothing, including nothing about \
             whether it holds anything. What every other store here shows is unaffected."
        );
    }
    Ok(0)
}
