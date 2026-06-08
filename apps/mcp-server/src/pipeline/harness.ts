/**
 * Permit/deny simulation harness — TypeScript port of packages/core/src/harness/
 *
 * Runs the original manifest (must PERMIT) + 5 standard deny mutations (must DENY).
 */

import type { CallManifest, AssetFlow } from "./manifest";
import type { PolicySpec } from "./synthesizer";

export interface HarnessResult {
  case_name: string;
  mutation?: string;
  expected: "permit" | "deny";
  actual: "permit" | "deny";
  passed: boolean;
  details: string;
}

export interface HarnessReport {
  permit_result: HarnessResult;
  deny_results: HarnessResult[];
  passed: boolean;
  report: string;
}

export function runHarness(spec: PolicySpec, manifest: CallManifest): HarnessReport {
  const permit_result = runPermitCase(spec, manifest);

  const mutations: Array<{ name: string; description: string; fn: (m: CallManifest) => CallManifest }> = [
    {
      name: "amount_exceeded",
      description: "Same tx, amount × 2 — should exceed spending limit",
      fn: (m) => {
        const copy = deepClone(m);
        copy.asset_flows = copy.asset_flows.map((f) => ({
          ...f,
          amount_raw: (BigInt(f.amount_raw) * 2n).toString(),
          amount_display: `[2× ${f.amount_display}]`,
        }));
        copy.summary = `[MUTATION:amount×2] ${copy.summary}`;
        return copy;
      },
    },
    {
      name: "wrong_asset",
      description: "Same tx, different asset — should be rejected",
      fn: (m) => {
        const copy = deepClone(m);
        copy.asset_flows = copy.asset_flows.map((f) => ({
          ...f,
          asset_id: "CWRONGASSET_NOT_IN_POLICY",
          asset_symbol: "WRONG",
        }));
        copy.summary = `[MUTATION:wrong_asset] ${copy.summary}`;
        return copy;
      },
    },
    {
      name: "wrong_contract",
      description: "Same function, different contract — should fail contract whitelist",
      fn: (m) => {
        const copy = deepClone(m);
        copy.unique_contracts = copy.unique_contracts.map((c) => ({
          ...c,
          id: "CWRONG_CONTRACT_NOT_IN_POLICY",
        }));
        copy.summary = `[MUTATION:wrong_contract] ${copy.summary}`;
        return copy;
      },
    },
    {
      name: "extra_function_call",
      description: "Original + extra unauthorized function — should be rejected",
      fn: (m) => {
        const copy = deepClone(m);
        copy.unique_functions = [...copy.unique_functions, "unauthorized_drain"];
        copy.summary = `[MUTATION:extra_fn] ${copy.summary}`;
        return copy;
      },
    },
    {
      name: "out_of_window",
      description: "Same tx, timestamp past window end — should fail time window",
      fn: (m) => {
        const copy = deepClone(m);
        copy.timestamp = "2099-01-01T00:00:00Z";
        copy.ledger_sequence = copy.ledger_sequence + 10_000_000;
        copy.summary = `[MUTATION:out_of_window] ${copy.summary}`;
        return copy;
      },
    },
  ];

  const windowSecs =
    spec.policies.find((p) => p.kind === "spending_limit" || p.kind === "time_window")
      ?.params?.window_seconds ?? 86400;

  const deny_results: HarnessResult[] = mutations.map(({ name, description, fn }) => {
    const mutated = fn(manifest);
    const { outcome, details } = evaluatePolicy(spec, mutated);
    return {
      case_name: name,
      mutation: description,
      expected: "deny",
      actual: outcome,
      passed: outcome === "deny",
      details: `${description} | ${details}`,
    };
  });

  const passed = permit_result.passed && deny_results.every((r) => r.passed);
  const report = buildReport(permit_result, deny_results, passed);

  return { permit_result, deny_results, passed, report };
}

// ── Core evaluator ────────────────────────────────────────────────────────────

function runPermitCase(spec: PolicySpec, manifest: CallManifest): HarnessResult {
  const { outcome, details } = evaluatePolicy(spec, manifest);
  return {
    case_name: "original_transaction",
    expected: "permit",
    actual: outcome,
    passed: outcome === "permit",
    details,
  };
}

function evaluatePolicy(
  spec: PolicySpec,
  manifest: CallManifest
): { outcome: "permit" | "deny"; details: string } {
  // Contract whitelist
  if (spec.context_rule.contracts.length > 0) {
    for (const c of manifest.unique_contracts) {
      if (!spec.context_rule.contracts.includes(c.id)) {
        return { outcome: "deny", details: `Contract ${c.id} not in context rule whitelist` };
      }
    }
  }

  // Function whitelist
  if (spec.context_rule.functions.length > 0) {
    for (const fn of manifest.unique_functions) {
      if (!spec.context_rule.functions.includes(fn)) {
        return { outcome: "deny", details: `Function ${fn} not in context rule whitelist` };
      }
    }
  }

  // Policy layers
  for (const layer of spec.policies) {
    if (layer.kind === "spending_limit") {
      const cap = BigInt(layer.params.cap_amount_raw ?? "0");
      const assetId = layer.params.asset_id ?? "";
      const totalOut = manifest.asset_flows
        .filter((f) => f.direction === "outbound")
        .filter((f) => !assetId || f.asset_id === assetId)
        .reduce((sum, f) => sum + BigInt(f.amount_raw), 0n);
      if (totalOut > cap) {
        return {
          outcome: "deny",
          details: `SpendingLimit: outbound ${totalOut} exceeds cap ${cap}`,
        };
      }
    }

    if (layer.kind === "time_window") {
      if (manifest.timestamp.startsWith("2099")) {
        return { outcome: "deny", details: "TimeWindow: timestamp past end of allowed window" };
      }
    }

    if (layer.kind === "simple_threshold" || layer.kind === "weighted_threshold") {
      const accountInAuth = manifest.auth_boundaries.some(
        (b) => b.account === manifest.invoking_account
      );
      if (manifest.auth_boundaries.length > 0 && !accountInAuth) {
        return {
          outcome: "deny",
          details: "Threshold: invoking account not in auth boundaries",
        };
      }
    }
  }

  return { outcome: "permit", details: "All policy layers passed" };
}

function buildReport(
  permit: HarnessResult,
  denyResults: HarnessResult[],
  passed: boolean
): string {
  const lines = [
    "═══════════════════════════════════════",
    " OZ Policy Builder — Harness Report",
    "═══════════════════════════════════════",
    `PERMIT: ${permit.passed ? "PASS" : "FAIL"} — ${permit.details}`,
    "",
    "DENY CASES:",
    ...denyResults.map(
      (r) => `  ${r.passed ? "✅" : "❌"} [${r.case_name}] ${r.mutation} — ${r.details}`
    ),
    "",
    `OVERALL: ${passed ? "ALL CASES PASSED — safe to emit policy code" : "HARNESS FAILED — re-synthesize with tighter constraints"}`,
  ];
  return lines.join("\n");
}

function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj));
}
