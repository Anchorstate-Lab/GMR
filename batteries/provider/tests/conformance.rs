#![cfg(any(
    feature = "git",
    feature = "claude-code",
    feature = "testkit",
    feature = "declared"
))]

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use gmr_content::ContentProvider;
use gmr_content::testkit::{Corpus, Listing, conforms, lists, retains};
use gmr_core::ExternalId;

struct Files<P> {
    dir: tempfile::TempDir,
    provider: P,
    written: AtomicUsize,
    build: fn(&Path) -> P,
}

impl<P: ContentProvider + 'static> Files<P> {
    fn new(prepare: fn(&Path), build: fn(&Path) -> P) -> Self {
        let dir = tempfile::tempdir().unwrap();
        prepare(dir.path());
        let provider = build(dir.path());
        Self {
            dir,
            provider,
            written: AtomicUsize::new(0),
            build,
        }
    }
}

#[async_trait]
impl<P: ContentProvider + 'static> Corpus for Files<P> {
    fn provider(&self) -> &dyn ContentProvider {
        &self.provider
    }

    async fn holding(&self, bytes: &[u8]) -> ExternalId {
        let rel = format!("{}.md", self.written.fetch_add(1, Ordering::SeqCst));
        std::fs::write(self.dir.path().join(&rel), bytes).unwrap();
        ExternalId::new(rel)
    }

    async fn never_held(&self) -> ExternalId {
        ExternalId::new("nobody-wrote-this.md")
    }

    async fn out_of_reach(&self) -> Box<dyn ContentProvider> {
        Box::new((self.build)(&self.dir.path().join("never-created")))
    }
}

#[cfg(feature = "git")]
#[tokio::test]
async fn git_conforms() {
    let corpus = Files::new(
        |root| {
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(root)
                .status()
                .ok();
        },
        |root| gmr_provider::git::Git::new(root),
    );
    conforms(&corpus).await.unwrap();
}

#[cfg(feature = "git")]
#[tokio::test]
async fn git_retains() {
    let corpus = Files::new(
        |root| {
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(root)
                .status()
                .ok();
        },
        |root| gmr_provider::git::Git::new(root),
    );
    retains(&corpus).await.unwrap();
}

#[cfg(feature = "claude-code")]
#[tokio::test]
async fn claude_code_conforms() {
    let corpus = Files::new(
        |_| {},
        |root| gmr_provider::claude_code::ClaudeMemory::at(root),
    );
    conforms(&corpus).await.unwrap();
}

#[cfg(feature = "claude-code")]
impl Listing for Files<gmr_provider::claude_code::ClaudeMemory> {
    fn source(&self) -> &dyn gmr_content::MemorySource {
        &self.provider
    }
}

#[cfg(feature = "claude-code")]
#[tokio::test]
async fn claude_code_lists() {
    let corpus = Files::new(
        |_| {},
        |root| gmr_provider::claude_code::ClaudeMemory::at(root),
    );
    lists(&corpus).await.unwrap();
}

#[cfg(feature = "testkit")]
struct Remote {
    store: gmr_provider::mem0::testkit::Memories,
    provider: gmr_provider::mem0::Mem0,
    written: AtomicUsize,
}

#[cfg(feature = "testkit")]
impl Remote {
    fn new() -> Self {
        let store = gmr_provider::mem0::testkit::Memories::new();
        let provider = store.provider();
        Self {
            store,
            provider,
            written: AtomicUsize::new(0),
        }
    }
}

#[cfg(feature = "testkit")]
#[async_trait]
impl Corpus for Remote {
    fn provider(&self) -> &dyn ContentProvider {
        &self.provider
    }

    async fn holding(&self, bytes: &[u8]) -> ExternalId {
        let id = format!("m-{}", self.written.fetch_add(1, Ordering::SeqCst));
        self.store.holds(&id, &String::from_utf8_lossy(bytes));
        ExternalId::new(id)
    }

    async fn never_held(&self) -> ExternalId {
        ExternalId::new("m-nobody-stored-this")
    }

    async fn out_of_reach(&self) -> Box<dyn ContentProvider> {
        Box::new(self.store.out_of_reach())
    }
}

