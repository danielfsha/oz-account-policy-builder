/**
 * Apply a user's answer to a pending clarification in a PolicySpec.
 */

import type { PolicySpec } from "./synthesizer";

export function applyConstraintOverride(
  spec: PolicySpec,
  field: string,
  answer: string
): PolicySpec {
  const updated = JSON.parse(JSON.stringify(spec)) as PolicySpec;

  switch (field) {
    case "amount_cap": {
      const cap = answer.replace(/[^0-9]/g, "");
      for (const layer of updated.policies) {
        if (layer.kind === "spending_limit") {
          layer.params.cap_amount_raw = cap;
          layer.description = layer.description.replace(/\d+\.\d+/, formatRaw(cap, layer.params.decimals ?? 7));
        }
      }
      break;
    }
    case "time_window_seconds": {
      const secs = parseInt(answer);
      if (!isNaN(secs)) {
        for (const layer of updated.policies) {
          if (layer.kind === "time_window" || layer.kind === "spending_limit") {
            layer.params.window_seconds = secs;
          }
        }
      }
      break;
    }
    case "lifetime_seconds": {
      const secs = parseInt(answer);
      if (!isNaN(secs)) updated.context_rule.lifetime_seconds = secs;
      break;
    }
    case "contract_lock": {
      // User chose to lock to exact contract — already in context_rule, no change needed
      break;
    }
    case "contract_complexity": {
      if (answer.toLowerCase().includes("custom") || answer.includes("1")) {
        updated.composition_mode = "generate";
      } else {
        updated.composition_mode = "compose";
      }
      break;
    }
    default:
      // Unknown field — still remove the clarification so we don't loop
      break;
  }

  // Remove the answered clarification
  updated.clarifications_needed = updated.clarifications_needed.filter(
    (c) => c.field !== field
  );

  return updated;
}

function formatRaw(raw: string, decimals: number): string {
  try {
    const n = BigInt(raw);
    const divisor = BigInt(10 ** decimals);
    return `${n / divisor}.${(n % divisor).toString().padStart(decimals, "0")}`;
  } catch {
    return raw;
  }
}
