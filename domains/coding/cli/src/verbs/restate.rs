use gmr::{AnchorKey, Change, Runtime, State};

use crate::error::CliError;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    state: String,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let value: serde_json::Value =
        serde_json::from_str(&state).map_err(|e| CliError(format!("新状态不是合法 JSON：{e}")))?;
    if !value.is_object() {
        return Err(CliError("新状态得是一个对象".into()));
    }

    let before = rt.read(&key).await?;
    let revised = rt
        .revise(
            &key,
            Change::Restate {
                state: State::new(value.clone()),
            },
            why.as_bytes(),
        )
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "restated": key, "from": before.state, "to": value,
                "context": revised.context, "rationale": revised.rationale,
            })
        );
    } else {
        println!("{key} 的状态改了");
        println!("  从  {}", before.state.as_value());
        println!("  到  {value}");
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
