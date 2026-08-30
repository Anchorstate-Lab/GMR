pub use gmr_core::{
    Binding, Claim, Expr, FactAddress, Openness, Ref, SaidId, Source, Verifiability, Version,
};

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    Anchored, Before, Blind, Depends, Evidence, Grounding, Holding, Instructions, Knowledge,
    Reading, Shown, Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v4";

pub const SHAPE: &str = "sha256:72e3fe85c897a6b7fa8adc0ade250cfbe37e615f2b113df3cc52248c72c936f9";
