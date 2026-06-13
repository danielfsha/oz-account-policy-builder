Set-Location "C:\Users\ROG\Desktop\oz-account-policy-builder"
git add -A
git commit -m "fix(mcp): add stdio transport, fix HTTP routing, remove OAuth

- Add src/stdio.ts: standalone Node stdio MCP server for Claude Desktop
  (no wrangler needed, no mcp-remote needed)
- Fix index.ts: proper fetch routing for /mcp, /sse, health check, and
  reject .well-known/oauth-* probes with 404
- Remove OAuth/GitHub auth from Worker (not needed for policy builder)
- Clean package.json: rename, add tsx dep, add 'stdio' script
- Fix arg types in stdio.ts for strict mode
- Tested: 'npx tsx src/stdio.ts' starts cleanly"
git push origin main
Write-Host "pushed" -ForegroundColor Green
