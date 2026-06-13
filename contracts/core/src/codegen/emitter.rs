//! Fill templates, emit crate.
//!
//! AUTO-DEPLOY IS INTENTIONALLY NOT IMPLEMENTED.
//! This is a security property: the tool generates reviewable code; deployment
//! is always a separate, explicit step performed by the user.

use crate::codegen::templates::{
    render, TemplateVars, CARGO_TOML, GITIGNORE, LIB_RS_COMPOSE, LIB_RS_GENERATE, REVIEW_MD,
};
use crate::synthesizer::policy_spec::{CompositionMode, PolicyLayerKind, PolicySpec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Output types ──────────────────────────────────────────────────────────────

/// The output of emitting a policy crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmittedCrate {
    /// Suggested path for the generated crate root (relative to project root)
    pub crate_path: String,
    /// All files to write, keyed by path relative to `crate_path`
    pub files: HashMap<String, String>,
    /// Full text of REVIEW.md (also included in `files`)
    pub review_md: String,
    /// Human-readable summary of what was generated
    pub summary: EmitSummary,
}

/// Human-readable summary of the emit result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmitSummary {
    /// Crate name
    pub crate_name: String,
    /// Composition mode used
    pub mode: String,
    /// Number of policy layers emitted
    pub layer_count: usize,
    /// List of layer descriptions
    pub layers: Vec<String>,
    /// Whether net-new code was generated (vs compose-only)
    pub has_custom_code: bool,
    /// Clarifications that were still pending (warnings)
    pub pending_clarifications: Vec<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Emit a full policy crate from a `PolicySpec`.
///
/// Returns an `EmittedCrate` with all file contents. The caller is responsible
/// for writing files to disk — this function only produces strings.
///
/// # Errors
/// Returns `EmitError` if the spec is structurally invalid.
pub fn emit_policy_crate(spec: &PolicySpec, output_dir: Option<&str>) -> Result<EmittedCrate, EmitError> {
    validate_spec(spec)?;

    let vars = build_vars(spec);
    let crate_name = vars.crate_name.clone();
    let crate_path = match output_dir {
        Some(dir) => format!("{}/{}", dir.trim_end_matches('/'), crate_name),
        None => format!("generated/{}", crate_name),
    };

    let mut files: HashMap<String, String> = HashMap::new();

    // Cargo.toml
    files.insert("Cargo.toml".to_string(), render(CARGO_TOML, &vars));

    // .gitignore
    files.insert(".gitignore".to_string(), GITIGNORE.to_string());

    // src/lib.rs — choose template based on composition mode
    let lib_rs = match spec.composition_mode {
        CompositionMode::Compose => render(LIB_RS_COMPOSE, &vars),
        CompositionMode::Generate => render(LIB_RS_GENERATE, &vars),
    };
    files.insert("src/lib.rs".to_string(), lib_rs);

    // REVIEW.md
    let review_md = render(REVIEW_MD, &vars);
    files.insert("REVIEW.md".to_string(), review_md.clone());

    let has_custom_code = spec.composition_mode == CompositionMode::Generate
        || spec.policies.iter().any(|p| !p.oz_primitive);

    let summary = EmitSummary {
        crate_name: crate_name.clone(),
        mode: format!("{:?}", spec.composition_mode),
        layer_count: spec.policies.len(),
        layers: spec.policies.iter().map(|p| p.description.clone()).collect(),
        has_custom_code,
        pending_clarifications: spec
            .clarifications_needed
            .iter()
            .map(|c| format!("[{}] {}", c.field, c.question))
            .collect(),
    };

    Ok(EmittedCrate {
        crate_path,
        files,
        review_md,
        summary,
    })
}

// ── Validation ────────────────────────────────────────────────────────────────

/// Errors that can occur during emission.
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum EmitError {
    #[error("Policy has {0} layers; OZ accounts supports at most 5")]
    TooManyPolicies(usize),
    #[error("Policy name is empty")]
    EmptyPolicyName,
    #[error("Policy name contains invalid characters: {0}")]
    InvalidPolicyName(String),
    #[error("SpendingLimit layer is missing required param: {0}")]
    MissingSpendingLimitParam(String),
}

