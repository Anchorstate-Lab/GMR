use gmr::sqlite::SqliteStore;

use crate::error::CliError;

pub async fn run(store: &SqliteStore, file: String, json: bool) -> Result<i32, CliError> {
    let f = std::fs::File::open(&file).map_err(|e| CliError(format!("cannot open {file}: {e}")))?;
    let summary = store.import_jsonl(std::io::BufReader::new(f)).await?;

    if json {
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!(
            "journal {}  bindings {}  binding_anchors {}  links {}  sealed {}",
            summary.journal,
            summary.bindings,
            summary.binding_anchors,
            summary.links,
            summary.sealed,
        );
    }
    Ok(0)
}
