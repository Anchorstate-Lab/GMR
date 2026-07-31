use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, json: bool) -> Result<i32, CliError> {
    let views = rt.read_all().await?;
    let live: Vec<_> = views.iter().filter(|v| !v.closed).collect();

    let unseen: Vec<&str> = live
        .iter()
        .filter(|v| v.attempts > 0)
        .map(|v| v.key.as_str())
        .collect();
    let absent: Vec<&str> = live
        .iter()
        .filter(|v| v.sighting == gmr::Sighting::Absent)
        .map(|v| v.key.as_str())
        .collect();
    let barren: Vec<&str> = live
        .iter()
        .filter(|v| v.memories.is_empty())
        .map(|v| v.key.as_str())
        .collect();
    let states: Vec<String> = live
        .iter()
        .filter_map(|v| v.status.as_ref().map(|s| s.to_string()))
        .collect();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "anchors": views.len(), "live": live.len(),
                "absent": absent, "unseen": unseen, "barren": barren,
            })
        );
        return Ok(0);
    }

    println!("锚        {}（活着的 {}）", views.len(), live.len());
    if !states.is_empty() {
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for s in &states {
            *counts.entry(s.as_str()).or_default() += 1;
        }
        let line: Vec<String> = counts.iter().map(|(s, n)| format!("{s}×{n}")).collect();
        println!("状态      {}", line.join("  "));
    }
    if !absent.is_empty() {
        println!(
            "还没锚上  {}\n          ← 探针至今什么都没看到。先写判据后做实现时这是正常的",
            absent.join(" · ")
        );
    }
    if !unseen.is_empty() {
        println!("没看成    {}  ← 修探针或凭证", unseen.join(" · "));
    }
    if !barren.is_empty() {
        println!(
            "没有记忆  {}\n          ← 在守一个没人写过东西的位置，纯观测开销",
            barren.join(" · ")
        );
    }
    Ok(0)
}
