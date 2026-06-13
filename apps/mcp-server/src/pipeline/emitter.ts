/**
 * Emit a compilable Rust Soroban policy crate from a PolicySpec.
 *
 * AUTO-DEPLOY IS INTENTIONALLY NOT IMPLEMENTED.
 * Returns file contents as strings — caller writes them to disk.
 */

import type { PolicySpec, PolicyLayer } from "./synthesizer";

export interface EmittedCrate {
  crate_path: string;
  files: Record<string, string>;
  review_md: string;
  summary: {
    crate_name: string;
    mode: string;
    layer_count: number;
    layers: string[];
    has_custom_code: boolean;
    pending_clarifications: string[];
  };
}

export function emitPolicyCrate(spec: PolicySpec, outputDir?: string): EmittedCrate {
  validateSpec(spec);

  const crateName = sanitizeCrateName(spec.policy_name);
  const structName = toPascalCase(crateName);
  const crate_path = `${(outputDir ?? "generated").replace(/\/$/, "")}/${crateName}`;

  const spendingLayer = spec.policies.find((p) => p.kind === "spending_limit");
  const timeLayer = spec.policies.find((p) => p.kind === "time_window");

  const cap = spendingLayer?.params.cap_amount_raw ?? "100_000_000";
  const windowSecs = (spendingLayer?.params.window_seconds ?? timeLayer?.params.window_seconds ?? 86400) as number;
  const periodLedgers = Math.round(windowSecs / 5);
  const assetId = spendingLayer?.params.asset_id ?? "CUNKNOWN";
  const assetSymbol = spendingLayer?.params.asset_symbol ?? "TOKEN";
  const lifetimeSecs = spec.context_rule.lifetime_seconds;

  const contextContracts = JSON.stringify(spec.context_rule.contracts);
  const contextFunctions = JSON.stringify(spec.context_rule.functions);
  const layerDescriptions = spec.policies.map((p) => p.description).join("; ");

  const isGenerate = spec.composition_mode === "generate";
  const hasCustomCode = isGenerate || spec.policies.some((p) => !p.oz_primitive);

  // Determine protocol from unique contracts in the original manifest
  // For now, we'll use a simple detection based on contract labels/IDs
  let protocol: string | undefined;
  
  // Check if we have access to the original manifest or its unique_contracts
  // This is a placeholder - in reality, we'd need to pass this information
  // For now, we'll leave it undefined and the generated code will work without protocol adapters

  const files: Record<string, string> = {
    "Cargo.toml": renderCargoToml(crateName, layerDescriptions, protocol),
    ".gitignore": "target/\nCargo.lock\n",
    "src/lib.rs": isGenerate
      ? renderGenerateLib(structName, cap, periodLedgers, assetId, spec.rationale, protocol)
      : renderComposeLib(structName, spec.rationale, spec.policies),
    "REVIEW.md": renderReviewMd({
      crateName,
      displayName: spec.policy_name.replace(/-/g, " "),
      description: layerDescriptions,
      rationale: spec.rationale,
      contextContracts,
      contextFunctions,
      lifetimeSecs,
      cap,
      windowSecs,
    }),
  };

  return {
    crate_path,
    files,
    review_md: files["REVIEW.md"],
    summary: {
      crate_name: crateName,
      mode: spec.composition_mode,
      layer_count: spec.policies.length,
      layers: spec.policies.map((p) => p.description),
      has_custom_code: hasCustomCode,
      pending_clarifications: spec.clarifications_needed.map(
        (c) => `[${c.field}] ${c.question}`
      ),
    },
  };
}

// ── Validation ────────────────────────────────────────────────────────────────

function validateSpec(spec: PolicySpec) {
  if (!spec.policy_name?.trim()) throw new Error("Policy name is empty");
  if (spec.policies.length > 5)
    throw new Error(`Policy has ${spec.policies.length} layers; OZ accounts supports at most 5`);
  for (const layer of spec.policies) {
    if (layer.kind === "spending_limit" && !layer.params.cap_amount_raw) {
      throw new Error("SpendingLimit layer is missing cap_amount_raw");
    }
  }
}

// ── Templates ─────────────────────────────────────────────────────────────────

