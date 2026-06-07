//! Net-new Policy trait codegen.
//!
//! When composition_mode == Generate, this module drives the codegen
//! pipeline to produce a custom Policy trait implementation.

use crate::synthesizer::policy_spec::PolicySpec;

/// Metadata required for generating a new policy crate.
#[derive(Debug, Clone)]
pub struct GenerationRequest {
    /// Sanitized crate name (kebab-case)
    pub crate_name: String,
    /// Human-readable policy name
    pub display_name: String,
    /// Output directory path
    pub output_dir: String,
}

impl GenerationRequest {
    /// Build a generation request from a PolicySpec and output directory.
    pub fn from_spec(spec: &PolicySpec, output_dir: &str) -> Self {
        let crate_name = sanitize_crate_name(&spec.policy_name);
        let display_name = spec.policy_name.replace('-', " ");
        GenerationRequest {
            crate_name,
            display_name,
            output_dir: output_dir.to_string(),
        }
    }
}

/// Sanitize a policy name into a valid Rust crate name.
pub fn sanitize_crate_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_crate_name() {
        assert_eq!(sanitize_crate_name("Blend Yield Claim"), "blend-yield-claim");
        assert_eq!(sanitize_crate_name("swap_exact_tokens"), "swap_exact_tokens");
        assert_eq!(sanitize_crate_name("SEP-41 Sub!"), "sep-41-sub-");
    }
}
