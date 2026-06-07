//! Synthesizer decision tree.
//!
//! Implements the exact decision order from the spec:
//!   1. Count unique contracts → flag if > 3
//!   2. Check asset_flows → spending_limit if outbound flows exist
//!   3. Check auth_boundaries for time patterns → time_window
//!   4. Single vs multiple counterparties → simple_threshold vs weighted
//!   5. Check if anything can't be composed → composition_mode = generate
//!   6. Validate: total policies ≤ 5
//!   7. Build context_rule

use crate::recorder::manifest::{CallManifest, FlowDirection};
use crate::synthesizer::policy_spec::{
    Clarification, CompositionMode, ContextRule, PolicyLayer, PolicyLayerKind, PolicySpec,
    Constraints,
};
use std::collections::HashMap;

/// Seconds in a day / week / month
const ONE_DAY_SECS: u64 = 86_400;
const ONE_WEEK_SECS: u64 = 7 * ONE_DAY_SECS;
const ONE_MONTH_SECS: u64 = 30 * ONE_DAY_SECS;
const ONE_YEAR_SECS: u64 = 365 * ONE_DAY_SECS;

/// Run the decision tree against a manifest and produce a PolicySpec.
pub fn synthesize(manifest: &CallManifest, constraints: Option<&Constraints>) -> PolicySpec {
    let c = constraints.cloned().unwrap_or_default();
    let mut policies: Vec<PolicyLayer> = Vec::new();
    let mut clarifications: Vec<Clarification> = Vec::new();
    let mut needs_custom = false;

    // ── Step 1: Count unique contracts ─────────────────────────────────
    if manifest.unique_contracts.len() > 3 {
        clarifications.push(Clarification {
            field: "contract_complexity".to_string(),
            question: format!(
                "This transaction involves {} contracts, which is complex. \
                 Should I generate a custom policy, or attempt to compose primitives?",
                manifest.unique_contracts.len()
            ),
            options: Some(vec![
                "Compose primitives (simpler)".to_string(),
                "Generate custom policy (more precise)".to_string(),
            ]),
        });
        needs_custom = true;
    }

    // ── Step 2: Asset flows → spending_limit ───────────────────────────
    let outbound_flows: Vec<_> = manifest
        .asset_flows
        .iter()
        .filter(|f| f.direction == FlowDirection::Outbound)
        .collect();

    if !outbound_flows.is_empty() {
        let first_flow = &outbound_flows[0];
        let observed_amount = first_flow.amount_raw;
        let cap = c.amount_cap.unwrap_or(observed_amount);
        let asset_symbol = first_flow
            .asset_symbol
            .clone()
            .unwrap_or_else(|| first_flow.asset_id[..8.min(first_flow.asset_id.len())].to_string());

        // Infer time window from context
        let window_secs = c.time_window_seconds.unwrap_or_else(|| infer_window(manifest));
        let window_label = format_window_label(window_secs);

        if c.amount_cap.is_none() {
            clarifications.push(Clarification {
                field: "amount_cap".to_string(),
                question: format!(
                    "This transaction sent {} {}. Should I cap the policy at exactly {} {}, \
                     or allow some headroom — say {} {} per {}?",
                    first_flow.amount_display,
                    asset_symbol,
                    first_flow.amount_display,
                    asset_symbol,
                    format_amount_with_headroom(observed_amount, 1.2, first_flow.decimals),
                    asset_symbol,
                    window_label,
                ),
                options: Some(vec![
                    format!("Exact: {} {}", first_flow.amount_display, asset_symbol),
                    format!(
                        "With 20% headroom: {} {} per {}",
                        format_amount_with_headroom(observed_amount, 1.2, first_flow.decimals),
                        asset_symbol,
                        window_label
                    ),
                ]),
            });
        }

        let mut params = HashMap::new();
        params.insert("cap_amount_raw".to_string(), serde_json::json!(cap.to_string()));
        params.insert("asset_id".to_string(), serde_json::json!(first_flow.asset_id));
        params.insert("asset_symbol".to_string(), serde_json::json!(asset_symbol));
        params.insert("window_seconds".to_string(), serde_json::json!(window_secs));
        params.insert("decimals".to_string(), serde_json::json!(first_flow.decimals));

        policies.push(PolicyLayer {
            kind: PolicyLayerKind::SpendingLimit,
            params,
            oz_primitive: true,
            description: format!(
                "Limit outbound {} to {} per {}",
                asset_symbol, first_flow.amount_display, window_label
            ),
        });
    }

    // ── Step 3: Time patterns → time_window ────────────────────────────
    // Check auth_boundaries — if multiple calls or a subscription-like pattern exists
    let window_secs = c.time_window_seconds.unwrap_or_else(|| infer_window(manifest));
    let window_label = format_window_label(window_secs);

    if c.time_window_seconds.is_none() && !manifest.auth_boundaries.is_empty() {
        clarifications.push(Clarification {
            field: "time_window_seconds".to_string(),
            question: "The transaction ran once. Should this policy allow it daily, weekly, or on a different schedule?".to_string(),
            options: Some(vec![
                "Daily (86400 seconds)".to_string(),
                "Weekly (604800 seconds)".to_string(),
                "Monthly (2592000 seconds)".to_string(),
            ]),
        });
    }

    let mut tw_params = HashMap::new();
    tw_params.insert("window_seconds".to_string(), serde_json::json!(window_secs));
    tw_params.insert("window_label".to_string(), serde_json::json!(window_label.clone()));

    policies.push(PolicyLayer {
        kind: PolicyLayerKind::TimeWindow,
        params: tw_params,
        oz_primitive: true,
        description: format!("Restrict invocation to once per {}", window_label),
    });

    // ── Step 4: Single vs multiple counterparties ─────────────────────
    let unique_contract_count = manifest.unique_contracts.len();
    let unique_fn_count = manifest.unique_functions.len();

    if unique_contract_count <= 1 && unique_fn_count <= 1 {
        // Single counterparty — simple_threshold(1)
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), serde_json::json!(1));

        // Ask about contract locking
        if !manifest.unique_contracts.is_empty() && c.amount_cap.is_none() {
            let contract_id = &manifest.unique_contracts[0].id;
            let protocol_label = manifest.unique_contracts[0]
                .protocol
                .clone()
                .unwrap_or_else(|| "this protocol".to_string());
            clarifications.push(Clarification {
                field: "contract_lock".to_string(),
                question: format!(
                    "I see {} was called. Should I lock the policy to this exact contract address ({}), \
                     or allow any {} contract?",
                    protocol_label, contract_id, protocol_label
                ),
                options: Some(vec![
                    format!("Lock to exact address: {}", contract_id),
                    format!("Allow any {} contract", protocol_label),
                ]),
            });
        }

        policies.push(PolicyLayer {
            kind: PolicyLayerKind::SimpleThreshold,
            params,
            oz_primitive: true,
            description: "Single authorized signer is sufficient".to_string(),
        });
    } else {
        // Multiple counterparties — weighted_threshold
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), serde_json::json!(1));
        params.insert("total_signers".to_string(), serde_json::json!(1));

        policies.push(PolicyLayer {
            kind: PolicyLayerKind::WeightedThreshold,
            params,
            oz_primitive: true,
            description: "Weighted threshold for multi-contract interaction".to_string(),
        });
    }

    // ── Step 5: Check for slippage (swap use case) ────────────────────
    let is_swap = manifest
        .unique_functions
        .iter()
        .any(|f| f.contains("swap") || f.contains("exchange"));

    if is_swap && c.allow_slippage_percent.is_none() {
        needs_custom = true;
        let slippage = c.allow_slippage_percent.unwrap_or(5.0);
        let mut params = HashMap::new();
        params.insert("max_slippage_percent".to_string(), serde_json::json!(slippage));
        params.insert("reason".to_string(), serde_json::json!(
            "slippage_check cannot be composed from existing OZ primitives — net-new codegen required"
        ));

        policies.push(PolicyLayer {
            kind: PolicyLayerKind::Custom,
            params,
            oz_primitive: false,
            description: format!(
                "Slippage guard: reject swap if slippage exceeds {}%",
                slippage
            ),
        });
    }

    // ── Step 6: Validate policy count ≤ 5 ────────────────────────────
    if policies.len() > 5 {
        // Drop time_window if spending_limit already encodes a window
        let has_spending_with_window = policies.iter().any(|p| {
            p.kind == PolicyLayerKind::SpendingLimit && p.params.contains_key("window_seconds")
        });
        if has_spending_with_window {
            policies.retain(|p| p.kind != PolicyLayerKind::TimeWindow);
        }
        policies.truncate(5);
    }

    // ── Step 7: Build context_rule ────────────────────────────────────
    let context_rule = ContextRule {
        contracts: manifest.unique_contracts.iter().map(|c| c.id.clone()).collect(),
        functions: manifest.unique_functions.clone(),
        lifetime_seconds: c.lifetime_seconds.unwrap_or(ONE_YEAR_SECS),
    };

    // Determine composition mode
    let composition_mode = if needs_custom || policies.iter().any(|p| !p.oz_primitive) {
        CompositionMode::Generate
    } else {
        CompositionMode::Compose
    };

    // Build rationale
    let rationale = build_rationale(&policies, manifest, composition_mode == CompositionMode::Generate);

    // Build policy name from unique functions
    let policy_name = build_policy_name(manifest);

    PolicySpec {
        context_rule,
        policies,
        composition_mode,
        rationale,
        clarifications_needed: clarifications,
        policy_name,
    }
}

