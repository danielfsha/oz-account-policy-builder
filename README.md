# OpenZeppelin Account Policy Builder with MCP Server

A comprehensive policy ecosystem for OpenZeppelin smart accounts with Model Context Protocol (MCP) server integration. This project enables AI agents to synthesize, test, and deploy smart account policies for Stellar blockchain.

## Project Structure

```
├── contracts/                    # Rust smart contracts
│   ├── policy-primitives/       # Base traits and storage utilities
│   ├── policies/               # Policy implementations
│   │   ├── spending-limit/     # Spending limit policy
│   │   ├── time-window/        # Time window policy
│   │   ├── frequency-limit/    # Frequency limit policy
│   │   ├── allowlist/          # Allowlist policy
│   │   └── composite/          # Composite policy
│   ├── protocol-adapters/      # Protocol-specific adapters
│   │   ├── blend-adapter/      # Blend protocol adapter
│   │   ├── soroswap-adapter/   # Soroswap protocol adapter
│   │   └── sep41-adapter/      # SEP-41 token adapter
│   ├── simulation-harness/     # On-chain testing harness
│   └── core/                   # Policy synthesis engine
├── apps/
│   ├── mcp-server/             # MCP server for AI agent interaction
│   ├── docs/                   # Documentation website
│   └── wallet/                 # Wallet interface
└── packages/                   # Shared packages
```

## Core Components

### 1. Policy Contracts
- **Spending Limit Policy**: Enforces maximum spending amounts over rolling windows
- **Time Window Policy**: Restricts operations to specific time periods
- **Frequency Limit Policy**: Limits the number of operations per time period
- **Allowlist Policy**: Whitelists specific addresses or operations
- **Composite Policy**: Combines multiple policies with logical operators (AND/OR)

### 2. Protocol Adapters
- **Blend Adapter**: Integrates with Blend protocol for lending operations
- **Soroswap Adapter**: Supports Soroswap DEX operations
- **SEP-41 Adapter**: Handles SEP-41 token standards

### 3. MCP Server Tools
- `record_transaction`: Fetch Stellar transactions from Horizon → CallManifest
- `synthesize_policy`: CallManifest → PolicySpec (context rule + policy layers)
- `answer_clarification`: Resolve pending clarification questions
- `run_harness`: Permit + 5 deny-case simulation tests
- `emit_policy_crate`: PolicySpec → reviewable Rust crate files
- `list_primitives`: List available OZ policy primitives

## Getting Started

### Prerequisites
- Rust (latest stable)
- Node.js 18+
- pnpm or npm
- Soroban CLI

### Installation

```bash
# Clone the repository
git clone https://github.com/danielfsha/oz-account-policy-builder.git
cd oz-account-policy-builder

# Install dependencies
pnpm install

# Build contracts
cargo build --workspace

# Start MCP server
cd apps/mcp-server
npm run dev
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Test specific policy
cargo test -p spending-limit

# Test MCP server integration
cd apps/mcp-server
npm test
```

### Development

#### Policy Development
```bash
# Create new policy contract
cargo new --lib contracts/policies/new-policy

# Add to workspace Cargo.toml
# Implement Policy trait from stellar_accounts crate
# Use policy-primitives for storage utilities
```

#### MCP Server Development
```bash
cd apps/mcp-server
npm run dev          # starts on http://localhost:8792
```

## Connecting to MCP Clients

### Claude Desktop
Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "oz-policy-builder": {
      "command": "npx",
      "args": ["mcp-remote", "http://localhost:8792/mcp"]
    }
  }
}
```

### Cursor / Other MCP Clients
- MCP endpoint: `http://localhost:8792/mcp`
- SSE endpoint: `http://localhost:8792/sse`

## Deployment

### Smart Contracts
```bash
# Build WASM for deployment
cargo build --workspace --target wasm32-unknown-unknown --release

# Deploy to testnet
soroban contract deploy --wasm target/wasm32-unknown-unknown/release/spending_limit.wasm
```

### MCP Server to Cloudflare
```bash
cd apps/mcp-server
npm run deploy
```

## Usage Examples

### 1. Creating a Spending Limit Policy via MCP
```python
# AI Agent workflow example
1. record_transaction("tx_hash", "testnet")
2. synthesize_policy(manifest, amount_cap="1000000000", time_window="604800")
3. answer_clarification(spec, field="asset_lock", answer="USDC")
4. run_harness(spec, manifest)  # All 5 deny cases pass ✅
5. emit_policy_crate(spec)  # Generates Rust crate + REVIEW.md
```

### 2. Direct Contract Usage
```rust
use spending_limit::{SpendingLimitPolicy, InstallParams};

// Install spending limit policy
let params = InstallParams {
    cap_amount: 1000,
    asset_id: asset_address,
    window_ledgers: 1000,
    allow_partial: true,
};

SpendingLimitPolicy::install(env, account, context_rule_id, params)?;
```

### 3. Protocol Integration
```rust
use blend_adapter::BlendProtocolAdapter;

// Use blend adapter for lending operations
let adapter = BlendProtocolAdapter::new(env, blend_contract_address);
let result = adapter.execute_lend(account, amount, asset);
```

## Architecture

### Storage Scoping
All policies use scoped storage by `(smart_account, context_rule_id)`:
- `PolicyStorageKey::State(account, rule_id, subkey)`
- `PolicyStorageKey::Params(account, rule_id, subkey)`

### Error Handling
- `PolicyError` enum for consistent error types
- `PolicyResult<T>` alias for `Result<T, PolicyError>`
- Automatic conversion from `soroban_sdk::Error`

### Testing Strategy
- Unit tests for each policy contract
- Integration tests with simulation harness
- 5 deny-case tests for each policy (exhaustion, timing, overflow, etc.)
- Protocol adapter integration tests

## Contributing

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure all tests pass: `cargo test --workspace`
5. Submit a pull request

## License

MIT License - see LICENSE file for details

## Resources

- [OpenZeppelin Smart Accounts Documentation](https://docs.openzeppelin.com/contracts/stellar)
- [Stellar Documentation](https://developers.stellar.org/)
- [Soroban Documentation](https://soroban.stellar.org/)
- [Model Context Protocol](https://modelcontextprotocol.io/)


## Quick Start

### For Policy Developers
```bash
# Build and test all policies
cargo build --workspace
cargo test --workspace

# Create and test a new policy
cargo new --lib contracts/policies/my-policy
cd contracts/policies/my-policy
cargo test
```

### For MCP Server Users
```bash
# Start the MCP server
cd apps/mcp-server
npm install
npm run dev

# In another terminal, test the endpoints
curl http://localhost:8792/health
```

### For AI Agent Integration
```python
# Example AI agent workflow
1. Analyze transaction requirements
2. Use synthesize_policy to generate policy specification
3. Test with run_harness (5 deny cases)
4. Deploy with emit_policy_crate
```

## Current Status

✅ **Implemented**
- 5 core policy contracts with storage scoping
- Protocol adapters for Blend, Soroswap, SEP-41
- Policy primitives base crate
- MCP server with 6 core tools
- Comprehensive test suites

🚧 **In Progress**
- Fixing test compilation issues with storage context
- Implementing From<soroban_sdk::Error> for PolicyError
- Updating test syntax to use associated functions

📋 **Planned**
- Integration with blend-contract-sdk
- Advanced simulation harness features
- Additional protocol adapters
- Frontend wallet interface