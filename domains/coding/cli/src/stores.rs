use std::path::Path;
use std::sync::Arc;

use crate::error::CliError;
use gmr::{ContentError, MemoryStore};
use gmr_provider::mem0::{Mem0, Scope};

pub struct Warning {
    pub provider: String,
    pub message: String,
}

pub struct Where<'a> {
    pub root: &'a Path,
    pub notes: &'a Arc<crate::notes::Notes>,
}

type Made = Option<Result<MemoryStore, ContentError>>;

type Registration = (&'static str, fn(&Where) -> Made);

const REGISTERED: &[Registration] = &[
    ("git", |at| {
        Some(Ok(
            gmr_provider::git::store(at.root).listing(at.notes.clone())
        ))
    }),
    ("claude-code", |at| {
        Some(gmr_provider::claude_code::store(at.root))
    }),
    ("mem0", |_| Some(configured()?.map(Mem0::store))),
];

#[derive(Default)]
pub struct Stores {
    pub built: Vec<MemoryStore>,
    pub warnings: Vec<Warning>,
    pub names: crate::memories::Names,
}

impl Stores {
    pub fn locate(&self, text: &str, provider: Option<&str>) -> Result<gmr::Ref, CliError> {
        let known: Vec<&str> = self.built.iter().map(|s| s.provider().as_str()).collect();
        crate::memories::located(text, provider, &known)
    }

    pub fn listing(&self, provider: Option<&str>) -> Vec<&MemoryStore> {
        self.named(provider)
            .filter(|s| s.source().is_some())
            .collect()
    }

    pub fn silent(&self, provider: Option<&str>) -> Vec<&MemoryStore> {
        self.named(provider)
            .filter(|s| s.source().is_none())
            .collect()
    }

    fn named(&self, provider: Option<&str>) -> impl Iterator<Item = &MemoryStore> {
        self.built
            .iter()
            .filter(move |s| provider.is_none_or(|want| s.provider().as_str() == want))
    }

    pub fn registered(&self) -> Vec<String> {
        self.built
            .iter()
            .map(|s| s.provider().to_string())
            .collect()
    }

    fn take(&mut self, provider: &str, made: Made) {
        match made {
            None => {}
            Some(Ok(store)) => self.built.push(store),
            Some(Err(e)) => self.warnings.push(Warning {
                provider: provider.to_owned(),
                message: e.to_string(),
            }),
        }
    }
}

pub fn assembled(root: &Path) -> Result<Stores, CliError> {
    let mut stores = Stores::default();

    let notes = Arc::new(crate::memories::declaring(root));
    stores.names = crate::memories::Names::over(vec![notes.clone()]);
    let at = Where {
        root,
        notes: &notes,
    };

    for (name, build) in REGISTERED {
        let made = build(&at);
        stores.take(name, made);
    }
    for (name, decl) in crate::providers::declared(root)? {
        let made = crate::providers::assembled(root, &name, &decl)
            .map_err(|CliError(message)| ContentError::new(message));
        stores.take(&name, Some(made));
    }
    Ok(stores)
}

fn configured() -> Option<Result<Mem0, ContentError>> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let scope = Scope {
        user_id: env("MEM0_USER_ID"),
        agent_id: env("MEM0_AGENT_ID"),
        app_id: env("MEM0_APP_ID"),
    };
    match (env("MEM0_BASE_URL"), env("MEM0_API_KEY")) {
        (Some(base), key) => Some(Mem0::self_hosted(base, key, scope)),
        (None, Some(key)) => Some(Mem0::platform(key, scope)),
        (None, None) => None,
    }
}
