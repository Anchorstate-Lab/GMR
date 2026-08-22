#![cfg(any(feature = "git", feature = "claude-code", feature = "testkit"))]

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
