pub use gmr_core::{
    Binding, Claim, Derivation, Expr, FactAddress, Observes, Openness, Ref, SaidId, Source,
    Verifiability, Version,
};

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::link::Reached;
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    Anchored, Before, Blind, Depends, Evidence, Footing, Grounding, Holding, Instructions,
    Knowledge, Reading, Shown, Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v6";

pub const SHAPE: &str = "sha256:93e40401cfd453b976c912407c0b79df45784634a477f0e514883e65344d065c";
