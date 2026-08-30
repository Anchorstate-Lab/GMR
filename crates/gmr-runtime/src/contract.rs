pub use gmr_core::{
    Binding, Claim, Derivation, Expr, FactAddress, Observes, Openness, Ref, SaidId, Source,
    Verifiability, Version,
};

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::link::Reached;
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    Anchored, Asked, Before, Blind, Depends, Evidence, Footing, Grounding, Holding, Instructions,
    Knowledge, Reading, Shown, Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v8";

pub const SHAPE: &str = "sha256:2f62bb64f5143c8e8fe207245fd79768818ff084f044c8c8fd4cc46e11f61951";
