//! `matching` is one opinionated fuzzy-coordinate algorithm; `walk` is the file
//! tree traversal every extractor repeated. Neither knows a language.

pub mod cache;
pub mod matching;
pub mod narrow;
pub mod walk;

pub use cache::*;
pub use matching::*;
pub use narrow::*;
pub use walk::*;
