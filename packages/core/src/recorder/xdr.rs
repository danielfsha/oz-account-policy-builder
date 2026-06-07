//! XDR decode logic for Soroban TransactionMeta.
//!
//! Decodes base64-encoded TransactionMetaV3 XDR to extract
//! diagnostic events, auth entries, and state changes.

#![allow(missing_docs)]

use crate::recorder::manifest::{AuthBoundary, CallNode};
use serde::{Deserialize, Serialize};

/// Error types for XDR decoding.
#[derive(Debug, thiserror::Error)]
pub enum XdrDecodeError {
    #[error("Base64 decode failed: {0}")]
    Base64(String),
    #[error("XDR parse failed: {0}")]
    XdrParse(String),
    #[error("Not a V3 TransactionMeta")]
    NotV3,
    #[error("No Soroban meta in transaction")]
    NoSorobanMeta,
    #[error("Transaction failed on-chain")]
    TransactionFailed,
}

/// Raw decoded data extracted from TransactionMetaV3.
/// This is passed from TypeScript (which does the actual XDR decode
/// using stellar-sdk) via the WASM bridge in JSON form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedTransactionMeta {
    /// Whether the transaction succeeded
    pub success: bool,
    /// Diagnostic events extracted from sorobanMeta
    pub diagnostic_events: Vec<DiagnosticEvent>,
    /// Ledger sequence
    pub ledger_sequence: u32,
    /// Unix timestamp from ledger
    pub close_time: u64,
    /// CPU instructions (from resource usage)
    pub cpu_instructions: Option<u64>,
    /// Memory bytes (from resource usage)
    pub memory_bytes: Option<u64>,
}

/// A single diagnostic event from TransactionMetaV3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    /// Whether this event was in a successful sub-call
    pub in_successful_call: bool,
    /// Contract that emitted the event
    pub contract_id: Option<String>,
    /// Event type: "contract" | "diagnostic" | "system"
    pub event_type: String,
    /// Topics as JSON-serialized ScVal representations
    pub topics: Vec<serde_json::Value>,
    /// Event data as JSON-serialized ScVal
    pub data: serde_json::Value,
}

/// Auth entry from the transaction auth vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAuthEntry {
    /// The account that signed this auth entry
    pub account: String,
    /// Root invocation — contract + function + args
    pub root_invocation: Option<RawInvocation>,
}

/// A raw contract invocation from auth entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInvocation {
    pub contract_id: String,
    pub function_name: String,
    pub args: Vec<serde_json::Value>,
    pub sub_invocations: Vec<RawInvocation>,
}

impl RawInvocation {
    /// Convert a RawInvocation tree into a CallNode tree.
    pub fn into_call_node(self, authorized_by: Option<String>) -> CallNode {
        CallNode {
            contract_id: self.contract_id,
            contract_label: None, // Resolved later by protocol registry
            function_name: self.function_name,
            args: self.args,
            sub_calls: self
                .sub_invocations
                .into_iter()
                .map(|i| i.into_call_node(None))
                .collect(),
            requires_auth: authorized_by.is_some(),
            authorized_by,
        }
    }
}

/// Extract auth boundaries from raw auth entries.
pub fn extract_auth_boundaries(auth_entries: &[RawAuthEntry]) -> Vec<AuthBoundary> {
    let mut boundaries = Vec::new();
    for entry in auth_entries {
        if let Some(ref inv) = entry.root_invocation {
            boundaries.push(AuthBoundary {
                contract_id: inv.contract_id.clone(),
                function_name: inv.function_name.clone(),
                account: entry.account.clone(),
            });
            // Recurse into sub-invocations
            collect_auth_boundaries_recursive(&inv.sub_invocations, &entry.account, &mut boundaries);
        }
    }
    boundaries
}

fn collect_auth_boundaries_recursive(
    invocations: &[RawInvocation],
    account: &str,
    out: &mut Vec<AuthBoundary>,
) {
    for inv in invocations {
        out.push(AuthBoundary {
            contract_id: inv.contract_id.clone(),
            function_name: inv.function_name.clone(),
            account: account.to_string(),
        });
        collect_auth_boundaries_recursive(&inv.sub_invocations, account, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_invocation_into_call_node() {
        let raw = RawInvocation {
            contract_id: "CABC123".to_string(),
            function_name: "swap".to_string(),
            args: vec![serde_json::json!(100)],
            sub_invocations: vec![RawInvocation {
                contract_id: "CDEF456".to_string(),
                function_name: "transfer".to_string(),
                args: vec![],
                sub_invocations: vec![],
            }],
        };

        let node = raw.into_call_node(Some("GABC".to_string()));
        assert_eq!(node.contract_id, "CABC123");
        assert_eq!(node.function_name, "swap");
        assert_eq!(node.sub_calls.len(), 1);
        assert_eq!(node.sub_calls[0].function_name, "transfer");
        assert!(node.requires_auth);
    }

    #[test]
    fn test_extract_auth_boundaries() {
        let entries = vec![RawAuthEntry {
            account: "GABC".to_string(),
            root_invocation: Some(RawInvocation {
                contract_id: "CABC".to_string(),
                function_name: "claim".to_string(),
                args: vec![],
                sub_invocations: vec![],
            }),
        }];
        let boundaries = extract_auth_boundaries(&entries);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].account, "GABC");
        assert_eq!(boundaries[0].function_name, "claim");
    }
}
