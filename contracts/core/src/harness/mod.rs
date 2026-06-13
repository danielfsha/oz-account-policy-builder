//! Harness — permit/deny test runner for synthesized policies.
//!
//! Runs the original transaction context (must pass) and 5 auto-generated
//! mutations (must all fail).

pub mod deny;
pub mod mutations;
pub mod permit;

use crate::recorder::manifest::CallManifest;
use crate::synthesizer::policy_spec::PolicySpec;
use serde::{Deserialize, Serialize};

/// Full harness report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessReport {
    /// Original tx result — must be Permit
    pub permit_result: HarnessResult,
    /// Mutation results — must all be Deny
    pub deny_results: Vec<HarnessResult>,
    /// true only if permit passes AND all deny cases fail
    pub passed: bool,
    /// Human-readable full report
    pub report: String,
}

/// Result of a single test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessResult {
    /// Descriptive name for this case
    pub case_name: String,
    /// Mutation applied (None for the original)
    pub mutation: Option<String>,
    /// Expected outcome
    pub expected: TestOutcome,
    /// Actual outcome from policy evaluation
    pub actual: TestOutcome,
    /// Did it match expected?
    pub passed: bool,
    /// Details of the evaluation
    pub details: String,
}

/// Expected or actual test outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TestOutcome {
    Permit,
    Deny,
}

/// Run the full harness — permit + all 5 deny mutations.
pub fn run_harness(
    spec: &PolicySpec,
    manifest: &CallManifest,
    custom_deny_cases: Option<Vec<deny::DenyCase>>,
) -> HarnessReport {
    let permit_result = permit::run_permit_case(spec, manifest);
    let mut deny_results = deny::run_deny_cases(spec, manifest);

    if let Some(extra) = custom_deny_cases {
        for case in extra {
            deny_results.push(deny::evaluate_deny_case(spec, manifest, &case));
        }
    }

    let all_deny_passed = deny_results.iter().all(|r| r.passed);
    let passed = permit_result.passed && all_deny_passed;

    let report = build_report(&permit_result, &deny_results, passed);

    HarnessReport {
        permit_result,
        deny_results,
        passed,
        report,
    }
}

fn build_report(
    permit: &HarnessResult,
    deny_results: &[HarnessResult],
    passed: bool,
) -> String {
    let mut lines = vec![
        "═══════════════════════════════════════".to_string(),
        " OZ Policy Builder — Harness Report".to_string(),
        "═══════════════════════════════════════".to_string(),
        format!(
            "PERMIT: {} — {}",
            if permit.passed { "✅ PASS" } else { "❌ FAIL" },
            permit.details
        ),
        String::new(),
        "DENY CASES:".to_string(),
    ];

    for dr in deny_results {
        lines.push(format!(
            "  {} [{}]: {} — {}",
            if dr.passed { "✅" } else { "❌" },
            dr.case_name,
            dr.mutation.as_deref().unwrap_or("—"),
            dr.details
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "OVERALL: {}",
        if passed {
            "✅ ALL CASES PASSED — safe to generate code"
        } else {
            "❌ HARNESS FAILED — re-synthesize with tighter constraints"
        }
    ));

    lines.join("\n")
}