#[cfg(feature = "testkit")]
impl Listing for Remote {
    fn source(&self) -> &dyn gmr_content::MemorySource {
        &self.provider
    }
}

#[cfg(feature = "testkit")]
#[tokio::test]
async fn mem0_conforms() {
    conforms(&Remote::new()).await.unwrap();
}

#[cfg(feature = "testkit")]
#[tokio::test]
async fn mem0_lists() {
    lists(&Remote::new()).await.unwrap();
}

#[cfg(feature = "testkit")]
#[tokio::test]
async fn mem0_retains() {
    retains(&Remote::new()).await.unwrap();
}

#[cfg(feature = "declared")]
mod declared {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use gmr_content::ContentProvider;
    use gmr_content::testkit::{Corpus, Listing, conforms, lists};
    use gmr_core::{Derivation, ExternalId, Facts, Kind, Outcome, ProbeName, ProbeRef};
    use gmr_probe::{ProbeCall, ProbeError, Transport};
    use gmr_provider::declared::Declared;
    use serde_json::{Value, json};

    #[derive(Default)]
    struct Held(Mutex<BTreeMap<String, String>>);

    struct Served {
        held: Arc<Held>,
        kind: Kind,
        reachable: bool,
    }

    impl Served {
        fn new(held: Arc<Held>, reachable: bool) -> Self {
            Self {
                held,
                kind: Kind::new("script"),
                reachable,
            }
        }
    }

    #[async_trait]
    impl Transport for Served {
        fn kind(&self) -> &Kind {
            &self.kind
        }

        fn resolve(&self, _name: &ProbeName) -> Option<Derivation> {
            None
        }

        async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
            if !self.reachable {
                return Err(ProbeError::unreachable("this store is not answering"));
            }
            let held = self.held.0.lock().unwrap();
            if call.probe.name.as_str() == "list" {
                let records: Vec<Value> = held
                    .iter()
                    .map(|(id, text)| json!({ "id": id, "text": text }))
                    .collect();
                return Ok(Outcome::Found {
                    facts: Facts::new(json!({ "records": records })),
                });
            }
            let id = call
                .position
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("");
            Ok(match held.get(id) {
                None => Outcome::NotFound,
                Some(text) => Outcome::Found {
                    facts: Facts::new(json!({ "text": text })),
                },
            })
        }
    }

    fn probe(name: &str) -> ProbeRef {
        ProbeRef::new(Kind::new("script"), ProbeName::new(name), Value::Null)
    }

    fn provider(held: Arc<Held>, reachable: bool) -> Declared {
        Declared::new(
            "declared",
            probe("fetch"),
            Arc::new(Served::new(held, reachable)),
        )
    }

    struct Recipe {
        held: Arc<Held>,
        content: Arc<dyn ContentProvider>,
        store: gmr_content::MemoryStore,
        written: std::sync::atomic::AtomicUsize,
    }

    impl Recipe {
        fn new() -> Self {
            let held = Arc::new(Held::default());
            let store = provider(Arc::clone(&held), true).listing(probe("list"));
            Self {
                content: store.content(),
                store,
                held,
                written: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Corpus for Recipe {
        fn provider(&self) -> &dyn ContentProvider {
            self.content.as_ref()
        }

        async fn holding(&self, bytes: &[u8]) -> ExternalId {
            let id = format!(
                "r-{}",
                self.written
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            );
            self.held
                .0
                .lock()
                .unwrap()
                .insert(id.clone(), String::from_utf8_lossy(bytes).into_owned());
            ExternalId::new(id)
        }

        async fn never_held(&self) -> ExternalId {
            ExternalId::new("r-nobody-declared-this")
        }

        async fn out_of_reach(&self) -> Box<dyn ContentProvider> {
            Box::new(provider(Arc::clone(&self.held), false))
        }
    }

    impl Listing for Recipe {
        fn source(&self) -> &dyn gmr_content::MemorySource {
            self.store
                .source()
                .expect("a recipe that declares a listing script offers one")
        }
    }

    #[tokio::test]
    async fn a_provider_read_out_of_a_recipe_conforms() {
        conforms(&Recipe::new()).await.unwrap();
    }

    #[tokio::test]
    async fn a_provider_read_out_of_a_recipe_lists() {
        lists(&Recipe::new()).await.unwrap();
    }
}
