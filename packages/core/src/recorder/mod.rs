//! Recorder — transforms raw XDR into a structured CallManifest.

pub mod asset_extractor;
pub mod invocation_tree;
pub mod manifest;
pub mod xdr;

pub use manifest::CallManifest;