function renderCargoToml(crateName: string, description: string, protocol?: string): string {
  // Determine which protocol adapters to include based on the policy
  let protocolDeps = '';
  
  if (protocol === 'blend') {
    protocolDeps = 'blend-adapter = { path = "../protocol-adapters/blend" }\n';
  } else if (protocol === 'soroswap') {
    protocolDeps = 'soroswap-adapter = { path = "../protocol-adapters/soroswap" }\n';
  } else if (protocol === 'sep41') {
    protocolDeps = 'sep41-adapter = { path = "../protocol-adapters/sep41" }\n';
  }
  
  return `[package]
name = "${crateName}"
version = "0.1.0"
edition = "2021"
description = "${description.replace(/"/g, "'")}"
license = "MIT"

[lib]
crate-type = ["cdylib"]

[dependencies]
soroban-sdk      = { version = "=22.0.0" }
stellar-accounts = { version = "=0.7.1" }
policy-primitives = { path = "../policy-primitives" }
${protocolDeps}

[dev-dependencies]
soroban-sdk = { version = "=22.0.0", features = ["testutils"] }

[profile.release]
opt-level = "z"
overflow-checks = true
debug = false
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
`;
}

function renderComposeLib(
  structName: string,
  rationale: string,
  layers: PolicyLayer[]
): string {
  const layerList = layers
    .map((l) => `//!   - ${l.kind}: ${l.description}`)
    .join("\n");

  return `//! ${structName} — generated by OZ Policy Builder (compose mode)
//!
//! This crate documents the OZ primitive configuration.
//! Deploy each primitive contract separately; no net-new Rust is required.
//!
//! Rationale: ${rationale}
//!
//! Policy layers:
${layerList}
//!
//! ⚠  REVIEW BEFORE DEPLOYING. See REVIEW.md.

#![no_std]

// Compose mode: all constraints are expressed via existing OZ primitives.
// See REVIEW.md for the install snippets for each policy contract.
// No new Rust contract needs to be compiled.
`;
}

function renderGenerateLib(
  structName: string,
  cap: string,
  periodLedgers: number,
  _assetId: string,
  rationale: string,
  protocol?: string
): string {
  // Add protocol adapter imports if needed
  let protocolImports = '';
  let protocolUtils = '';
  
  if (protocol === 'blend') {
    protocolImports = 'use blend_adapter::{policy_utils, BlendConstraint, addresses};\n';
    protocolUtils = '    // Use Blend adapter utilities\n    // let function_name = policy_utils::extract_blend_function(&auth_context);\n';
  } else if (protocol === 'soroswap') {
    protocolImports = 'use soroswap_adapter::{policy_utils, SoroswapConstraint, addresses, slippage};\n';
    protocolUtils = '    // Use Soroswap adapter utilities\n    // let function_name = policy_utils::extract_soroswap_function(&auth_context);\n';
  } else if (protocol === 'sep41') {
    protocolImports = 'use sep41_adapter::{policy_utils, Sep41Constraint, addresses, subscription, functions};\n';
    protocolUtils = '    // Use SEP-41 adapter utilities\n    // let function_name = policy_utils::extract_sep41_function(&auth_context);\n';
  }
  
  return `//! ${structName} — generated by OZ Policy Builder (generate mode)
//!
//! Implements the OZ Policy trait with a stateful rolling spending cap.
//! Rationale: ${rationale}
//!
//! ⚠  REVIEW BEFORE DEPLOYING. See REVIEW.md.
//! Storage is scoped by (smart_account, context_rule.id) — no cross-account leakage.

#![no_std]

use soroban_sdk::{
    auth::Context, contract, contractimpl, contracttype, symbol_short, Address, Env, Val, Vec,
};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};
use policy_primitives::{PolicyStorage, PolicyError, PolicyResult, ValidateParams};
${protocolImports}

const TTL_THRESHOLD: u32 = 120_960;
const EXTEND_AMOUNT: u32 = 518_400;

#[contracttype]
#[derive(Clone)]
enum StorageKey {
    Params(Address, u32),
    State(Address, u32),
}

#[contracttype]
#[derive(Clone)]
pub struct InstallParams {
    pub spending_limit: i128,
    pub period_ledgers: u32,
}

#[contracttype]
#[derive(Clone)]
struct SpendState {
    spent: i128,
    window_start_ledger: u32,
}

#[contract]
pub struct ${structName};

impl Policy for ${structName} {
    type AccountParams = InstallParams;

    fn enforce(
        e: &Env,
        context: Context,
        _authenticated_signers: Vec<Signer>,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();
        let params: InstallParams = e
            .storage().persistent()
            .get(&StorageKey::Params(smart_account.clone(), context_rule.id))
            .unwrap_or_else(|| panic!("${structName}: not installed"));
        let mut state: SpendState = e
            .storage().persistent()
            .get(&StorageKey::State(smart_account.clone(), context_rule.id))
            .unwrap_or(SpendState { spent: 0, window_start_ledger: e.ledger().sequence() });

        let cur = e.ledger().sequence();
        if cur.saturating_sub(state.window_start_ledger) >= params.period_ledgers {
            state.spent = 0;
            state.window_start_ledger = cur;
        }
        let amount = sac_transfer_amount(&context);
        if amount > 0 {
            state.spent = state.spent.saturating_add(amount);
            if state.spent > params.spending_limit {
                panic!("${structName}: spending limit exceeded ({} > {})", state.spent, params.spending_limit);
            }
            e.storage().persistent().set(&StorageKey::State(smart_account.clone(), context_rule.id), &state);
            e.storage().persistent().extend_ttl(&StorageKey::State(smart_account, context_rule.id), TTL_THRESHOLD, EXTEND_AMOUNT);
        }
    }

    fn install(e: &Env, install_params: InstallParams, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();
        assert!(install_params.spending_limit > 0);
        assert!(install_params.period_ledgers > 0);
        
        // Use PolicyStorage from policy-primitives
        PolicyStorage::set(e, &smart_account, context_rule.id, "params", &install_params);
        PolicyStorage::set(e, &smart_account, context_rule.id, "state", &SpendState { 
            spent: 0, 
            window_start_ledger: e.ledger().sequence() 
        });
        
        e.events().publish((symbol_short!("installed"),), (smart_account, context_rule.id));
    }

    fn uninstall(e: &Env, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();
        
        // Use PolicyStorage from policy-primitives
        if !PolicyStorage::has(e, &smart_account, context_rule.id, "params") {
            panic!("${structName}: not installed");
        }
        
        PolicyStorage::set(e, &smart_account, context_rule.id, "params", &());
        PolicyStorage::set(e, &smart_account, context_rule.id, "state", &());
        
        e.events().publish((symbol_short!("uninstalled"),), (smart_account, context_rule.id));
    }
}

#[contractimpl]
impl ${structName} {}

fn sac_transfer_amount(context: &Context) -> i128 {
    match context {
        Context::Contract(ctx) if ctx.fn_name == symbol_short!("transfer") => {
            ctx.args.get(2).and_then(|v| i128::try_from(v).ok()).unwrap_or(0)
        }
        _ => 0,
    }
}

${protocolUtils}
`;
}

