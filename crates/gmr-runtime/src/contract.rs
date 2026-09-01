pub use gmr_core::{
    Binding, Claim, Derivation, Expr, FactAddress, LinkKind, Observes, Openness, Ref, SaidId,
    Source, Verifiability, Version,
};

pub use gmr_content::ContentErrorCode;

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::link::Reached;
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    AnchorView, Anchored, Asked, Before, Blind, Depends, Evidence, Footing, Grounded, Grounding,
    Holding, Instructions, Knowledge, Linked, MemoryView, Reading, Shown, Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v10";

pub const SHAPE: &str = "sha256:49ec4b324f7e06f28d53a40b7ba4941424eb79813cb019f5a17c9c6ff382ad2a";
