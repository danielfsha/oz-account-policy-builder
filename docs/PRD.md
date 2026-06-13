# OZ Accounts Policy Builder — Product Requirements Document

**Version:** 1.0
**Date:** 2026-06-13
**Status:** Draft — Active Development

---

## 1. Problem Statement

OpenZeppelin's smart accounts framework for Stellar decomposes authorization into context rules, signers, and policies. Writing a custom policy today requires authoring a Soroban contract that correctly implements the `Policy` trait, handles storage segregation by `(smart_account, context_rule.id)`, manages the install/enforce/uninstall lifecycle, and passes security review. This bar is too high for application developers and prohibitive for end users who want to delegate a narrow capability to an agent.

## 2. Solution

An AI-assisted "record and generate" toolkit: a user executes a representative transaction, and the tool synthesizes the exact context rule + policy set to allow repeating that flow — and nothing else. The tool generates reviewable Rust code; deployment is always a separate, explicit human step.

## 3. Target Users

| Persona | Use case |
|---------|----------|
| **DeFi user** | Delegate weekly yield claims to an AI agent with a 100 USDC/week cap |
| **dApp developer** | Generate subscription billing policies for their service |
| **Treasury admin** | Set up bounded trading permissions for an agent on Soroswap |
| **Wallet integrator** | Embed policy generation into their smart wallet UI |

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                          apps/                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  mcp-server  │  │  agent-skill │  │  wallet (superpeach) │  │
│  │  (stdio/CF)  │  │  (Claude)    │  │  (passkey smart acct)│  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
│         │                  │                      │              │
│         └──────────┬───────┘──────────────────────┘              │
│                    ▼                                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │          packages/policy-synthesizer                     │    │
│  │  recorder → analyzer → synthesizer → codegen → harness  │    │
│  └──────────────────────────┬──────────────────────────────┘    │
│                             │                                    │
│  ┌──────────────────────────▼──────────────────────────────┐    │
│  │          packages/stellar-tx-parser                      │    │
│  │  TransactionMeta XDR → InvokeHostFunction → events      │    │
│  └──────────────────────────┬──────────────────────────────┘    │
│                             │                                    │
│  ┌──────────────────────────▼──────────────────────────────┐    │
│  │       packages/oz-accounts-client                        │    │
│  │  add_context_rule() / add_policy() / install()           │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       contracts/ (Soroban/Rust)                   │
│  ┌──────────────┐  ┌─────────────────────────────────────────┐  │
│  │policy-primitives│ │ policies/ (spending-limit, time-window, │  │
│  │(Policy trait)   │ │  frequency-limit, allowlist, composite) │  │
│  └──────────────┘  └─────────────────────────────────────────┘  │
│  ┌──────────────────────┐  ┌─────────────────────────────────┐  │
│  │ protocol-adapters/   │  │ simulation-harness/              │  │
│  │ (blend, soroswap,    │  │ (on-chain permit/deny testing)   │  │
│  │  sep41 interfaces)   │  │                                  │  │
│  └──────────────────────┘  └─────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## 5. Key Dependencies (What to Use, Not Rewrite)

| Dependency | Source | What we use |
|------------|--------|-------------|
| `stellar-accounts` | `crates.io =0.7.1` (OpenZeppelin) | `Policy` trait, `ContextRule`, `Signer`, `SmartAccount` types |
| `blend-contract-sdk` | `crates.io` (blend-capital) | Pool/Backstop client + WASM for test fixtures |
| `soroswap-core` | github.com/soroswap/core | Router interface (`swap_exact_tokens_for_tokens`), Factory, Pair |
| `soroswap-aggregator` | github.com/soroswap/aggregator | Aggregator adapter interface for multi-DEX routing |
| `superpeach` (kalepail) | github.com/kalepail/superpeach | Passkey smart wallet — fork as wallet app |
| `@stellar/stellar-sdk` | npm | XDR decode, transaction simulation, Horizon API |
| `passkey-kit` | npm (kalepail) | WebAuthn credential management for smart accounts |

## 6. Deliverables

### 6.1 MCP Server (`apps/mcp-server/`)
- **Transport:** stdio (local) + Cloudflare Workers (remote)
- **Tools:** `record_transaction`, `synthesize_policy`, `answer_clarification`, `run_harness`, `emit_policy_crate`, `list_primitives`, `install_policy`
- **Resources:** policy-templates, protocol-registry (known contract addresses)
- **Error codes:** machine-readable, deterministic

### 6.2 Agent Skill (`apps/agent-skill/`)
- System prompt defining the conversational workflow
- Intent classifier: "user wants to delegate X"
- Clarifier: asks about caps, windows, recipients when ambiguous
- 3 built-in walkthroughs: Blend yield, SEP-41 subscription, Soroswap bounded swap

### 6.3 Smart Wallet (`apps/wallet/`)
- Fork of `kalepail/superpeach` — passkey-powered smart wallet
- Extended with policy management UI:
  - `/policies/new` — record → generate → simulate flow
  - `/policies` — list active policies
  - `/policies/[id]` — review, manage, uninstall
