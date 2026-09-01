pub use gmr_core::{
    Binding, Claim, Derivation, Expr, FactAddress, Observes, Openness, Ref, SaidId, Source,
    Verifiability, Version,
};

pub use gmr_content::ContentErrorCode;

pub use crate::bind::Landed;
pub use crate::edges::{Edge, Edges, Raised};
pub use crate::link::Reached;
pub use crate::open::{OpenRequest, Opened, Supersede};
pub use crate::read::{
    AnchorView, Anchored, Asked, Before, Blind, Depends, Evidence, Footing, Grounded, Grounding,
    Holding, Instructions, Knowledge, MemoryView, Reading, Shown, Standing, Warrant,
};

pub const CONTRACT: &str = "gmr.contract.v9";

pub const SHAPE: &str = "sha256:ed1983d4fd45957fe964259dd4a09db403d6cfe677116cdfe4991c1c3abef38b";
