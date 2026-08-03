use gmr::{AnchorKey, AnchorView, Runtime, StatusId};

use crate::{error::CliError, render};

pub async fn run(
    rt: &Runtime,
    key: Option<String>,
    status: Vec<String>,
    not_status: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    let views = match key {
        Some(k) => vec![rt.read(&AnchorKey::new(k)).await?],
        None => rt.read_all().await?,
    };

    let wanted: Vec<StatusId> = status.iter().map(StatusId::new).collect();
    let unwanted: Vec<StatusId> = not_status.iter().map(StatusId::new).collect();
    let shown: Vec<_> = views
        .iter()
        .filter(|v| keep(v, &wanted, &unwanted))
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
    } else if shown.is_empty() {
        println!("no anchors match.");
    } else {
        for v in &shown {
            print!("{}", render::anchor(v));
        }
    }
    Ok(0)
}

/// An anchor with no status is excluded by `--status` and kept by
/// `--not-status`: the second names statuses to drop, not "having one".
fn keep(view: &AnchorView, wanted: &[StatusId], unwanted: &[StatusId]) -> bool {
    if !wanted.is_empty() {
        return view.status.as_ref().is_some_and(|s| wanted.contains(s));
    }
    match &view.status {
        Some(s) => !unwanted.contains(s),
        None => true,
    }
}
