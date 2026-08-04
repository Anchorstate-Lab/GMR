//! The subprocess half of the probe contract: env vars in, exit code out.
//! Everything else lives in `gmr-survey`, which knows nothing about processes.

pub mod env;

pub use env::*;
pub use gmr_survey::*;
