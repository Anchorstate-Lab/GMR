use gmr_core::{AnchorKey, Claim, ProviderId};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("anchor `{key}` has never been opened")]
    NoSuchAnchor { key: AnchorKey },

    #[error(
        "no provider named `{provider}` is registered in this binary — \
         this is an assembly fault, not the world saying the record is gone. \
         Which providers exist depends on how this binary was built"
    )]
    NoProvider { provider: ProviderId },

    #[error("`{claim}` is not bound to anything — nothing to reaffirm; bind it first")]
    NotBound { claim: Claim },

    #[error("anchor `{key}` is already open — change it with revise, which leaves a sealed record")]
    AlreadyOpen { key: AnchorKey },

    #[error(
        "anchor `{key}` has finished — finishing is irreversible. \
         A wrong criterion means opening a new generation (supersede `{key}`), \
         not dragging this one back"
    )]
    AnchorClosed { key: AnchorKey },

    #[error("`{key}` is still running — only a finished anchor can be superseded")]
    NotClosedYet { key: AnchorKey },

    #[error(
        "the probe would not run while opening: {message}. \
         An anchor may precede its target, but with no successful observation \
         there is no starting point to capture"
    )]
    CannotOpen { message: String },

    #[error(
        "anchor `{key}` records digests only, and the probe answered with values that are \
         not sha256 digests. Nothing was written -- refusing is the enforcement, because an \
         anchor whose facts are secret cannot be protected by asking its probe nicely"
    )]
    Undigested { key: AnchorKey },

    #[error("this deployment has no Queue — pass is a polling-only verb")]
    NoQueue,

    #[error(
        "the lease on `{key}` is held by someone else — let the holder write, do not slip in beside it"
    )]
    Leased { key: AnchorKey },

    #[error(transparent)]
    Store(#[from] gmr_store::StoreError),

    #[error(transparent)]
    Content(#[from] gmr_content::ContentError),

    #[error(transparent)]
    Canonicalize(#[from] gmr_core::CanonicalizeError),
}

impl RuntimeError {
    pub fn head_moved(&self) -> bool {
        matches!(self, Self::Store(e) if e.code == gmr_store::ErrorCode::HeadMoved)
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::NoSuchAnchor { .. } => "no_such_anchor",
            Self::NoProvider { .. } => "no_provider",
            Self::NotBound { .. } => "not_bound",
            Self::AlreadyOpen { .. } => "already_open",
            Self::AnchorClosed { .. } => "anchor_closed",
            Self::NotClosedYet { .. } => "not_closed_yet",
            Self::CannotOpen { .. } => "cannot_open",
            Self::Undigested { .. } => "undigested",
            Self::NoQueue => "no_queue",
            Self::Leased { .. } => "leased",
            Self::Store(e) => e.code(),
            Self::Content(e) => e.code(),
            Self::Canonicalize(_) => "canonicalize_failed",
        }
    }
}
