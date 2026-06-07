//! Synthesizer — converts a CallManifest into a PolicySpec.

pub mod composer;
pub mod decision_tree;
pub mod generator;
pub mod policy_spec;

pub use decision_tree::synthesize;
pub use policy_spec::{Constraints, PolicySpec};
