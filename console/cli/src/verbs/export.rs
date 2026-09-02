use gmr::sqlite::SqliteStore;

use crate::error::CliError;

pub async fn run(store: &SqliteStore, out: Option<String>, json: bool) -> Result<i32, CliError> {
    let summary = match &out {
        Some(path) => {
            let mut file = std::fs::File::create(path)
                .map_err(|e| CliError(format!("cannot create {path}: {e}")))?;
            store.export_jsonl(&mut file).await?
        }
        None => store.export_jsonl(&mut std::io::stdout().lock()).await?,
    };

    if json {
        eprintln!("{}", serde_json::to_string(&summary)?);
    } else {
        eprintln!(
            "journal {}  bindings {}  binding_anchors {}  links {}  sealed {}",
            summary.journal,
            summary.bindings,
            summary.binding_anchors,
            summary.links,
            summary.sealed,
        );
        eprintln!(
            "settings and the queue were not exported — they say how an anchor runs, \
             not what it judged; `sync` rebuilds them from declarations."
        );
    }
    Ok(0)
}
