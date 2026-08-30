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

pub const CONTRACT: &str = "gmr.contract.v7";

pub const SHAPE: &str = "sha256:b068c63d8945791f59c9482ac872705fba92a44592abf696ec77fa44fda3caf4";
