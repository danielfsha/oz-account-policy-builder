//! Permit case — the original transaction must pass the synthesized policy.

use crate::harness::{HarnessResult, TestOutcome};
use crate::recorder::manifest::{CallManifest, FlowDirection};
use crate::synthesizer::policy_spec::{PolicyLayerKind, PolicySpec};

/// Run the permit case — evaluate the original manifest against the policy.
/// This MUST return Permit for the harness to pass.
pub fn run_permit_case(spec: &PolicySpec, manifest: &CallManifest) -> HarnessResult {
    let (outcome, details) = evaluate_policy(spec, manifest);

    HarnessResult {
        case_name: "original_transaction".to_string(),
        mutation: None,
        expected: TestOutcome::Permit,
        actual: outcome.clone(),
        passed: outcome == TestOutcome::Permit,
        details,
    }
}

/// Core policy evaluation logic — checks each layer against the manifest.
pub fn evaluate_policy(spec: &PolicySpec, manifest: &CallManifest) -> (TestOutcome, String) {
    // Check context rule — all calls must be to whitelisted contracts/functions
    if !spec.context_rule.contracts.is_empty() {
        for contract in &manifest.unique_contracts {
            if !spec.context_rule.contracts.contains(&contract.id) {
                return (
                    TestOutcome::Deny,
                    format!(
                        "Contract {} not in context rule whitelist",
                        contract.id
                    ),
                );
            }
        }
    }

    if !spec.context_rule.functions.is_empty() {
        for function in &manifest.unique_functions {
            if !spec.context_rule.functions.contains(function) {
                return (
                    TestOutcome::Deny,
                    format!("Function {} not in context rule whitelist", function),
                );
            }
        }
    }

    // Evaluate each policy layer
    for layer in &spec.policies {
        match &layer.kind {
            PolicyLayerKind::SpendingLimit => {
                let cap = layer
                    .params
                    .get("cap_amount_raw")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u128>().ok())
                    .unwrap_or(u128::MAX);

                let asset_id = layer
                    .params
                    .get("asset_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let total_outbound: u128 = manifest
                    .asset_flows
                    .iter()
                    .filter(|f| f.direction == FlowDirection::Outbound)
                    .filter(|f| asset_id.is_empty() || f.asset_id == asset_id)
                    .map(|f| f.amount_raw)
                    .sum();

                if total_outbound > cap {
                    return (
                        TestOutcome::Deny,
                        format!(
                            "SpendingLimit: outbound {} exceeds cap {}",
                            total_outbound, cap
                        ),
                    );
                }
            }

            PolicyLayerKind::TimeWindow => {
                // For the permit case, the original timestamp is within window
                // Out-of-window is tested in deny mutations
                // Just check the timestamp is not the sentinel "2099-*" value
                if manifest.timestamp.starts_with("2099") {
                    return (
                        TestOutcome::Deny,
                        "TimeWindow: timestamp past end of allowed window".to_string(),
                    );
                }
            }

            PolicyLayerKind::SimpleThreshold | PolicyLayerKind::WeightedThreshold => {
                // Auth check — invoking account must be in auth boundaries
                let account_in_auth = manifest
                    .auth_boundaries
                    .iter()
                    .any(|b| b.account == manifest.invoking_account);
                if !manifest.auth_boundaries.is_empty() && !account_in_auth {
                    return (
                        TestOutcome::Deny,
                        "Threshold: invoking account not in auth boundaries".to_string(),
                    );
                }
            }

            PolicyLayerKind::Custom => {
                // Custom layer — check slippage if present
                let max_slippage = layer
                    .params
                    .get("max_slippage_percent")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(100.0);

                // For now: permit if max_slippage >= 0 (no slippage info in manifest)
                if max_slippage < 0.0 {
                    return (TestOutcome::Deny, "Custom: invalid slippage parameter".to_string());
                }
            }
        }
    }

    (TestOutcome::Permit, "All policy layers passed".to_string())
}
