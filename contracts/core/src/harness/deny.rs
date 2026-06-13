//! Deny cases — all 5 mutations must be rejected by the policy.

use crate::harness::mutations::{apply_mutation, MutationType};
use crate::harness::permit::evaluate_policy;
use crate::harness::{HarnessResult, TestOutcome};
use crate::recorder::manifest::CallManifest;
use crate::synthesizer::policy_spec::PolicySpec;

/// Custom deny case (for extra mutations provided by the caller).
#[derive(Debug, Clone)]
pub struct DenyCase {
    pub name: String,
    pub description: String,
    pub mutated_manifest: CallManifest,
}

/// Run all 5 standard deny mutations against the policy.
pub fn run_deny_cases(spec: &PolicySpec, manifest: &CallManifest) -> Vec<HarnessResult> {
    let window_secs = extract_window_secs(spec);

    let mutations = [
        MutationType::AmountExceeded,
        MutationType::WrongAsset,
        MutationType::WrongContract,
        MutationType::ExtraFunctionCall,
        MutationType::OutOfWindow,
    ];

    mutations
        .iter()
        .map(|mutation| {
            let mutated = apply_mutation(manifest, mutation, window_secs);
            let case = DenyCase {
                name: mutation.name().to_string(),
                description: mutation.description().to_string(),
                mutated_manifest: mutated,
            };
            evaluate_deny_case(spec, manifest, &case)
        })
        .collect()
}

/// Evaluate a single deny case.
pub fn evaluate_deny_case(
    spec: &PolicySpec,
    _original: &CallManifest,
    case: &DenyCase,
) -> HarnessResult {
    let (outcome, details) = evaluate_policy(spec, &case.mutated_manifest);

    HarnessResult {
        case_name: case.name.clone(),
        mutation: Some(case.description.clone()),
        expected: TestOutcome::Deny,
        actual: outcome.clone(),
        passed: outcome == TestOutcome::Deny,
        details: format!(
            "{} | Policy eval: {}",
            case.description, details
        ),
    }
}

fn extract_window_secs(spec: &PolicySpec) -> u64 {
    spec.policies
        .iter()
        .find_map(|p| {
            p.params
                .get("window_seconds")
                .and_then(|v| v.as_u64())
        })
        .unwrap_or(86_400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::manifest::{
        AssetFlow, AuthBoundary, CallManifest, ContractRef, FlowDirection,
    };
    use crate::synthesizer::decision_tree::synthesize;
    use std::collections::HashMap;

    fn test_manifest() -> CallManifest {
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
            unique_functions: vec!["claim".to_string()],
            asset_flows: vec![AssetFlow {
                asset_id: "CUSDC".to_string(),
                asset_symbol: Some("USDC".to_string()),
                direction: FlowDirection::Outbound,
                amount_raw: 50_000_000,
                amount_display: "5.0000000".to_string(),
                decimals: 7,
                counterparty: None,
            }],
            auth_boundaries: vec![AuthBoundary {
                contract_id: "CABC".to_string(),
                function_name: "claim".to_string(),
                account: "GABC".to_string(),
            }],
            observed_amounts: HashMap::new(),
            summary: "Blend yield claim".to_string(),
            is_simulation: false,
            simulation_cost: None,
        }
    }

    #[test]
    fn test_deny_amount_exceeded() {
        let manifest = test_manifest();
        let spec = synthesize(&manifest, None);
        let results = run_deny_cases(&spec, &manifest);

        let amount_case = results
            .iter()
            .find(|r| r.case_name == "amount_exceeded")
            .expect("amount_exceeded case missing");

        assert!(
            amount_case.passed,
            "amount_exceeded should be DENIED but got: {}",
            amount_case.details
        );
    }

    #[test]
    fn test_deny_wrong_contract() {
        let manifest = test_manifest();
        let spec = synthesize(&manifest, None);
        let results = run_deny_cases(&spec, &manifest);

        let case = results
            .iter()
            .find(|r| r.case_name == "wrong_contract")
            .expect("wrong_contract case missing");

        assert!(
            case.passed,
            "wrong_contract should be DENIED but got: {}",
            case.details
        );
    }

    #[test]
    fn test_deny_out_of_window() {
        let manifest = test_manifest();
        let spec = synthesize(&manifest, None);
        let results = run_deny_cases(&spec, &manifest);

        let case = results
            .iter()
            .find(|r| r.case_name == "out_of_window")
            .expect("out_of_window case missing");

        assert!(
            case.passed,
            "out_of_window should be DENIED but got: {}",
            case.details
        );
    }

    #[test]
    fn test_all_five_deny_cases_generated() {
        let manifest = test_manifest();
        let spec = synthesize(&manifest, None);
        let results = run_deny_cases(&spec, &manifest);
        assert_eq!(results.len(), 5, "Must have exactly 5 deny cases");
    }
}
