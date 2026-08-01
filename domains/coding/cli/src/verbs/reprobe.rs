use gmr::{AnchorKey, Change, Runtime};

use crate::error::CliError;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    artifact: String,
    params: String,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let probe = crate::rules::probe(&artifact, &params)?;
    let revised = rt
        .revise(&key, Change::Reprobe { probe }, why.as_bytes())
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "reprobed": key,
                "context": revised.context,
                "rationale": revised.rationale,
                "incomparable_state": revised.incomparable_state,
            })
        );
    } else {
        println!("{key} 换了探针");
        if revised.incomparable_state {
            println!(
                "  ! 状态里的值是用**另一条派生规则**算出来的，跟新观测不可比。\n    \
                 要么 restate 重新捕获，要么明确接受它跨规则仍然可比 —— 这是你的断言，基底只记录"
            );
        }
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
