//! `matching` is one opinionated fuzzy-coordinate algorithm; `walk` is the file
//! tree traversal every extractor repeated. Neither knows a language.

pub mod cache;
pub mod index;
pub mod matching;
pub mod narrow;
pub mod recipe;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "testkit")]
pub mod testkit;
pub mod walk;

pub use cache::*;
pub use index::*;
pub use matching::*;
pub use narrow::*;
pub use recipe::*;
pub use walk::*;
