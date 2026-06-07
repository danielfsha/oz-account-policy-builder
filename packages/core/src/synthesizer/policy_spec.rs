//! PolicySpec — structured description of a synthesized policy.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete output of the synthesizer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    /// Context rule — contracts and functions to whitelist
    pub context_rule: ContextRule,
    /// Ordered policy layers (max 5 per OZ limit)
    pub policies: Vec<PolicyLayer>,
    /// Whether to compose existing OZ primitives or generate net-new code
    pub composition_mode: CompositionMode,
    /// Rationale for the chosen primitives
    pub rationale: String,
    /// Questions to ask user before proceeding
    pub clarifications_needed: Vec<Clarification>,
    /// Human-readable policy name (used for crate name)
    pub policy_name: String,
}

/// The context rule — defines what this policy applies to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRule {
    /// Whitelisted contract IDs
    pub contracts: Vec<String>,
    /// Whitelisted function names
    pub functions: Vec<String>,
    /// Lifetime in seconds (default: 365 days)
    pub lifetime_seconds: u64,
}

impl Default for ContextRule {
    fn default() -> Self {
        ContextRule {
            contracts: vec![],
            functions: vec![],
            lifetime_seconds: 365 * 24 * 3600,
        }
    }
}

/// A single policy layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyLayer {
    /// Policy type
    pub kind: PolicyLayerKind,
    /// Parameters for this layer
    pub params: HashMap<String, serde_json::Value>,
    /// Whether this uses an existing OZ primitive (true) or needs net-new codegen (false)
    pub oz_primitive: bool,
    /// Human-readable description
    pub description: String,
}

/// The type of policy layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyLayerKind {
    /// Cap on spending per time window
    SpendingLimit,
    /// Allow only within a time window
    TimeWindow,
    /// Single signer sufficient
    SimpleThreshold,
    /// Weighted multi-signer
    WeightedThreshold,
    /// Net-new custom policy
    Custom,
}

/// Whether to compose primitives or generate new code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompositionMode {
    /// Use existing OZ primitives
    Compose,
    /// Generate net-new Policy trait implementation
    Generate,
}

/// A clarification question to ask the user before proceeding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clarification {
    /// Field this clarification addresses
    pub field: String,
    /// Question to ask
    pub question: String,
    /// Optional suggested answers
    pub options: Option<Vec<String>>,
}

/// User-supplied constraint overrides for synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Constraints {
    /// Override observed amount cap (raw u128)
    pub amount_cap: Option<u128>,
    /// Time window in seconds
    pub time_window_seconds: Option<u64>,
    /// Max calls per time window
    pub max_calls_per_window: Option<u32>,
    /// Slippage percentage (0-100)
    pub allow_slippage_percent: Option<f64>,
    /// Context rule lifetime in seconds
    pub lifetime_seconds: Option<u64>,
}
