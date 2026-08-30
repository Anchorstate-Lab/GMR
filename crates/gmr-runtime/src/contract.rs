pub use gmr_core::{Claim, FactAddress, Openness, Ref, SaidId, Source, Verifiability, Version};

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    Anchored, Before, Blind, Evidence, Grounding, Holding, Instructions, Knowledge, Reading, Shown,
    Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v3";

pub const SHAPE: &str = "sha256:215859c39a7bb1fbe117af2f25cd97a210efe9d6435c4db4d39b10a4fb429755";