fn validate_spec(spec: &PolicySpec) -> Result<(), EmitError> {
    if spec.policy_name.trim().is_empty() {
        return Err(EmitError::EmptyPolicyName);
    }

    // Policy name must be valid kebab/snake case for a Rust crate name
    let valid = spec.policy_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(EmitError::InvalidPolicyName(spec.policy_name.clone()));
    }

    if spec.policies.len() > 5 {
        return Err(EmitError::TooManyPolicies(spec.policies.len()));
    }

    // SpendingLimit layers must have cap_amount_raw
    for layer in &spec.policies {
        if layer.kind == PolicyLayerKind::SpendingLimit {
            if !layer.params.contains_key("cap_amount_raw") {
                return Err(EmitError::MissingSpendingLimitParam("cap_amount_raw".to_string()));
            }
        }
    }

    Ok(())
}

// ── Variable builder ──────────────────────────────────────────────────────────

fn build_vars(spec: &PolicySpec) -> TemplateVars {
    let crate_name = sanitize_crate_name(&spec.policy_name);
    let policy_struct_name = to_pascal_case(&crate_name);

    // Extract spending limit params
    let spending_layer = spec.policies.iter().find(|p| p.kind == PolicyLayerKind::SpendingLimit);
    let spending_limit_cap = spending_layer
        .and_then(|l| l.params.get("cap_amount_raw"))
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_u64().map(|n| n.to_string())))
        .unwrap_or_else(|| "100_000_000".to_string());

    let spending_limit_asset = spending_layer
        .and_then(|l| l.params.get("asset_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("CUNKNOWN")
        .to_string();

    let spending_limit_window = spending_layer
        .and_then(|l| l.params.get("window_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(86_400)
        .to_string();

    // Extract time window params (prefer time_window layer, fallback to spending window)
    let time_window_layer = spec.policies.iter().find(|p| p.kind == PolicyLayerKind::TimeWindow);
    let time_window_secs = time_window_layer
        .and_then(|l| l.params.get("window_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| spending_limit_window.parse().unwrap_or(86_400))
        .to_string();

    // Extract slippage param
    let slippage_percent = spec
        .policies
        .iter()
        .find(|p| p.kind == PolicyLayerKind::Custom)
        .and_then(|l| l.params.get("max_slippage_percent"))
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0)
        .to_string();

    // Context rule
    let context_contracts = format_string_list(&spec.context_rule.contracts);
    let context_functions = format_string_list(&spec.context_rule.functions);
    let context_lifetime_secs = spec.context_rule.lifetime_seconds.to_string();

    // Policy description from layers
    let layer_descriptions: Vec<&str> = spec.policies.iter().map(|p| p.description.as_str()).collect();
    let policy_description = if layer_descriptions.is_empty() {
        format!("Generated by OZ Policy Builder for '{}'", spec.policy_name)
    } else {
        layer_descriptions.join("; ")
    };

    TemplateVars {
        crate_name,
        policy_struct_name,
        policy_display_name: spec.policy_name.replace('-', " "),
        spending_limit_cap,
        spending_limit_asset,
        spending_limit_window,
        time_window_secs,
        slippage_percent,
        context_contracts,
        context_functions,
        context_lifetime_secs,
        policy_description,
        rationale: spec.rationale.clone(),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a kebab/snake crate name to PascalCase struct name.
fn to_pascal_case(name: &str) -> String {
    name.split(|c| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Sanitize any string into a valid Rust crate name.
pub fn sanitize_crate_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase();

    sanitized.trim_matches(|c| c == '-' || c == '_').to_string()
}

/// Format a string slice as a Rust array literal for inclusion in code.
fn format_string_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{}\"", s)).collect();
    format!("[{}]", quoted.join(", "))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::manifest::{AssetFlow, AuthBoundary, CallManifest, ContractRef, FlowDirection};
    use crate::synthesizer::decision_tree::synthesize;
    use std::collections::HashMap;

    fn blend_manifest() -> CallManifest {
        CallManifest {
            transaction_hash: "abc123".to_string(),
            network: "testnet".to_string(),
            ledger_sequence: 100,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            invoking_account: "GABC".to_string(),
            top_level_calls: vec![],
            unique_contracts: vec![ContractRef {
                id: "CBLEND_POOL".to_string(),
                label: Some("Blend Pool".to_string()),
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
                counterparty: Some("CBLEND_POOL".to_string()),
            }],
            auth_boundaries: vec![AuthBoundary {
                contract_id: "CBLEND_POOL".to_string(),
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
    fn test_emit_compose_mode() {
        let manifest = blend_manifest();
        let spec = synthesize(&manifest, None);
        let emitted = emit_policy_crate(&spec, None).expect("emit failed");

        assert!(emitted.files.contains_key("Cargo.toml"));
        assert!(emitted.files.contains_key("src/lib.rs"));
        assert!(emitted.files.contains_key("REVIEW.md"));
        assert!(emitted.files.contains_key(".gitignore"));
    }

    #[test]
    fn test_emit_crate_path_uses_output_dir() {
        let manifest = blend_manifest();
        let spec = synthesize(&manifest, None);
        let emitted = emit_policy_crate(&spec, Some("/tmp/policies")).expect("emit failed");
        assert!(emitted.crate_path.starts_with("/tmp/policies/"));
    }

    #[test]
    fn test_emit_cargo_toml_has_crate_name() {
        let manifest = blend_manifest();
        let spec = synthesize(&manifest, None);
        let emitted = emit_policy_crate(&spec, None).expect("emit failed");
        let cargo = &emitted.files["Cargo.toml"];
        assert!(cargo.contains(&emitted.summary.crate_name), "Cargo.toml must contain crate name");
    }

    #[test]
    fn test_emit_review_md_has_spending_info() {
        let manifest = blend_manifest();
        let spec = synthesize(&manifest, None);
        let emitted = emit_policy_crate(&spec, None).expect("emit failed");
        assert!(emitted.review_md.contains("50000000") || emitted.review_md.contains("100_000_000"));
    }

    #[test]
    fn test_emit_rejects_too_many_policies() {
        use crate::synthesizer::policy_spec::{CompositionMode, ContextRule, PolicyLayer, PolicyLayerKind};
        let spec = PolicySpec {
            policy_name: "over-limit".to_string(),
            context_rule: ContextRule::default(),
            composition_mode: CompositionMode::Compose,
            rationale: String::new(),
            clarifications_needed: vec![],
            policies: (0..6).map(|i| PolicyLayer {
                kind: PolicyLayerKind::SimpleThreshold,
                params: HashMap::new(),
                oz_primitive: true,
                description: format!("Layer {}", i),
            }).collect(),
        };
        let result = emit_policy_crate(&spec, None);
        assert!(matches!(result, Err(EmitError::TooManyPolicies(6))));
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("blend-yield-claim"), "BlendYieldClaim");
        assert_eq!(to_pascal_case("custom_policy"), "CustomPolicy");
        assert_eq!(to_pascal_case("sep41"), "Sep41");
    }

    #[test]
    fn test_sanitize_crate_name() {
        assert_eq!(sanitize_crate_name("Blend Yield Claim!"), "blend-yield-claim-");
        assert_eq!(sanitize_crate_name("---my-policy---"), "my-policy");
    }

    #[test]
    fn test_emit_generate_mode_has_policy_trait() {
        use crate::synthesizer::policy_spec::{CompositionMode, ContextRule, PolicyLayer, PolicyLayerKind};
        use std::collections::HashMap as HM;

        let mut spending_params = HM::new();
        spending_params.insert("cap_amount_raw".to_string(), serde_json::json!("50000000"));
        spending_params.insert("asset_id".to_string(), serde_json::json!("CUSDC"));
        spending_params.insert("window_seconds".to_string(), serde_json::json!(86400u64));

        let spec = PolicySpec {
            policy_name: "my-generate-policy".to_string(),
            context_rule: ContextRule::default(),
            composition_mode: CompositionMode::Generate,
            rationale: "Net-new codegen required for slippage".to_string(),
            clarifications_needed: vec![],
            policies: vec![PolicyLayer {
                kind: PolicyLayerKind::SpendingLimit,
                params: spending_params,
                oz_primitive: false,
                description: "Custom spending limit".to_string(),
            }],
        };

        let emitted = emit_policy_crate(&spec, None).expect("emit failed");
        let lib = &emitted.files["src/lib.rs"];
        assert!(lib.contains("contractimpl"), "generate mode must include contractimpl");
        assert!(lib.contains("enforce"), "generate mode must include enforce fn");
    }
}
