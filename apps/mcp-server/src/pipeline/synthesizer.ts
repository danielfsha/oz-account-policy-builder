/**
 * Policy synthesizer — TypeScript port of packages/core/src/synthesizer/decision_tree.rs
 *
 * Decision order (matches Rust):
 *   1. Count unique contracts → flag if > 3
 *   2. Asset flows → spending_limit
 *   3. Time patterns → time_window
 *   4. Single vs multiple counterparties → simple_threshold vs weighted_threshold
 *   5. Swap detection → custom slippage layer
 *   6. Validate: total policies ≤ 5
 *   7. Build context_rule
 */

import type { CallManifest } from "./manifest";

export type PolicyLayerKind =
  | "spending_limit"
  | "time_window"
  | "simple_threshold"
  | "weighted_threshold"
  | "custom";

export type CompositionMode = "compose" | "generate";

export interface PolicyLayer {
  kind: PolicyLayerKind;
  params: Record<string, any>;
  oz_primitive: boolean;
  description: string;
}

export interface ContextRule {
  contracts: string[];
  functions: string[];
  lifetime_seconds: number;
}

export interface Clarification {
  field: string;
  question: string;
  options?: string[];
}

export interface PolicySpec {
  policy_name: string;
  context_rule: ContextRule;
  policies: PolicyLayer[];
  composition_mode: CompositionMode;
  rationale: string;
  clarifications_needed: Clarification[];
}

export interface Constraints {
  amount_cap?: bigint;
  time_window_seconds?: number;
  lifetime_seconds?: number;
  allow_slippage_percent?: number;
  max_calls_per_window?: number;
}

const ONE_DAY = 86_400;
const ONE_WEEK = 7 * ONE_DAY;
const ONE_MONTH = 30 * ONE_DAY;
const ONE_YEAR = 365 * ONE_DAY;

