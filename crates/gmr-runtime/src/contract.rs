pub use gmr_core::{
    Binding, Claim, Derivation, Expr, FactAddress, LinkKind, Observes, Openness, Ref, SaidId,
    Source, Verifiability, Version,
};

pub use gmr_content::ContentErrorCode;

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::link::{Inbound, Links, Reached};
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    AnchorView, Anchored, Asked, Before, Blind, Depends, Evidence, Footing, Grounded, Grounding,
    Holding, Instructions, Knowledge, Linked, MemoryView, Reading, SaidView, Shown, Standing,
    Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v11";

pub const SHAPE: &str = "sha256:c71b1e3cbde07e3cb890f97f9adf5e7d9332571036eed973dd310cedf346c039";
