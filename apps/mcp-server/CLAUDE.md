# OZ Policy Builder MCP Server

Cloudflare Workers MCP server exposing the OZ Policy Builder pipeline to AI agents.

## Tools

| Tool | Description |
|------|-------------|
| `record_transaction` | Fetch a Stellar tx from Horizon → CallManifest |
| `synthesize_policy` | CallManifest → PolicySpec (context rule + policy layers) |
| `answer_clarification` | Resolve pending clarification questions in a PolicySpec |
| `run_harness` | Permit + 5 deny-case simulation tests |
| `emit_policy_crate` | PolicySpec → reviewable Rust crate files |
| `list_primitives` | List OZ policy primitives available |

## Local dev

```bash
npm install
npm run dev          # starts on http://localhost:8792
```

## Connect to Claude Desktop

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

Then restart Claude Desktop. You'll see the 6 tools available.

## Connect to Cursor / other MCP clients

MCP endpoint: `http://localhost:8792/mcp` (SSE also available at `/sse`)

## Deploy to Cloudflare

```bash
npm run deploy
```

No secrets required for the policy builder itself.

## Workflow example

```
User: I want to delegate yield claiming on Blend to an AI agent, max 100 USDC/week.

Claude:
1. record_transaction("abc123...", "mainnet")
2. synthesize_policy(manifest_json, amount_cap="1000000000", time_window_seconds=604800)
3. answer_clarification(spec_json, field="contract_lock", answer="Lock to exact address")
4. run_harness(spec_json, manifest_json)  → all 5 deny cases pass ✅
5. emit_policy_crate(spec_json)  → Rust crate + REVIEW.md
```
