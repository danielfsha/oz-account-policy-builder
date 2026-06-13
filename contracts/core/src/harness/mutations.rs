//! The 5 standard deny mutations.
//!
//! Each mutation modifies the manifest in a specific way that the policy
//! MUST reject. All 5 must fail for the harness to pass.

use crate::recorder::manifest::{AssetFlow, CallManifest, CallNode, FlowDirection};

/// Mutation type identifier.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationType {
    /// Same tx, amount doubled
    AmountExceeded,
    /// Same tx, different asset
    WrongAsset,
    /// Same function, different contract address
    WrongContract,
    /// Original + extra unauthorized function call
    ExtraFunctionCall,
    /// Same tx, timestamp past end of time window
    OutOfWindow,
}

impl MutationType {
    pub fn name(&self) -> &'static str {
        match self {
            MutationType::AmountExceeded => "amount_exceeded",
            MutationType::WrongAsset => "wrong_asset",
            MutationType::WrongContract => "wrong_contract",
            MutationType::ExtraFunctionCall => "extra_function_call",
            MutationType::OutOfWindow => "out_of_window",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            MutationType::AmountExceeded => "Same transaction but amount × 2 — should exceed spending limit",
            MutationType::WrongAsset => "Same transaction but asset swapped — should be rejected by asset whitelist",
            MutationType::WrongContract => "Same function but different contract address — should fail contract whitelist",
            MutationType::ExtraFunctionCall => "Original + extra function call not in manifest — should be rejected",
            MutationType::OutOfWindow => "Same tx but timestamp past window end — should fail time window check",
        }
    }
}

/// Apply a mutation to a manifest, returning the mutated copy.
pub fn apply_mutation(
    manifest: &CallManifest,
    mutation: &MutationType,
    time_window_seconds: u64,
) -> CallManifest {
    let mut mutated = manifest.clone();

    match mutation {
        MutationType::AmountExceeded => {
            for flow in &mut mutated.asset_flows {
                flow.amount_raw *= 2;
                flow.amount_display = format!(
                    "{}",
                    format_raw(flow.amount_raw, flow.decimals)
                );
            }
            mutated.summary = format!("[MUTATION: amount×2] {}", mutated.summary);
        }

        MutationType::WrongAsset => {
            for flow in &mut mutated.asset_flows {
                // Swap asset to a different known token
                if flow.asset_symbol.as_deref() == Some("USDC") {
                    flow.asset_symbol = Some("EURC".to_string());
                    flow.asset_id = "CEURC_TESTNET_WRONG".to_string();
                } else if flow.asset_symbol.as_deref() == Some("XLM") {
                    flow.asset_symbol = Some("USDC".to_string());
                    flow.asset_id = "CUSDC_TESTNET_WRONG".to_string();
                } else {
                    flow.asset_symbol = Some("WRONGASSET".to_string());
                    flow.asset_id = "CWRONGASSET".to_string();
                }
            }
            mutated.summary = format!("[MUTATION: wrong_asset] {}", mutated.summary);
        }

        MutationType::WrongContract => {
            // Replace first contract with a known-wrong address
            if let Some(contract) = mutated.unique_contracts.first_mut() {
                contract.id = "CWRONG_CONTRACT_ADDRESS_DO_NOT_PERMIT".to_string();
                contract.label = Some("Wrong contract".to_string());
            }
            for node in &mut mutated.top_level_calls {
                swap_contract_id(node, "CWRONG_CONTRACT_ADDRESS_DO_NOT_PERMIT");
            }
            mutated.summary = format!("[MUTATION: wrong_contract] {}", mutated.summary);
        }

        MutationType::ExtraFunctionCall => {
            // Inject an unauthorized function call
            mutated.unique_functions.push("unauthorized_drain".to_string());
            mutated.top_level_calls.push(CallNode {
                contract_id: "CUNAUTHORIZED".to_string(),
                contract_label: None,
                function_name: "unauthorized_drain".to_string(),
                args: vec![serde_json::json!("all_funds")],
                sub_calls: vec![],
                requires_auth: true,
                authorized_by: Some(manifest.invoking_account.clone()),
            });
            mutated.summary = format!("[MUTATION: extra_function_call] {}", mutated.summary);
        }

        MutationType::OutOfWindow => {
            // Advance timestamp beyond the time window
            mutated.timestamp = "2099-01-01T00:00:00Z".to_string();
            mutated.ledger_sequence = mutated.ledger_sequence + 10_000_000;
            mutated.summary = format!("[MUTATION: out_of_window] {}", mutated.summary);
        }
    }

    mutated
}

fn swap_contract_id(node: &mut CallNode, new_id: &str) {
    node.contract_id = new_id.to_string();
    for sub in &mut node.sub_calls {
        swap_contract_id(sub, new_id);
    }
}

fn format_raw(raw: u128, decimals: u8) -> String {
    let divisor = 10u128.pow(decimals as u32);
    let whole = raw / divisor;
    let frac = raw % divisor;
    format!("{}.{:0>width$}", whole, frac, width = decimals as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::manifest::{AssetFlow, CallManifest, ContractRef, FlowDirection};
    use std::collections::HashMap;

    fn base_manifest() -> CallManifest {
        CallManifest {
            transaction_hash: "abc".to_string(),
            network: "testnet".to_string(),
            ledger_sequence: 100,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            invoking_account: "GABC".to_string(),
            top_level_calls: vec![],
            unique_contracts: vec![ContractRef {
                id: "CABC".to_string(),
                label: None,
                protocol: None,
            }],
            unique_functions: vec!["swap".to_string()],
            asset_flows: vec![AssetFlow {
                asset_id: "CUSDC".to_string(),
                asset_symbol: Some("USDC".to_string()),
                direction: FlowDirection::Outbound,
                amount_raw: 50_000_000,
                amount_display: "5.0000000".to_string(),
                decimals: 7,
                counterparty: None,
            }],
            auth_boundaries: vec![],
            observed_amounts: HashMap::new(),
            summary: "Test swap".to_string(),
            is_simulation: false,
            simulation_cost: None,
        }
    }

    #[test]
    fn test_amount_exceeded_doubles_amount() {
        let m = base_manifest();
        let mutated = apply_mutation(&m, &MutationType::AmountExceeded, 86400);
        assert_eq!(mutated.asset_flows[0].amount_raw, 100_000_000);
    }

    #[test]
    fn test_wrong_asset_changes_symbol() {
        let m = base_manifest();
        let mutated = apply_mutation(&m, &MutationType::WrongAsset, 86400);
        assert_ne!(
            mutated.asset_flows[0].asset_symbol,
            Some("USDC".to_string())
        );
    }

    #[test]
    fn test_extra_function_call_adds_node() {
        let m = base_manifest();
        let mutated = apply_mutation(&m, &MutationType::ExtraFunctionCall, 86400);
        assert!(mutated
            .unique_functions
            .contains(&"unauthorized_drain".to_string()));
    }

    #[test]
    fn test_out_of_window_advances_time() {
        let m = base_manifest();
        let mutated = apply_mutation(&m, &MutationType::OutOfWindow, 86400);
        assert!(mutated.timestamp.starts_with("2099"));
    }
}
