use gmr::{AnchorKey, Change, Runtime};

use crate::error::CliError;
use crate::rules;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    terminal: Vec<String>,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let want = rules::terminal(&terminal);
    let revised = rt
        .revise(
            &key,
            Change::Reterminal {
                terminal: want.clone(),
            },
            why.as_bytes(),
        )
        .await?;

    let names: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "reterminal": key, "terminal": names,
                "context": revised.context, "rationale": revised.rationale,
            })
        );
    } else {
        let view = rt.read(&key).await?;
        println!("{key} 的终结集合现在是：{}", names.join(" · "));
        if view.closed {
            println!("  → 它当前的状态就在这个集合里，这个锚现在是关的");
        }
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
