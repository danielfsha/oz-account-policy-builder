//! OZ Policy Builder — Core Library
//!
//! Public API for the policy synthesizer. Exposed to TypeScript via WASM bindings.
//!
//! Pipeline:
//!   XDR input → recorder → CallManifest
//!   CallManifest → synthesizer → PolicySpec
//!   PolicySpec + CallManifest → harness → HarnessReport
//!   PolicySpec → codegen → generated Rust crate

#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub mod codegen;
pub mod harness;
pub mod recorder;
pub mod synthesizer;

// Re-export primary types at crate root for convenience
pub use recorder::manifest::{
    AmountRange, AssetFlow, AuthBoundary, CallManifest, CallNode, ContractRef,
};
pub use synthesizer::policy_spec::{Clarification, PolicyLayer, PolicyLayerKind, PolicySpec};
pub use harness::HarnessReport;