fn infer_window(manifest: &CallManifest) -> u64 {
    // Heuristic: subscription/transfer patterns → monthly, swaps → daily, yield → weekly
    let fns_lower: Vec<String> = manifest
        .unique_functions
        .iter()
        .map(|f| f.to_lowercase())
        .collect();

    if fns_lower.iter().any(|f| f.contains("subscription") || f.contains("subscribe")) {
        ONE_MONTH_SECS
    } else if fns_lower.iter().any(|f| f.contains("claim") || f.contains("yield")) {
        ONE_WEEK_SECS
    } else {
        ONE_DAY_SECS
    }
}

fn format_window_label(secs: u64) -> String {
    match secs {
        s if s <= ONE_DAY_SECS => "day".to_string(),
        s if s <= ONE_WEEK_SECS => "week".to_string(),
        s if s <= ONE_MONTH_SECS => "month".to_string(),
        _ => "year".to_string(),
    }
}

fn format_amount_with_headroom(raw: u128, multiplier: f64, decimals: u8) -> String {
    let with_headroom = (raw as f64 * multiplier) as u128;
    let divisor = 10u128.pow(decimals as u32);
    let whole = with_headroom / divisor;
    let frac = with_headroom % divisor;
    format!("{}.{:0>width$}", whole, frac, width = decimals as usize)
}

