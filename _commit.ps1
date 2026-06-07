Set-Location "C:\Users\ROG\Desktop\oz-account-policy-builder"
git add -A
git commit -m "feat(mcp): complete OZ Policy Builder MCP server

- Remove GitHub OAuth / PostgreSQL — not needed for policy builder
- Replace database tools with 6 policy pipeline tools:
  record_transaction, synthesize_policy, answer_clarification,
  run_harness, emit_policy_crate, list_primitives
- Add TypeScript pipeline (synthesizer, emitter, harness, recorder,
  manifest, clarification) matching the Rust core logic
- Simplify index.ts: plain McpAgent.serve('/mcp') — no OAuth wrapper
- Clean wrangler.jsonc: only Durable Object binding, no KV/DB/secrets
- Clean worker-configuration.d.ts: only MCP_OBJECT + NODE_ENV
- Clean .dev.vars / .dev.vars.example: no secrets needed for local dev
- Add CLAUDE.md: how to connect to Claude Desktop / Cursor
- Fix types.ts: add index signature to McpTextContent/McpResponse
  so they match MCP SDK CallToolResult shape
- sep41_subscription: add get_state() public accessor"
git push origin main
Write-Host "pushed" -ForegroundColor Green
git log --oneline -5
