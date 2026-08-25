use std::collections::BTreeMap;
use std::path::Path;

use gmr_probe::{Budget, Spent};

use crate::matching::{Fragment, Want};
use crate::recipe::Recipe;
use crate::walk::{Held, Stamp, hash, sort_key, visit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    Spent(Spent),
    Faulted(String),
    Refused(String),
}

impl Halt {
    pub fn deterministic(&self) -> bool {
        matches!(self, Self::Refused(_))
    }
}

impl From<String> for Halt {
    fn from(why: String) -> Self {
        Self::Refused(why)
    }
}

impl std::fmt::Display for Halt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spent(spent) => f.write_str(spent.as_str()),
            Self::Faulted(why) | Self::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Halt {}

pub trait Corpus {
    fn refresh(&self, recipe: &Recipe, budget: &Budget) -> Result<(), Halt>;

    fn populated(&self, recipe: &Recipe, root: &str) -> Result<bool, Halt>;

    fn whole(&self, recipe: &Recipe, root: &str) -> Result<Vec<Fragment>, Halt>;

    fn touching(&self, recipe: &Recipe, root: &str, want: &Want) -> Result<Vec<Fragment>, Halt>;
}

pub struct Fresh {
    pub rel: String,
    pub hash: String,
    pub sort: String,
    pub stamp: Option<Stamp>,
    pub fragments: Vec<Fragment>,
}

#[derive(Default)]
pub struct Rescan {
    pub fresh: Vec<Fresh>,
    pub restamped: Vec<(String, Option<Stamp>)>,
    pub gone: Vec<String>,
}

pub fn rescan(
    tree: &Path,
    recipe: &Recipe,
    known: &BTreeMap<String, Held>,
    budget: &Budget,
) -> Result<Rescan, Halt> {
    let mut out = Rescan::default();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut halted = None;
    let mut refused = None;

    visit(tree, &mut |at, rel| {
        if let Err(spent) = budget.checkpoint() {
            halted = Some(spent);
            return Err(String::new());
        }
        if !(recipe.eligible)(rel) {
            return Ok(());
        }
        let rel = rel.replace('\\', "/");
        let stamp = Stamp::of(at);
        if let Some(had) = known.get(&rel)
            && had.stamp.is_some()
            && had.stamp == stamp
        {
            seen.insert(rel);
            return Ok(());
        }
        let Ok(bytes) = std::fs::read(at) else {
            return Ok(());
        };
        let digest = hash(&String::from_utf8_lossy(&bytes));
        if known.get(&rel).is_some_and(|had| had.hash == digest) {
            out.restamped.push((rel.clone(), stamp));
            seen.insert(rel);
            return Ok(());
        }
        let mut fragments = Vec::new();
        if let Err(why) = (recipe.collect)(&rel, &bytes, &mut fragments) {
            refused = Some(why);
            return Err(String::new());
        }
        out.fresh.push(Fresh {
            sort: sort_key(&rel),
            hash: digest,
            stamp,
            fragments,
            rel: rel.clone(),
        });
        seen.insert(rel);
        Ok(())
    })
    .map_err(|why| match (halted, refused.take()) {
        (Some(spent), _) => Halt::Spent(spent),
        (None, Some(refused)) => Halt::Refused(refused),
        (None, None) => Halt::Refused(why),
    })?;

    out.gone = known
        .keys()
        .filter(|rel| !seen.contains(rel.as_str()))
        .cloned()
        .collect();
    Ok(out)
}