function renderReviewMd(vars: {
  crateName: string;
  displayName: string;
  description: string;
  rationale: string;
  contextContracts: string;
  contextFunctions: string;
  lifetimeSecs: number;
  cap: string;
  windowSecs: number;
}): string {
  return `# ${vars.displayName} — Review Checklist

> Generated by [OZ Policy Builder](https://github.com/danielfsha/oz-account-policy-builder).
> **Never deploy without completing this checklist.**

## Policy Summary

${vars.description}

## Rationale

${vars.rationale}

## Context Rule

| Field | Value |
|-------|-------|
| Contracts | \`${vars.contextContracts}\` |
| Functions | \`${vars.contextFunctions}\` |
| Lifetime | ${vars.lifetimeSecs} seconds |

## Constraints

- Spending cap: \`${vars.cap}\` raw units
- Time window: \`${vars.windowSecs}\` seconds (${Math.round(vars.windowSecs / 86400)} days)

## Pre-Deployment Checklist

- [ ] I have read all generated Rust code in \`src/lib.rs\`
- [ ] Storage keys are scoped by \`(smart_account, context_rule.id)\`
- [ ] \`uninstall()\` removes all persistent storage
- [ ] I have run \`run_harness\` and all permit + deny cases pass
- [ ] I have NOT enabled auto-deploy (this is a security property)
- [ ] (Recommended) Code has been reviewed by a qualified Soroban security engineer

## Deployment (Manual — never automatic)

\`\`\`bash
# 1. Build
stellar contract build

# 2. Deploy
stellar contract deploy \\
  --wasm target/wasm32-unknown-unknown/release/${vars.crateName.replace(/-/g, "_")}.wasm \\
  --network testnet \\
  --source YOUR_KEYPAIR

# 3. Install on context rule
stellar contract invoke \\
  --id $POLICY_CONTRACT_ID \\
  -- install \\
  --smart_account $YOUR_SMART_ACCOUNT \\
  --context_rule_id $CONTEXT_RULE_ID
\`\`\`

## Security Notes

- Generated from a single observed transaction — verify permissions are not overly broad.
- Net-new policy contracts (generate mode) require a security audit before mainnet use.
- The spending cap is derived from the observed transaction amount — adjust if needed.
`;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

export function sanitizeCrateName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\-_]/g, "-")
    .replace(/^[-_]+|[-_]+$/g, "");
}

function toPascalCase(name: string): string {
  return name
    .split(/[-_]/)
    .filter(Boolean)
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join("");
}
