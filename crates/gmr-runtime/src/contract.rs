pub use gmr_core::{Openness, Ref, Source, Verifiability, Version};

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    Anchored, Before, Blind, Evidence, Grounding, Holding, Instructions, Knowledge, Standing,
    Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v2";

pub const SHAPE: &str = "sha256:4259eb32c1bafac89465799bccab90c2f89afa0d1e69775c7e938e140ef18bcf";
