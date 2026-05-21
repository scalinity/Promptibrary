//! Extraction — source detection, fetching, normalization, Anthropic call, response validation.
//!
//! L4 wires the full pipeline. `types` carries the discriminated unions
//! shared across submodules and IPC.

pub mod types;

pub mod detect;
pub mod fetchers;
pub mod normalize;
pub mod anthropic;
pub mod prompts;
pub mod response;
pub mod cache;
pub mod rate_limit;
pub mod ssrf;