fn build_rationale(layers: &[PolicyLayer], manifest: &CallManifest, is_custom: bool) -> String {
    let kinds: Vec<String> = layers.iter().map(|l| format!("{:?}", l.kind)).collect();
    let mut parts = vec![format!("Policy composition: {}", kinds.join(" + "))];

    if manifest.asset_flows.iter().any(|f| f.direction == FlowDirection::Outbound) {
        parts.push("spending_limit added because transaction has outbound asset flow".to_string());
    }
    if !manifest.auth_boundaries.is_empty() {
        parts.push("time_window added to restrict invocation frequency".to_string());
    }
    if is_custom {
        parts.push(
            "custom layer required — constraint cannot be expressed by existing OZ primitives"
                .to_string(),
        );
    }

    parts.join(". ")
}

fn build_policy_name(manifest: &CallManifest) -> String {
    if let Some(fn_name) = manifest.unique_functions.first() {
        // Convert snake_case to kebab-case
        fn_name.replace('_', "-")
    } else if let Some(contract) = manifest.unique_contracts.first() {
        if let Some(label) = &contract.label {
            label.to_lowercase().replace(' ', "-")
        } else {
            "custom-policy".to_string()
        }
    } else {
        "custom-policy".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::manifest::{
        AssetFlow, AuthBoundary, CallManifest, CallNode, ContractRef, FlowDirection,
    };
    use std::collections::HashMap;

    fn base_manifest() -> CallManifest {
        CallManifest {
            transaction_hash: "abc123".to_string(),
            network: "testnet".to_string(),
            ledger_sequence: 100,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            invoking_account: "GABC".to_string(),
            top_level_calls: vec![],
            unique_contracts: vec![ContractRef {
                id: "CABC".to_string(),
                label: Some("Blend".to_string()),
                protocol: Some("blend".to_string()),
            }],
            unique_functions: vec!["claim".to_string()],
            asset_flows: vec![AssetFlow {
                asset_id: "CUSDC".to_string(),
                asset_symbol: Some("USDC".to_string()),
                direction: FlowDirection::Outbound,
                amount_raw: 50_000_000,
                amount_display: "5.0000000".to_string(),
                decimals: 7,
                counterparty: Some("BLEND_POOL".to_string()),
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
    fn test_synthesize_blend_claim() {
        let manifest = base_manifest();
        let spec = synthesize(&manifest, None);
        assert!(!spec.policies.is_empty());
        assert!(spec.policies.len() <= 5);
        // Should have spending_limit due to outbound flow
        assert!(spec
            .policies
            .iter()
            .any(|p| p.kind == PolicyLayerKind::SpendingLimit));
        // Should have time_window
        assert!(spec
            .policies
            .iter()
            .any(|p| p.kind == PolicyLayerKind::TimeWindow));
    }

    #[test]
    fn test_synthesize_with_constraints() {
        let manifest = base_manifest();
        let constraints = Constraints {
            amount_cap: Some(100_000_000),
            time_window_seconds: Some(ONE_WEEK_SECS),
            ..Default::default()
        };
        let spec = synthesize(&manifest, Some(&constraints));
        // With explicit constraints, no clarifications about amount/time
        let amount_clarification = spec
            .clarifications_needed
            .iter()
            .any(|c| c.field == "amount_cap");
        assert!(!amount_clarification);
    }

    #[test]
    fn test_max_five_policies() {
        let mut manifest = base_manifest();
        // Add many contracts to stress policy count
        for i in 0..5 {
            manifest.unique_contracts.push(ContractRef {
                id: format!("C{i}"),
                label: None,
                protocol: None,
            });
        }
        let spec = synthesize(&manifest, None);
        assert!(spec.policies.len() <= 5);
    }
}
