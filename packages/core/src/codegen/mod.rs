//! Codegen — fill templates and emit a compilable Rust policy crate.

pub mod emitter;
pub mod templates;

pub use emitter::{emit_policy_crate, EmittedCrate};
