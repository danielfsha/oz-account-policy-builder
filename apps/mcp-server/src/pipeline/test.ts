/**
 * Quick smoke test for the pipeline. Run with: npx tsx src/pipeline/test.ts
 */
import { synthesizePolicy } from "./synthesizer.js";
import { emitPolicyCrate } from "./emitter.js";
import { runHarness } from "./harness.js";
import { OZ_PRIMITIVES } from "./primitives.js";
import { applyConstraintOverride } from "./clarification.js";
import type { CallManifest } from "./manifest.js";

// ── Test 1: Blend yield-claim manifest ─────────────────────────────────────
const blendManifest: CallManifest = {
  transaction_hash: "abc123def456789012345678901234567890123456789012345678901234",
  network: "testnet",
  ledger_sequence: 500000,
  timestamp: "2026-01-15T12:00:00Z",
  invoking_account: "GABC1234567890123456789012345678901234567890123456789012345",
  top_level_calls: [],
  unique_contracts: [
    { id: "CBLENDPOOL123456789012345678901234567890123456789012345", label: "Blend Pool", protocol: "blend" },
  ],
  unique_functions: ["claim"],
  asset_flows: [
    {
      asset_id: "CUSDC12345678901234567890123456789012345678901234567890123",
      asset_symbol: "USDC",
      direction: "outbound",
      amount_raw: "50000000", // 5 USDC (7 decimals)
      amount_display: "5.0000000",
      decimals: 7,
      counterparty: "CBLENDPOOL123456789012345678901234567890123456789012345",
    },
  ],
  auth_boundaries: [
    {
      contract_id: "CBLENDPOOL123456789012345678901234567890123456789012345",
      function_name: "claim",
      account: "GABC1234567890123456789012345678901234567890123456789012345",
    },
  ],
  observed_amounts: {},
  summary: "Blend yield claim — 5 USDC",
  is_simulation: false,
};

console.log("═══ Test 1: Synthesize Blend yield-claim ═══");
const spec = synthesizePolicy(blendManifest, { amount_cap: BigInt("100000000") });
console.log(`  Policy name: ${spec.policy_name}`);
console.log(`  Mode: ${spec.composition_mode}`);
console.log(`  Layers: ${spec.policies.length}`);
spec.policies.forEach((p) => console.log(`    - ${p.kind}: ${p.description}`));
console.log(`  Clarifications: ${spec.clarifications_needed.length}`);
console.log("");

// ── Test 2: Run harness ─────────────────────────────────────────────────────
console.log("═══ Test 2: Run harness ═══");
const report = runHarness(spec, blendManifest);
console.log(`  Permit: ${report.permit_result.passed ? "✅" : "❌"} — ${report.permit_result.details}`);
report.deny_results.forEach((r) =>
  console.log(`  Deny [${r.case_name}]: ${r.passed ? "✅" : "❌"} — ${r.details.slice(0, 80)}`)
);
console.log(`  Overall: ${report.passed ? "✅ PASSED" : "❌ FAILED"}`);
console.log("");

// ── Test 3: Emit crate ──────────────────────────────────────────────────────
console.log("═══ Test 3: Emit policy crate ═══");
// Clear clarifications first
let cleanSpec = spec;
for (const c of spec.clarifications_needed) {
  cleanSpec = applyConstraintOverride(cleanSpec, c.field, c.options?.[0] ?? "default");
}
const emitted = emitPolicyCrate(cleanSpec);
console.log(`  Crate: ${emitted.crate_path}`);
console.log(`  Files: ${Object.keys(emitted.files).join(", ")}`);
console.log(`  Summary: ${emitted.summary.mode} mode, ${emitted.summary.layer_count} layers`);
console.log("");

// ── Test 4: List primitives ─────────────────────────────────────────────────
console.log("═══ Test 4: List primitives ═══");
OZ_PRIMITIVES.forEach((p) => console.log(`  ${p.name}: ${p.kind}`));
console.log("");

// ── Test 5: Soroswap swap manifest ──────────────────────────────────────────
console.log("═══ Test 5: Synthesize Soroswap swap ═══");
const swapManifest: CallManifest = {
  transaction_hash: "def456789012345678901234567890123456789012345678901234567890ab",
  network: "mainnet",
  ledger_sequence: 600000,
  timestamp: "2026-02-01T09:30:00Z",
  invoking_account: "GXYZ1234567890123456789012345678901234567890123456789012345",
  top_level_calls: [],
  unique_contracts: [
    { id: "CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH", label: "Soroswap Router", protocol: "soroswap" },
  ],
  unique_functions: ["swap_exact_tokens_for_tokens"],
  asset_flows: [
    {
      asset_id: "CUSDC_MAINNET",
      asset_symbol: "USDC",
      direction: "outbound",
      amount_raw: "5000000000", // 500 USDC
      amount_display: "500.0000000",
      decimals: 7,
      counterparty: "CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH",
    },
  ],
  auth_boundaries: [
    {
      contract_id: "CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH",
      function_name: "swap_exact_tokens_for_tokens",
      account: "GXYZ1234567890123456789012345678901234567890123456789012345",
    },
  ],
  observed_amounts: {},
  summary: "Soroswap swap 500 USDC → XLM",
  is_simulation: false,
};

const swapSpec = synthesizePolicy(swapManifest, { allow_slippage_percent: 3.0 });
console.log(`  Policy name: ${swapSpec.policy_name}`);
console.log(`  Mode: ${swapSpec.composition_mode}`);
console.log(`  Layers: ${swapSpec.policies.length}`);
swapSpec.policies.forEach((p) => console.log(`    - ${p.kind}: ${p.description}`));

const swapReport = runHarness(swapSpec, swapManifest);
console.log(`  Harness: ${swapReport.passed ? "✅ PASSED" : "❌ FAILED"}`);
console.log("");

console.log("═══ ALL TESTS DONE ═══");