- Calls MCP server for synthesis, shows generated code for review

### 6.4 Policy Contracts (`contracts/`)
- `policy-primitives/` — OZ `Policy` trait re-export + storage helpers
- `policies/spending-limit/` — wraps OZ `spending_limit` with our install params
- `policies/time-window/` — once-per-period enforcement
- `policies/frequency-limit/` — max N calls per window
- `policies/allowlist/` — contract + function whitelist
- `policies/composite/` — AND-chains up to 5 policies
- `protocol-adapters/blend/` — Blend pool/backstop client interfaces (from `blend-contract-sdk`)
- `protocol-adapters/soroswap/` — Soroswap router interface (from `soroswap-core`)
- `protocol-adapters/sep41/` — Generic SEP-41 token interface
- `simulation-harness/` — on-chain permit/deny test runner

### 6.5 TypeScript Packages (`packages/`)
- `policy-synthesizer/` — core pipeline: recorder → analyzer → synthesizer → codegen → harness
- `stellar-tx-parser/` — decode `TransactionMeta` XDR, extract auth entries + events
- `oz-accounts-client/` — TS client for `add_context_rule`, `add_policy`, `install`
- `protocol-clients/` — typed wrappers for Blend, Soroswap, SEP-41

### 6.6 Documentation (`apps/docs/`)
- Quickstart guide
- Synthesizer decision logic explained
- Extending with new policy primitives
- MCP tool reference
- 3 end-to-end walkthroughs:
  1. Blend yield-claim (weekly, 100 USDC cap)
  2. SEP-41 subscription billing (monthly, fixed recipient)
  3. Soroswap bounded swap (daily, 5% slippage guard)

## 7. Non-Goals

- **Auto-deployment** — never. Code is always reviewed first.
- **Hosted service** — this is a developer tool / MCP server, not a SaaS.
- **Custom wallet from scratch** — we fork superpeach.
- **Rewriting OZ primitives** — we compose them, adding only what's missing.
- **Mainnet deployment in v1** — testnet-first, mainnet after audit.

## 8. Protocol Contract Addresses (Testnet)

| Protocol | Contract | Address |
|----------|----------|---------|
| Soroswap Factory | `SoroswapFactory` | See `soroswap-core/public/testnet.contracts.json` |
| Soroswap Router | `SoroswapRouter` | See `soroswap-core/public/testnet.contracts.json` |
| Blend Pool Factory | `PoolFactory` | See Blend docs |
| USDC (testnet SAC) | SAC | `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA` |

## 9. Security Model

1. **Storage segregation** — all policy state keyed by `(smart_account_address, context_rule_id)`
2. **No cross-account leakage** — one policy contract can serve many accounts safely
3. **Minimal permissions** — synthesizer biases toward the tightest possible constraints
4. **Deny-case testing** — harness generates 5 standard mutations that must all fail
5. **No auto-deploy** — tool produces code, human deploys
6. **Audit scope** — synthesizer logic + codegen templates + all policy contracts

## 10. Milestones

| # | Milestone | Duration | Status |
|---|-----------|----------|--------|
| 1 | Core pipeline (synthesizer + harness + codegen) | 2 weeks | ✅ Done |
| 2 | MCP server (stdio + 6 tools working end-to-end) | 1 week | 🔧 In progress |
| 3 | Policy contracts (5 policies + adapters + tests) | 2 weeks | 🔜 Next |
| 4 | Wallet integration (superpeach fork + policy UI) | 2 weeks | — |
| 5 | Agent skill + 3 walkthroughs | 1 week | — |
| 6 | XDR decode + real Horizon tx recording | 1 week | — |
| 7 | Security audit prep + remediation | 2 weeks | — |
| 8 | Production release (testnet) | 1 week | — |

## 11. Technology Stack

| Layer | Technology |
|-------|-----------|
| Smart contracts | Rust, Soroban SDK 22, `stellar-accounts` 0.7.1 |
| Synthesizer (TS) | TypeScript, `@stellar/stellar-sdk`, Zod |
| MCP server | Node.js stdio + Cloudflare Workers (Durable Objects) |
| Agent skill | Claude MCP SDK, system prompt engineering |
| Wallet | Astro + Svelte (forked from superpeach) |
| Wallet auth | WebAuthn passkeys via `passkey-kit` |
| CI | GitHub Actions — `cargo test`, `pnpm test`, deploy on tag |
| Package manager | pnpm workspaces + Cargo workspace |

## 12. Open Questions

1. **OZ reviewer cadence** — how often do we share design decisions with OZ team?
2. **Upstream primitives** — which new primitives (frequency-limit, allowlist) should we propose upstreaming to `stellar-accounts`?
3. **Wallet cohort coordination** — which C-Address Tooling wallets should we integrate with beyond superpeach?
4. **Audit firm** — who audits the synthesizer + generated policy templates?

---

*This is a living document. Updated as implementation progresses.*
