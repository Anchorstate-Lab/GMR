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

pub const CONTRACT: &str = "gmr.contract.v12.0";

pub const SHAPE: &str = "sha256:dcfb44b05c5113db957eee6bdd7d830cf30608ab8f66a9142afc6e42823d8d44";
