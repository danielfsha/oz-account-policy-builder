//! CallManifest — the structured representation of a recorded Soroban transaction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single contract invocation node in the call tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallNode {
    /// Contract ID (C-address format)
    pub contract_id: String,
    /// Human-readable label from protocol registry (if known)
    pub contract_label: Option<String>,
    /// Function name invoked
    pub function_name: String,
    /// Decoded argument list (JSON-serializable)
    pub args: Vec<serde_json::Value>,
    /// Sub-calls made by this invocation
    pub sub_calls: Vec<CallNode>,
    /// Whether this call triggered a require_auth()
    pub requires_auth: bool,
    /// The account that authorized this call (if known)
    pub authorized_by: Option<String>,
}

/// A contract referenced in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContractRef {
    /// Contract ID (C-address format)
    pub id: String,
    /// Human-readable label (from protocol registry)
    pub label: Option<String>,
    /// Protocol family (blend, soroswap, aquarius, etc.)
    pub protocol: Option<String>,
}

/// Net token movement involving the invoking account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetFlow {
    /// Asset identifier (contract ID for SAC tokens, or "native" for XLM)
    pub asset_id: String,
    /// Human-readable asset symbol (USDC, XLM, etc.)
    pub asset_symbol: Option<String>,
    /// Direction relative to invoking account
    pub direction: FlowDirection,
    /// Raw integer amount (7 decimal places for XLM, SEP-41 decimals otherwise)
    pub amount_raw: u128,
    /// Human-readable amount string with correct decimals
    pub amount_display: String,
    /// Decimal places for this asset
    pub decimals: u8,
    /// Counterparty address
    pub counterparty: Option<String>,
}

/// Direction of asset flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FlowDirection {
    /// Token leaving invoking account
    Outbound,
    /// Token entering invoking account
    Inbound,
}

/// An observed require_auth() call site in the auth tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthBoundary {
    /// Contract that called require_auth()
    pub contract_id: String,
    /// Function that called require_auth()
    pub function_name: String,
    /// Account whose auth was required
    pub account: String,
}

/// Observed amount range for a (contract, function) pair.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AmountRange {
    /// Minimum observed amount (raw)
    pub min_raw: u128,
    /// Maximum observed amount (raw)
    pub max_raw: u128,
    /// Asset ID this range refers to
    pub asset_id: String,
}

/// The complete structured output of recording a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallManifest {
    /// Transaction hash (or "simulated_{id}" for simulations)
    pub transaction_hash: String,
    /// Network this transaction was observed on
    pub network: String,
    /// Ledger sequence number (0 for simulations)
    pub ledger_sequence: u32,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Invoking account (G-address or C-address)
    pub invoking_account: String,
    /// Pruned call tree — user-initiated calls only
    pub top_level_calls: Vec<CallNode>,
    /// Unique contracts referenced in the manifest
    pub unique_contracts: Vec<ContractRef>,
    /// Unique function names observed
    pub unique_functions: Vec<String>,
    /// Net asset flows involving the invoking account
    pub asset_flows: Vec<AssetFlow>,
    /// require_auth() call sites
    pub auth_boundaries: Vec<AuthBoundary>,
    /// Observed amount ranges per (contract_id, function_name)
    pub observed_amounts: HashMap<String, AmountRange>,
    /// Human-readable one-liner summary
    pub summary: String,
    /// Whether this was a simulation (not an executed transaction)
    pub is_simulation: bool,
    /// Simulation resource cost (only set for simulations)
    pub simulation_cost: Option<SimulationCost>,
}

/// Resource consumption from a simulated transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationCost {
    /// CPU instructions consumed
    pub cpu_instructions: u64,
    /// Memory bytes consumed
    pub memory_bytes: u64,
}
