use std::path::Path;

use crate::error::CliError;
use gmr::{ContentError, MemoryStore};
use gmr_provider::mem0::{Mem0, Scope};

pub struct Warning {
    pub provider: String,
    pub message: String,
}

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
        self.built
            .iter()
            .filter(|s| s.source().is_some())
            .filter(|s| provider.is_none_or(|want| s.provider().as_str() == want))
            .collect()
    }

    fn take(&mut self, provider: &str, made: Result<MemoryStore, ContentError>) {
        match made {
            Ok(store) => self.built.push(store),
            Err(e) => self.warnings.push(Warning {
                provider: provider.to_owned(),
                message: e.to_string(),
            }),
        }
    }
}

pub fn assembled(root: &Path) -> Stores {
    let mut stores = Stores::default();

    let notes = std::sync::Arc::new(crate::memories::declaring(root));
    stores.names = crate::memories::Names::over(vec![notes.clone()]);
    stores
        .built
        .push(gmr_provider::git::store(root).listing(notes));

    stores.take("claude-code", gmr_provider::claude_code::store(root));
    if let Some(made) = from_env() {
        stores.take("mem0", made.map(Mem0::store));
    }
    stores
}

fn from_env() -> Option<Result<Mem0, ContentError>> {
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
