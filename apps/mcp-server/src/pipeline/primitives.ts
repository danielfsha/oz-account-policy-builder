/**
 * Known OpenZeppelin account policy primitives (stellar-accounts 0.7.1).
 */

export interface OzPrimitive {
  name: string;
  kind: string;
  description: string;
  install_params: string;
  crate: string;
  docs: string;
}

export const OZ_PRIMITIVES: OzPrimitive[] = [
  {
    name: "SpendingLimit",
    kind: "spending_limit",
    description:
      "Caps outbound token transfers per rolling time window. Uses stellar-accounts::policies::spending_limit. Params: spending_limit (i128, raw units), period_ledgers (u32).",
    install_params: "SpendingLimitAccountParams { spending_limit: i128, period_ledgers: u32 }",
    crate: "stellar-accounts",
    docs: "https://docs.rs/stellar-accounts/latest/stellar_accounts/policies/spending_limit",
  },
  {
    name: "SimpleThreshold",
    kind: "simple_threshold",
    description:
      "Requires a minimum number of equal-weight signers (M-of-N multisig). Params: threshold (u32).",
    install_params: "SimpleThresholdAccountParams { threshold: u32 }",
    crate: "stellar-accounts",
    docs: "https://docs.rs/stellar-accounts/latest/stellar_accounts/policies/simple_threshold",
  },
  {
    name: "WeightedThreshold",
    kind: "weighted_threshold",
    description:
      "Weighted multisig — different signers have different voting weights; total weight must meet threshold. Params: signer_weights (Vec<(Address, u64)>), threshold (u64).",
    install_params:
      "WeightedThresholdInstallParams { signer_weights: Vec<(Address, u64)>, threshold: u64 }",
    crate: "stellar-accounts",
    docs: "https://docs.rs/stellar-accounts/latest/stellar_accounts/policies/weighted_threshold",
  },
  {
    name: "TimeWindow (Custom)",
    kind: "time_window",
    description:
      "Restricts invocations to a configurable rolling window. Not a standalone OZ primitive — implemented as a custom policy layer that tracks last_invocation_ledger.",
    install_params: "Custom: period_ledgers: u32",
    crate: "custom (generated)",
    docs: "https://docs.openzeppelin.com/stellar-contracts/accounts/policies",
  },
  {
    name: "SlippageGuard (Custom)",
    kind: "custom",
    description:
      "Swap slippage guard — rejects swaps where amount_out_min/amount_in falls below (1 - max_slippage_bps/10_000). Net-new codegen required; no OZ primitive exists.",
    install_params: "Custom: max_slippage_bps: u32",
    crate: "custom (generated)",
    docs: "https://docs.openzeppelin.com/stellar-contracts/accounts/policies",
  },
];
