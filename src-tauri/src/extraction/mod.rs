//! Extraction — source detection, fetching, normalization, Anthropic call, response validation.
//!
//! L0 scaffold — submodules are stubs until L4.

pub mod detect;
pub mod fetchers;
pub mod normalize;
pub mod anthropic;
pub mod prompts;
pub mod response;
pub mod cache;
pub mod rate_limit;
