Set-Location "C:\Users\ROG\Desktop\oz-account-policy-builder"
git add -A
git commit -m "fix(mcp): worker.fetch is not a function — use McpAgent.serve() directly as default export"
git push origin main
Write-Host "pushed" -ForegroundColor Green
