# OZ Account Policy Builder — Technical Architecture

> AI-assisted toolkit for crafting OpenZeppelin smart account policies from observed Stellar transactions.
> Record → Synthesize → Emit → Harness → Install

---

## Table of Contents

1. [System Overview](#system-overview)
2. [High-Level Architecture](#high-level-architecture)
3. [Monorepo Structure](#monorepo-structure)
4. [Pipeline Architecture](#pipeline-architecture)
5. [MCP Server Architecture](#mcp-server-architecture)
6. [Contract Layer Architecture](#contract-layer-architecture)
7. [Wallet Integration](#wallet-integration)
8. [Data Flow & Sequence Diagrams](#data-flow--sequence-diagrams)
9. [Security Architecture](#security-architecture)
10. [Deployment Architecture](#deployment-architecture)
11. [Technology Stack](#technology-stack)
12. [Extension Points](#extension-points)

---

## System Overview

The OZ Policy Builder is a developer/end-user toolkit that synthesizes OpenZeppelin smart account policies from observed or simulated Stellar transactions. It implements a "record-and-generate" workflow where:

1. A user executes a representative transaction (e.g., claiming yield on Blend)
2. The tool analyzes contract calls, auth entries, and asset flows
3. A policy synthesizer produces a context rule + minimum policy set
4. The output is reviewable Rust code — deployment is always explicit and manual

The system is **not** a hosted service that auto-deploys policies. It generates code; the user deploys.

```mermaid
graph LR
    A[User / Agent] -->|tx hash or simulation| B[MCP Server]
    B -->|fetch| C[Horizon API]
    B -->|analyze| D[Pipeline]
    D -->|synthesize| E[PolicySpec]
    E -->|emit| F[Rust Crate]
    E -->|test| G[Harness]
    F -->|review & deploy| H[Soroban Network]
    
    style A fill:#e1f5fe
    style B fill:#fff3e0
    style D fill:#f3e5f5
    style F fill:#e8f5e9
    style H fill:#fce4ec
```

---

## High-Level Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        CL1[Claude Desktop / Agent]
        CL2[MCP Inspector]
        CL3[Wallet UI - SuperPeach]
        CL4[Custom MCP Client]
    end

    subgraph "Transport Layer"
        T1[stdio transport<br/>Local Claude Desktop]
        T2[HTTP /mcp endpoint<br/>Streamable HTTP]
        T3[HTTP /sse endpoint<br/>Legacy SSE]
    end

    subgraph "Auth Layer"
        A1[GitHub OAuth Provider]
        A2[Cookie-based Session]
        A3[Role-based Access Control]
    end

    subgraph "MCP Server - Cloudflare Worker"
        S1[McpAgent Durable Object]
        S2[Tool Registry]
        S3[6 MCP Tools]
    end

    subgraph "Pipeline Engine"
        P1[Recorder]
        P2[Manifest Builder]
        P3[Synthesizer]
        P4[Emitter]
        P5[Harness]
        P6[Clarification Resolver]
    end

    subgraph "Contract Layer - Rust/Soroban"
        R1[policy-primitives crate]
        R2[oz-policy-core crate]
        R3[Pre-built Policies<br/>spending-limit, time-window,<br/>frequency-limit, allowlist, composite]
        R4[Protocol Adapters<br/>Blend, Soroswap, SEP-41]
        R5[Simulation Harness]
    end

    subgraph "External Services"
        E1[Horizon API<br/>mainnet / testnet / futurenet]
        E2[Soroban RPC]
        E3[Stellar Network]
    end

    CL1 --> T1
    CL2 --> T2
    CL3 --> T2
    CL4 --> T3

    T1 --> S1
    T2 --> A1 --> S1
    T3 --> A1 --> S1

    S1 --> S2 --> S3
    S3 --> P1
    S3 --> P3
    S3 --> P4
    S3 --> P5
    S3 --> P6

    P1 --> E1
    P2 --> P3
    P3 --> P4
    P4 --> R1
    P5 --> R5

    R3 --> E3
    R4 --> E3
```

---

## Monorepo Structure

```
oz-account-policy-builder/
├── apps/
│   ├── mcp-server/          # Cloudflare Worker MCP server (TypeScript)
│   │   ├── src/
│   │   │   ├── index.ts           # HTTP Worker entry (Durable Object)
│   │   │   ├── stdio.ts           # Local stdio transport for Claude Desktop
│   │   │   ├── types.ts           # Zod schemas, response helpers
│   │   │   ├── auth/              # GitHub OAuth flow
│   │   │   ├── pipeline/          # Core logic (TS port of Rust core)
│   │   │   │   ├── recorder.ts    # Fetch tx from Horizon
│   │   │   │   ├── manifest.ts    # Build CallManifest
│   │   │   │   ├── synthesizer.ts # Decision tree → PolicySpec
│   │   │   │   ├── emitter.ts     # PolicySpec → Rust crate
│   │   │   │   ├── harness.ts     # Permit/deny simulation
│   │   │   │   ├── primitives.ts  # OZ primitive registry
│   │   │   │   └── clarification.ts # Resolve ambiguities
│   │   │   └── tools/             # MCP tool handlers
│   │   │       ├── register-tools.ts
│   │   │       └── policy-tools.ts   # 6 policy pipeline tools
│   │   ├── tests/
│   │   └── contracts/             # Embedded policy-primitives source
│   │
│   ├── wallet/              # SuperPeach — passkey smart wallet (Astro/Svelte)
│   └── docs/                # Next.js documentation site
│
├── contracts/               # Rust Soroban contracts (Cargo workspace)
│   ├── core/                # Transaction analyzer + synthesizer (Rust)
│   ├── policy-primitives/   # Base traits, storage helpers
│   ├── policies/            # Pre-built policy contracts
│   │   ├── spending-limit/
│   │   ├── time-window/
│   │   ├── frequency-limit/
│   │   ├── allowlist/
│   │   └── composite/
│   ├── protocol-adapters/   # DeFi protocol integrations
│   │   ├── blend/
│   │   ├── soroswap/
│   │   └── sep41/
│   └── simulation-harness/  # On-chain permit/deny test runner
│
├── packages/                # Shared TS packages (eslint, tsconfig, UI)
├── Cargo.toml               # Rust workspace root
├── turbo.json               # Turborepo task runner
└── pnpm-workspace.yaml      # PNPM workspace definition
```

---

## Pipeline Architecture

The pipeline implements a 6-stage decision flow. Each stage is stateless and deterministic.

```mermaid
flowchart TD
    START([User provides tx_hash + network]) --> FETCH

    subgraph "Stage 1: Record"
        FETCH[Fetch from Horizon API] --> CHECK{Transaction<br/>successful?}
        CHECK -->|No| ERR1[Return actionable error:<br/>- Check hash/network<br/>- Try other network<br/>- Use successful tx]
        CHECK -->|Yes| PARSE[Parse envelope_xdr +<br/>result_meta_xdr]
    end

    subgraph "Stage 2: Manifest"
        PARSE --> MANIFEST[Build CallManifest]
        MANIFEST --> EXTRACT[Extract:<br/>• Contract calls<br/>• Auth boundaries<br/>• Asset flows<br/>• Function signatures]
    end

    subgraph "Stage 3: Synthesize"
        EXTRACT --> DT[Decision Tree]
        DT --> S1[Count contracts > 3?]
        S1 --> S2[Outbound flows → spending_limit]
        S2 --> S3[Time patterns → time_window]
        S3 --> S4[Counterparties → threshold type]
        S4 --> S5[Swap detection → slippage guard]
        S5 --> S6[Cap at 5 layers]
        S6 --> CTX[Build context_rule]
        CTX --> SPEC[PolicySpec + Clarifications]
    end

    subgraph "Stage 4: Clarify (if needed)"
        SPEC --> CLARIFY{Clarifications<br/>pending?}
        CLARIFY -->|Yes| ASK[Ask user via agent:<br/>- Amount cap<br/>- Time window<br/>- Composition mode]
        ASK --> APPLY[Apply overrides]
        APPLY --> CLARIFY
        CLARIFY -->|No| READY[Spec ready]
    end

    subgraph "Stage 5: Harness"
        READY --> PERMIT[Run PERMIT case:<br/>original tx must pass]
        PERMIT --> DENY[Run 5 DENY mutations:<br/>• amount × 3<br/>• wrong asset<br/>• wrong contract<br/>• extra function<br/>• out of window]
        DENY --> RESULT{All pass?}
        RESULT -->|No| TIGHTEN[Re-synthesize with<br/>tighter constraints]
        TIGHTEN --> DT
        RESULT -->|Yes| EMIT_READY[Safe to emit]
    end

    subgraph "Stage 6: Emit"
        EMIT_READY --> MODE{Composition<br/>mode?}
        MODE -->|compose| COMPOSE[Configure existing<br/>OZ primitives only]
        MODE -->|generate| GENERATE[Generate Rust crate:<br/>• Cargo.toml<br/>• src/lib.rs<br/>• REVIEW.md<br/>• .gitignore]
        COMPOSE --> OUTPUT[Output: file contents<br/>+ REVIEW.md checklist]
        GENERATE --> OUTPUT
    end

    OUTPUT --> DEPLOY([User reviews + deploys manually])

    style ERR1 fill:#ffcdd2
    style DEPLOY fill:#c8e6c9
    style SPEC fill:#e1bee7
```

### Pipeline Data Types

```mermaid
classDiagram
    class RawTransaction {
        +string hash
        +Network network
        +number ledger
        +string created_at
        +string source_account
        +boolean successful
        +string envelope_xdr
        +string result_meta_xdr
    }

    class CallManifest {
        +string transaction_hash
        +string network
        +number ledger_sequence
        +string timestamp
        +string invoking_account
        +CallNode[] top_level_calls
        +ContractRef[] unique_contracts
        +string[] unique_functions
        +AssetFlow[] asset_flows
        +AuthBoundary[] auth_boundaries
        +Record observed_amounts
        +string summary
        +boolean is_simulation
    }

    class PolicySpec {
        +string policy_name
        +ContextRule context_rule
        +PolicyLayer[] policies
        +CompositionMode composition_mode
        +string rationale
        +Clarification[] clarifications_needed
    }

    class ContextRule {
        +string[] contracts
        +string[] functions
        +number lifetime_seconds
    }

    class PolicyLayer {
        +PolicyLayerKind kind
        +Record params
        +boolean oz_primitive
        +string description
    }

    class EmittedCrate {
        +string crate_path
        +Record~string,string~ files
        +string review_md
        +CrateSummary summary
    }

    class HarnessReport {
        +HarnessResult permit_result
        +HarnessResult[] deny_results
        +boolean passed
        +string report
    }

    RawTransaction --> CallManifest : buildCallManifest()
    CallManifest --> PolicySpec : synthesizePolicy()
    PolicySpec --> EmittedCrate : emitPolicyCrate()
    PolicySpec --> HarnessReport : runHarness()
    PolicySpec --> PolicySpec : applyConstraintOverride()
    ContextRule --* PolicySpec
    PolicyLayer --* PolicySpec
```

---

## MCP Server Architecture

### Dual Transport Design

The MCP server supports two deployment modes:

| Mode | Transport | Use Case | Auth |
|------|-----------|----------|------|
| **Local (stdio)** | stdin/stdout | Claude Desktop, local dev | None needed |
| **Remote (HTTP)** | Cloudflare Worker + Durable Object | Multi-user, deployed | GitHub OAuth |

```mermaid
graph TB
    subgraph "Local Mode (stdio.ts)"
        LC[Claude Desktop] <-->|stdin/stdout| LS[McpServer<br/>StdioServerTransport]
        LS --> LP[Pipeline Functions]
    end

    subgraph "Remote Mode (index.ts)"
        RC[Remote Client] -->|HTTPS| CF[Cloudflare Worker]
        CF -->|OAuth| GH[GitHub]
        CF -->|Route| DO[Durable Object<br/>MyMCP extends McpAgent]
        DO --> RP[Pipeline Functions]
        DO -->|props| AUTH[User Identity:<br/>login, email, accessToken]
    end

    LP --> HORIZON[Horizon API]
    RP --> HORIZON
```

### Tool Inventory

| # | Tool | Input | Output | Stateless |
|---|------|-------|--------|-----------|
| 1 | `record_transaction` | tx_hash, network, invoking_account? | CallManifest | ✅ |
| 2 | `synthesize_policy` | manifest_json, constraints? | PolicySpec + Clarifications | ✅ |
| 3 | `emit_policy_crate` | spec_json, output_dir? | EmittedCrate (files as strings) | ✅ |
| 4 | `run_harness` | spec_json, manifest_json | HarnessReport (permit + 5 deny) | ✅ |
| 5 | `list_primitives` | (none) | OZ primitive catalog | ✅ |
| 6 | `answer_clarification` | spec_json, field, answer | Updated PolicySpec | ✅ |

All tools are deterministic and side-effect-free. No tool deploys anything.

### Error Handling Strategy

```mermaid
flowchart LR
    subgraph "Error Categories"
        E1[Network Error<br/>Horizon unreachable]
        E2[Not Found<br/>404 from Horizon]
        E3[Failed Tx<br/>successful=false]
        E4[Parse Error<br/>Invalid XDR/JSON]
        E5[Validation Error<br/>Spec constraints violated]
    end

    subgraph "Response Strategy"
        R1[Retry with backoff]
        R2[Suggest: check hash,<br/>try other network]
        R3[Structured error with<br/>suggestions + alternatives]
        R4[Detail which field failed]
        R5[List specific violations]
    end

    E1 --> R1
    E2 --> R2
    E3 --> R3
    E4 --> R4
    E5 --> R5
```

---

## Contract Layer Architecture

### Rust Workspace

```mermaid
graph TB
    subgraph "contracts/ — Cargo Workspace"
        CORE[oz-policy-core<br/>Transaction analyzer<br/>+ synthesizer in Rust]
        PRIMS[policy-primitives<br/>Base traits, PolicyStorage,<br/>PolicyError, ValidateParams]
        
        subgraph "Pre-built Policies"
            SL[spending-limit]
            TW[time-window]
            FL[frequency-limit]
            AL[allowlist]
            CP[composite]
        end

        subgraph "Protocol Adapters"
            BL[blend adapter]
            SS[soroswap adapter]
            S41[sep41 adapter]
        end

        HARNESS[simulation-harness<br/>On-chain permit/deny runner]
    end

    subgraph "External Dependencies"
        SDK[soroban-sdk 26.1.0]
        XDR[stellar-xdr 27.0.0]
        OZ[stellar-accounts 0.7.2<br/>OZ Policy trait]
    end

    CORE --> SDK
    CORE --> XDR
    PRIMS --> SDK
    PRIMS --> OZ
    SL --> PRIMS
    TW --> PRIMS
    FL --> PRIMS
    AL --> PRIMS
    CP --> PRIMS
    BL --> PRIMS
    SS --> PRIMS
    S41 --> PRIMS
    HARNESS --> PRIMS
    HARNESS --> OZ
```

### Policy Trait (from stellar-accounts)

```rust
pub trait Policy {
    type AccountParams;

    fn install(e: &Env, params: Self::AccountParams, rule: ContextRule, account: Address);
    fn enforce(e: &Env, ctx: Context, signers: Vec<Signer>, rule: ContextRule, account: Address);
    fn uninstall(e: &Env, rule: ContextRule, account: Address);
}
```

### Storage Segregation Pattern

Every policy MUST scope its storage by `(smart_account, context_rule.id)` to prevent cross-account data leakage:

```mermaid
graph LR
    subgraph "Storage Keys"
        K1["Params(AccountA, Rule1)"]
        K2["State(AccountA, Rule1)"]
        K3["Params(AccountB, Rule1)"]
        K4["State(AccountB, Rule1)"]
    end

    subgraph "Persistent Storage"
        S[Soroban Persistent<br/>Storage]
    end

    K1 --> S
    K2 --> S
    K3 --> S
    K4 --> S
```

### Composition Modes

```mermaid
flowchart TD
    INPUT[PolicySpec] --> DECIDE{Can all constraints<br/>be expressed by<br/>OZ primitives?}
    
    DECIDE -->|Yes| COMPOSE[Compose Mode]
    DECIDE -->|No| GENERATE[Generate Mode]
    
    subgraph "Compose Mode"
        COMPOSE --> C1[Configure spending_limit contract]
        COMPOSE --> C2[Configure simple_threshold contract]
        COMPOSE --> C3[No new Rust code needed]
        C1 --> C4[Output: install snippets +<br/>contract addresses]
    end

    subgraph "Generate Mode"
        GENERATE --> G1[Render Cargo.toml]
        GENERATE --> G2[Render src/lib.rs<br/>implements Policy trait]
        GENERATE --> G3[Render REVIEW.md<br/>pre-deploy checklist]
        G1 --> G4[Output: complete compilable<br/>Rust Soroban crate]
    end
```

---

## Wallet Integration

### SuperPeach (Passkey Smart Wallet)

The wallet serves as the reference integration for the end-to-end flow: record → generate → simulate → sign → install.

```mermaid
sequenceDiagram
    participant U as User
    participant W as SuperPeach Wallet
    participant MCP as MCP Server
    participant H as Horizon
    participant S as Soroban

    U->>W: "Delegate yield claiming to agent"
    W->>W: User performs sample tx<br/>(claim yield on Blend)
    W->>H: Transaction submitted
    H-->>W: tx_hash returned
    
    W->>MCP: record_transaction(tx_hash, "mainnet")
    MCP->>H: Fetch transaction
    H-->>MCP: RawTransaction
    MCP-->>W: CallManifest

    W->>MCP: synthesize_policy(manifest)
    MCP-->>W: PolicySpec + Clarifications

    W->>U: "Cap at 50 USDC or allow headroom?"
    U->>W: "100 USDC per week"
    W->>MCP: answer_clarification(spec, "amount_cap", "1000000000")
    MCP-->>W: Updated PolicySpec

    W->>MCP: run_harness(spec, manifest)
    MCP-->>W: HarnessReport (all pass ✅)

    W->>MCP: emit_policy_crate(spec)
    MCP-->>W: EmittedCrate (files)

    W->>U: Show REVIEW.md + generated code
    U->>W: Approve deployment
    W->>S: stellar contract deploy --wasm policy.wasm
    S-->>W: policy_contract_id
    W->>S: Install policy on smart account
    S-->>W: Context rule active ✅

    Note over U,S: Agent can now operate<br/>under this policy
```

### Wallet Tech Stack

| Component | Technology | Role |
|-----------|-----------|------|
| Framework | Astro + Svelte | SSR + reactive UI |
| Auth | Passkeys (WebAuthn) | Passwordless signing |
| Stellar SDK | `@stellar/stellar-sdk` | Transaction building |
| Smart Account | `passkey-kit` | OZ smart account management |
| Styling | Tailwind CSS | UI components |
| Deployment | Cloudflare Pages | Static hosting |

---

## Data Flow & Sequence Diagrams

### Complete Record-to-Install Flow

```mermaid
sequenceDiagram
    participant Agent as AI Agent
    participant MCP as MCP Server
    participant Horizon as Stellar Horizon
    participant Synth as Synthesizer
    participant Harness as Harness Engine
    participant Emitter as Code Emitter
    participant User as User

    Agent->>MCP: record_transaction("abc123...", "mainnet")
    MCP->>Horizon: GET /transactions/abc123...
    
    alt Transaction not found
        Horizon-->>MCP: 404
        MCP-->>Agent: Error: not found, suggest check hash/network
    else Transaction failed
        Horizon-->>MCP: {successful: false}
        MCP-->>Agent: Error: failed on-chain + suggestions
    else Transaction successful
        Horizon-->>MCP: {successful: true, envelope_xdr, result_meta_xdr}
        MCP->>MCP: buildCallManifest(raw)
        MCP-->>Agent: CallManifest JSON
    end

    Agent->>MCP: synthesize_policy(manifest_json, constraints)
    MCP->>Synth: Decision tree evaluation
    Synth-->>MCP: PolicySpec
    MCP-->>Agent: PolicySpec + clarifications

    opt Clarifications needed
        Agent->>User: "Should I cap at 50 or 100 USDC?"
        User-->>Agent: "100 USDC per week"
        Agent->>MCP: answer_clarification(spec, "amount_cap", "1000000000")
        MCP-->>Agent: Updated PolicySpec
    end

    Agent->>MCP: run_harness(spec_json, manifest_json)
    MCP->>Harness: Evaluate permit + 5 deny mutations
    Harness-->>MCP: HarnessReport
    MCP-->>Agent: {passed: true/false, details}

    alt Harness failed
        Agent->>MCP: synthesize_policy(manifest, tighter_constraints)
        Note right of Agent: Loop until harness passes
    end

    Agent->>MCP: emit_policy_crate(spec_json)
    MCP->>Emitter: Generate Rust files
    Emitter-->>MCP: EmittedCrate
    MCP-->>Agent: {files, review_md, summary}

    Agent->>User: "Here's the generated policy. Review REVIEW.md before deploying."
    Note over User: Manual review + deploy
```

### Synthesizer Decision Tree

```mermaid
flowchart TD
    M[CallManifest] --> S1{unique_contracts > 3?}
    
    S1 -->|Yes| CL1[Clarification: compose vs generate?]
    S1 -->|No| S2

    S2{Outbound asset flows?}
    S2 -->|Yes| SPL[Add: spending_limit layer<br/>cap = observed amount<br/>window = inferred]
    S2 -->|Yes + no cap specified| CL2[Clarification: exact cap or headroom?]
    S2 -->|No| S3

    SPL --> S3{Auth boundaries present?}
    S3 -->|Yes + no window specified| CL3[Clarification: daily/weekly/monthly?]
    S3 --> TW[Add: time_window layer]

    TW --> S4{Single contract + function?}
    S4 -->|Yes| ST[Add: simple_threshold<br/>threshold=1]
    S4 -->|No| WT[Add: weighted_threshold]

    ST --> S5
    WT --> S5{Function contains "swap"?}
    S5 -->|Yes| SG[Add: custom slippage_guard<br/>mark as generate mode]
    S5 -->|No| S6

    SG --> S6{Total layers > 5?}
    S6 -->|Yes| TRIM[Merge time_window into<br/>spending_limit.window_seconds]
    S6 -->|No| CTX[Build context_rule:<br/>contracts, functions, lifetime]

    TRIM --> CTX
    CTX --> OUTPUT[PolicySpec complete]
```

---

## Security Architecture

### Threat Model

```mermaid
graph TB
    subgraph "Threats"
        T1[Overly permissive policy<br/>allows unauthorized actions]
        T2[Policy code injection<br/>via crafted manifest]
        T3[Cross-account storage leakage]
        T4[Auto-deploy without review]
        T5[Stolen agent credentials]
    end

    subgraph "Mitigations"
        M1[Harness: 5 deny mutations<br/>must all fail]
        M2[Input sanitization +<br/>template-based codegen]
        M3[Storage scoped by<br/>account + rule ID]
        M4[No deploy tool exists<br/>code-first, deploy-second]
        M5[Context rules are narrow:<br/>specific contracts + functions +<br/>time-bounded lifetime]
    end

    T1 --> M1
    T2 --> M2
    T3 --> M3
    T4 --> M4
    T5 --> M5
```

### Security Properties

| Property | Implementation |
|----------|---------------|
| Minimal permission | Synthesizer only permits observed contracts/functions |
| Spending caps | Derived from observed amounts (with optional headroom) |
| Time-bounded | Context rules have explicit `lifetime_seconds` |
| No auto-deploy | `emit_policy_crate` returns file strings, never deploys |
| Deny-case validation | Harness tests 5 mutations; all must be rejected |
| Storage isolation | All state scoped by `(smart_account, context_rule.id)` |
| Review gate | REVIEW.md checklist is always generated and shown |
| Audit trail | Transaction hash recorded in manifest for provenance |

### Authentication Flow (Remote Mode)

```mermaid
sequenceDiagram
    participant Client as MCP Client
    participant Worker as CF Worker
    participant OAuth as OAuth Provider (KV)
    participant GH as GitHub

    Client->>Worker: POST /mcp (no token)
    Worker-->>Client: 401 → redirect to /authorize

    Client->>Worker: GET /authorize
    Worker->>Worker: Check approval cookie
    alt Not approved
        Worker-->>Client: Render approval dialog
        Client->>Worker: POST /authorize (approve)
    end
    Worker-->>Client: 302 → GitHub OAuth
    
    Client->>GH: Authorize
    GH-->>Client: 302 + code → /callback
    
    Client->>Worker: GET /callback?code=xxx
    Worker->>GH: Exchange code for token
    GH-->>Worker: access_token
    Worker->>GH: GET /user
    GH-->>Worker: {login, name, email}
    Worker->>OAuth: Store token + props
    Worker-->>Client: 302 → client redirect_uri + MCP token

    Client->>Worker: POST /mcp (with token)
    Worker->>OAuth: Validate token → extract props
    Worker->>Worker: Durable Object handles request<br/>with user props available
    Worker-->>Client: MCP response
```

---

## Deployment Architecture

```mermaid
graph TB
    subgraph "Development"
        DEV1[stdio.ts<br/>npx tsx src/stdio.ts]
        DEV2[wrangler dev<br/>localhost:8789]
        DEV3[MCP Inspector]
    end

    subgraph "Cloudflare Edge"
        CF1[Worker<br/>Global distribution]
        CF2[Durable Object<br/>MyMCP per session]
        CF3[KV Namespace<br/>OAuth tokens]
    end

    subgraph "Wallet Hosting"
        W1[Cloudflare Pages<br/>SuperPeach wallet]
    end

    subgraph "External"
        EXT1[Horizon API<br/>horizon.stellar.org<br/>horizon-testnet.stellar.org]
        EXT2[GitHub OAuth]
        EXT3[Soroban RPC<br/>soroban-testnet.stellar.org]
    end

    DEV1 -->|local only| EXT1
    DEV2 -->|local tunnel| EXT1
    DEV3 --> DEV2

    CF1 --> CF2
    CF2 --> CF3
    CF1 --> EXT1
    CF1 --> EXT2
    W1 --> CF1
    W1 --> EXT3
```

### Environment Configuration

| Environment | Entry Point | Auth | Horizon | Deploy Method |
|-------------|------------|------|---------|---------------|
| Local (Claude) | `src/stdio.ts` | None | testnet | `npx tsx` |
| Local (HTTP) | `wrangler dev` | GitHub OAuth (localhost) | testnet | wrangler |
| Production | `wrangler deploy` | GitHub OAuth (workers.dev) | mainnet + testnet | wrangler |

---

## Technology Stack

### TypeScript (MCP Server + Pipeline)

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Runtime | Cloudflare Workers | Latest | Edge compute, global distribution |
| MCP SDK | `@modelcontextprotocol/sdk` | 1.13.1 | MCP protocol implementation |
| Agent Framework | `agents` (Cloudflare) | 0.0.100 | Durable Object MCP agent |
| HTTP Framework | Hono | 4.8.3 | OAuth route handling |
| Validation | Zod | 3.25.67 | Input schema validation |
| OAuth | `@cloudflare/workers-oauth-provider` | 0.0.5 | GitHub OAuth flow |
| GitHub API | Octokit | 5.0.3 | User identity verification |
| Testing | Vitest | 3.2.4 | Unit tests |
| Build | Wrangler | 4.23.0 | Worker bundling + deploy |
| Monorepo | Turborepo | 2.9.6 | Task orchestration |
| Package Manager | pnpm | 9.0.0 | Dependency management |

### Rust (Contract Layer)

| Crate | Version | Purpose |
|-------|---------|---------|
| soroban-sdk | 26.1.0 | Soroban smart contract SDK |
| stellar-xdr | 27.0.0 | XDR encoding/decoding |
| stellar-accounts | 0.7.2 | OZ Policy trait + smart account framework |
| serde / serde_json | 1.0 | Serialization (for core library) |
| proptest | 1.4 | Property-based testing |

### Wallet (SuperPeach)

| Technology | Purpose |
|-----------|---------|
| Astro | Static site generation + SSR |
| Svelte | Reactive UI components |
| Tailwind CSS | Styling |
| `@stellar/stellar-sdk` | Stellar transaction building |
| `passkey-kit` | WebAuthn-based smart account management |
| Cloudflare Pages | Hosting |

---

## Extension Points

### Adding a New Policy Primitive

1. Create `contracts/policies/my-primitive/` with `Cargo.toml` + `src/lib.rs`
2. Implement the `Policy` trait from `stellar-accounts`
3. Add to `contracts/policy-primitives/` if it's a reusable building block
4. Register in `apps/mcp-server/src/pipeline/primitives.ts` (`OZ_PRIMITIVES` array)
5. Add detection logic in `synthesizer.ts` decision tree
6. Add template rendering in `emitter.ts`

### Adding a New Protocol Adapter

1. Create `contracts/protocol-adapters/my-protocol/`
2. Implement protocol-specific address resolution, function detection, and constraint extraction
3. Add contract ID to `PROTOCOL_REGISTRY` in `manifest.ts`
4. Add adapter import path to `emitter.ts` `renderCargoToml()` / `renderGenerateLib()`

### Adding a New MCP Tool

1. Define Zod schema in `src/types.ts`
2. Implement handler in `src/tools/policy-tools.ts`
3. Register in `registerPolicyTools()` via `server.tool()`
4. Add equivalent in `src/stdio.ts` for local mode

### Adding a New Harness Mutation

1. Add mutation definition in `harness.ts` `mutations` array:
   ```ts
   { name: "my_mutation", description: "...", fn: (m) => { /* mutate manifest */ } }
   ```
2. The harness automatically runs all mutations and expects DENY for each

---

## Design Decisions & Rationale

| Decision | Rationale |
|----------|-----------|
| TypeScript pipeline mirrors Rust core | Cloudflare Workers cannot run WASM (Soroban) directly; TS port enables serverless execution without compilation |
| No auto-deploy | Security property: generated code must be reviewed. Deployment is always explicit. |
| Stateless tools | Enables horizontal scaling, deterministic testing, and idempotent retries |
| 5-layer cap | OZ smart accounts enforce max 5 policies per context rule |
| Clarification loop | Synthesizer asks rather than guesses when parameters are ambiguous |
| Dual transport (stdio + HTTP) | Covers both local Claude Desktop and multi-user remote deployment |
| Composition-first | Uses existing audited OZ primitives before generating new code |
| Harness runs before emit | Prevents shipping overly permissive policies |
| Template-based codegen | Avoids injection vectors; generated code follows known-good patterns |
| GitHub OAuth for remote | Leverages existing developer identity; maps to RBAC for write access |

---

## Audit Surface

Components requiring security audit:

1. **Synthesizer decision tree** — ensures minimal permissions are generated
2. **Emitter templates** — generated Rust must correctly implement Policy trait
3. **Harness evaluator** — deny cases must accurately model real policy enforcement
4. **Storage segregation** — all generated code must scope by `(account, rule_id)`
5. **Pre-built policy contracts** — spending-limit, time-window, etc.
6. **Policy-primitives crate** — base traits used by all generated/composed policies

Components NOT requiring audit (non-security-critical):

- MCP transport layer (uses well-tested SDK)
- OAuth flow (delegates to GitHub)
- Wallet UI (no policy logic)
- Documentation site
