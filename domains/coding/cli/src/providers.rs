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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ids {
    Readable,
    Opaque,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Versioning {
    #[default]
    ContentHash,
    Native,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Decl {
    pub fetch: String,
    #[serde(default)]
    pub list: Option<String>,
    pub ids: Ids,
    #[serde(default)]
    pub version: Versioning,
}

impl Decl {
    pub fn can(&self) -> Vec<&'static str> {
        vec![
            match self.ids {
                Ids::Readable => "ids you can write down",
                Ids::Opaque => "ids only the store issues",
            },
            match self.list.is_some() {
                true => "lists what it holds",
                false => "cannot be listed",
            },
            "versions by content hash, never the store's own",
        ]
    }

    pub fn caveat(&self) -> Option<&'static str> {
        match (self.ids, self.list.is_some()) {
            (Ids::Opaque, false) => Some(
                "nothing here can enumerate this store and its ids are not ones you could \
                 write down, so only memories written after it was wired up can be anchored \
                 — bind each one from the id the store hands back",
            ),
            _ => None,
        }
    }
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
    let declared = toml::from_str::<File>(&text)
        .map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))?
        .provider;
    for (name, decl) in &declared {
        gmr::ProviderId::try_new(name.as_str()).map_err(|e| {
            CliError(format!(
                "provider `{name}` cannot be named that: its name {e}. A store's name is \
                 the half of every address that says which store to ask, so a name the \
                 address form cannot carry is a store nothing could ever be bound to"
            ))
        })?;
        if decl.version == Versioning::Native {
            return Err(CliError(format!(
                "provider `{name}` declares `version = \"native\"`, and a store declared in \
                 a recipe cannot have one: a script answers with the memory and there is no \
                 channel in the call for the store's own revision. Every version here is a \
                 hash GMR computes over the text, which is why a store whose own version you \
                 need has to be compiled into this binary"
            )));
        }
    }
    Ok(declared)
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
