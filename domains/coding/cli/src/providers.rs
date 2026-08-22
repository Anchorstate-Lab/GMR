use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gmr::{Kind, MemoryStore, ProbeName, ProbeRef};
use gmr_provider::declared::Declared;
use gmr_transport::script::Script;
use serde::Deserialize;

use crate::error::CliError;

pub const RECIPES_FILE: &str = "providers.toml";

pub const FETCH: &str = "fetch";

pub const LIST: &str = "list";

#[derive(Debug, Clone, Deserialize)]
pub struct Decl {
    pub fetch: String,
    #[serde(default)]
    pub list: Option<String>,
}

#[derive(Debug, Deserialize)]
struct File {
    #[serde(default)]
    provider: BTreeMap<String, Decl>,
}

pub fn declared(root: &Path) -> Result<BTreeMap<String, Decl>, CliError> {
    let path = crate::probes::anchor_dir(root).join(RECIPES_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(BTreeMap::new());
    };
    Ok(toml::from_str::<File>(&text)
        .map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))?
        .provider)
}

pub fn assembled(root: &Path, name: &str, decl: &Decl) -> Result<MemoryStore, CliError> {
    let mut scripts = BTreeMap::from([(
        ProbeName::new(FETCH),
        script(root, name, FETCH, &decl.fetch)?,
    )]);
    if let Some(listing) = &decl.list {
        scripts.insert(ProbeName::new(LIST), script(root, name, LIST, listing)?);
    }
    let transport = Arc::new(Script::new(root, scripts));
    let provider = Declared::new(name, probe(FETCH), transport);
    Ok(match decl.list.is_some() {
        true => provider.listing(probe(LIST)),
        false => provider.store(),
    })
}

fn probe(name: &str) -> ProbeRef {
    ProbeRef::new(
        Kind::new("script"),
        ProbeName::new(name),
        serde_json::Value::Null,
    )
}

fn script(root: &Path, name: &str, what: &str, declared: &str) -> Result<PathBuf, CliError> {
    let path = root.join(declared);
    if !path.is_file() {
        return Err(CliError(format!(
            "provider `{name}` declares `{what} = \"{declared}\"` and there is no such file. \
             A store whose script is missing answers every read with a failure, which reads \
             as somebody else's service being down"
        )));
    }
    Ok(PathBuf::from(declared))
}