export function synthesizePolicy(manifest: CallManifest, constraints: Constraints = {}): PolicySpec {
  const policies: PolicyLayer[] = [];
  const clarifications: Clarification[] = [];
  let needsCustom = false;

  // ── Step 1: Contract count ────────────────────────────────────────────────
  if (manifest.unique_contracts.length > 3) {
    clarifications.push({
      field: "contract_complexity",
      question: `This transaction involves ${manifest.unique_contracts.length} contracts. Should I generate a custom policy or compose primitives?`,
      options: ["Compose primitives (simpler)", "Generate custom policy (more precise)"],
    });
    needsCustom = true;
  }

  // ── Step 2: Spending limit ────────────────────────────────────────────────
  const outbound = manifest.asset_flows.filter((f) => f.direction === "outbound");
  if (outbound.length > 0) {
    const flow = outbound[0];
    const observedCap = BigInt(flow.amount_raw);
    const cap = constraints.amount_cap ?? observedCap;
    const windowSecs = constraints.time_window_seconds ?? inferWindow(manifest);
    const windowLabel = formatWindowLabel(windowSecs);
    const symbol = flow.asset_symbol ?? flow.asset_id.slice(0, 8);

    if (!constraints.amount_cap) {
      clarifications.push({
        field: "amount_cap",
        question: `This transaction sent ${flow.amount_display} ${symbol}. Cap at exactly ${flow.amount_display} ${symbol}, or allow headroom (e.g. ${formatWithHeadroom(observedCap, flow.decimals, 1.2)} ${symbol} per ${windowLabel})?`,
        options: [
          `Exact: ${flow.amount_display} ${symbol}`,
          `With 20% headroom: ${formatWithHeadroom(observedCap, flow.decimals, 1.2)} ${symbol} per ${windowLabel}`,
        ],
      });
    }

    policies.push({
      kind: "spending_limit",
      params: {
        cap_amount_raw: cap.toString(),
        asset_id: flow.asset_id,
        asset_symbol: symbol,
        window_seconds: windowSecs,
        decimals: flow.decimals,
      },
      oz_primitive: true,
      description: `Limit outbound ${symbol} to ${formatAmount(cap, flow.decimals)} per ${windowLabel}`,
    });
  }

  // ── Step 3: Time window ───────────────────────────────────────────────────
  const windowSecs = constraints.time_window_seconds ?? inferWindow(manifest);
  const windowLabel = formatWindowLabel(windowSecs);

  if (!constraints.time_window_seconds && manifest.auth_boundaries.length > 0) {
    clarifications.push({
      field: "time_window_seconds",
      question: "How often should this operation be allowed?",
      options: ["Daily (86400s)", "Weekly (604800s)", "Monthly (2592000s)"],
    });
  }

  policies.push({
    kind: "time_window",
    params: { window_seconds: windowSecs, window_label: windowLabel },
    oz_primitive: true,
    description: `Restrict invocation to once per ${windowLabel}`,
  });

  // ── Step 4: Threshold ─────────────────────────────────────────────────────
  if (manifest.unique_contracts.length <= 1 && manifest.unique_functions.length <= 1) {
    policies.push({
      kind: "simple_threshold",
      params: { threshold: 1 },
      oz_primitive: true,
      description: "Single authorized signer sufficient",
    });
  } else {
    policies.push({
      kind: "weighted_threshold",
      params: { threshold: 1, total_signers: 1 },
      oz_primitive: true,
      description: "Weighted threshold for multi-contract interaction",
    });
  }

  // ── Step 5: Slippage (swap) ───────────────────────────────────────────────
  const isSwap = manifest.unique_functions.some(
    (f) => f.includes("swap") || f.includes("exchange")
  );
  if (isSwap && !constraints.allow_slippage_percent) {
    needsCustom = true;
    const slippage = constraints.allow_slippage_percent ?? 5.0;
    policies.push({
      kind: "custom",
      params: {
        max_slippage_percent: slippage,
        reason: "slippage_check cannot be expressed by existing OZ primitives — net-new codegen required",
      },
      oz_primitive: false,
      description: `Slippage guard: reject swap if slippage > ${slippage}%`,
    });
  }

  // ── Step 6: Cap at 5 ─────────────────────────────────────────────────────
  if (policies.length > 5) {
    const hasSpendingWithWindow = policies.some(
      (p) => p.kind === "spending_limit" && p.params.window_seconds != null
    );
    const filtered = hasSpendingWithWindow
      ? policies.filter((p) => p.kind !== "time_window")
      : policies;
    policies.splice(0, policies.length, ...filtered.slice(0, 5));
  }

  // ── Step 7: Context rule ──────────────────────────────────────────────────
  const context_rule: ContextRule = {
    contracts: manifest.unique_contracts.map((c) => c.id),
    functions: manifest.unique_functions,
    lifetime_seconds: constraints.lifetime_seconds ?? ONE_YEAR,
  };

  const composition_mode: CompositionMode =
    needsCustom || policies.some((p) => !p.oz_primitive) ? "generate" : "compose";

  const rationale = buildRationale(policies, manifest, composition_mode === "generate");
  const policy_name = buildPolicyName(manifest);

  return {
    policy_name,
    context_rule,
    policies,
    composition_mode,
    rationale,
    clarifications_needed: clarifications,
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function inferWindow(manifest: CallManifest): number {
  const fns = manifest.unique_functions.map((f) => f.toLowerCase());
  if (fns.some((f) => f.includes("subscri"))) return ONE_MONTH;
  if (fns.some((f) => f.includes("claim") || f.includes("yield"))) return ONE_WEEK;
  return ONE_DAY;
}

function formatWindowLabel(secs: number): string {
  if (secs <= ONE_DAY) return "day";
  if (secs <= ONE_WEEK) return "week";
  if (secs <= ONE_MONTH) return "month";
  return "year";
}

function formatAmount(raw: bigint, decimals: number): string {
  const divisor = BigInt(10 ** decimals);
  const whole = raw / divisor;
  const frac = raw % divisor;
  return `${whole}.${frac.toString().padStart(decimals, "0")}`;
}

function formatWithHeadroom(raw: bigint, decimals: number, multiplier: number): string {
  const withHeadroom = BigInt(Math.round(Number(raw) * multiplier));
  return formatAmount(withHeadroom, decimals);
}

function buildRationale(layers: PolicyLayer[], manifest: CallManifest, isCustom: boolean): string {
  const parts = [`Policy layers: ${layers.map((l) => l.kind).join(" + ")}`];
  if (manifest.asset_flows.some((f) => f.direction === "outbound")) {
    parts.push("spending_limit added for outbound asset flow");
  }
  if (manifest.auth_boundaries.length > 0) {
    parts.push("time_window added to restrict invocation frequency");
  }
  if (isCustom) {
    parts.push("custom layer required — constraint cannot be expressed by existing OZ primitives");
  }
  return parts.join(". ");
}

function buildPolicyName(manifest: CallManifest): string {
  const fn = manifest.unique_functions[0];
  if (fn) return fn.replace(/_/g, "-");
  const c = manifest.unique_contracts[0];
  if (c?.label) return c.label.toLowerCase().replace(/\s+/g, "-");
  return "custom-policy";
}
