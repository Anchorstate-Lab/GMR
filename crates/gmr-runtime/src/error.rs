use gmr_core::{AnchorKey, Ref};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("anchor `{key}` has never been opened")]
    NoSuchAnchor { key: AnchorKey },

    #[error("`{reference:?}` is not bound to anything — nothing to reaffirm; bind it first")]
    NotBound { reference: Ref },

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

    #[error("this deployment has no Queue — pass is a polling-only verb")]
    NoQueue,

    #[error(
        "the lease on `{key}` is held by someone else — let the holder write, do not slip in beside it"
    )]
    Leased { key: AnchorKey },

    #[error(transparent)]
    Store(#[from] gmr_store::StoreError),
}

impl RuntimeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoSuchAnchor { .. } => "no_such_anchor",
            Self::NotBound { .. } => "not_bound",
            Self::AlreadyOpen { .. } => "already_open",
            Self::AnchorClosed { .. } => "anchor_closed",
            Self::NotClosedYet { .. } => "not_closed_yet",
            Self::CannotOpen { .. } => "cannot_open",
            Self::NoQueue => "no_queue",
            Self::Leased { .. } => "leased",
            Self::Store(e) => e.code(),
        }
    }
}
