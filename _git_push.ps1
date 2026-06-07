Set-Location "C:\Users\ROG\Desktop\oz-account-policy-builder"

# Stage everything
git add --all

# Show what's staged
Write-Host "=== Staged files ===" -ForegroundColor Cyan
git diff --cached --name-only

# Commit
git commit -m "feat: real OZ Policy trait contracts + codegen emitter + gitignore

- Fix .gitignore: add Rust target/, *.wasm, wrangler, generated/
- Update root Cargo.toml: pin stellar-accounts =0.7.1 as workspace dep
- Fix decision_tree.rs: borrow checker error in policy count truncation
- Add codegen/templates.rs: Soroban policy Rust + REVIEW.md templates
- Add codegen/emitter.rs: emit_policy_crate() with full validation
- contracts/blend_yield_claim: real Policy trait impl using stellar-accounts 0.7.1
  - Rolling spending cap scoped by (smart_account, context_rule.id)
  - SAC transfer amount extraction from soroban_sdk::auth::Context
- contracts/soroswap_bounded_swap: real Policy trait impl
  - Slippage guard (bps) + daily volume cap with rolling window
  - swap_exact_tokens_for_tokens arg extraction
- contracts/sep41_subscription: real Policy trait impl
  - Fixed recipient, amount cap, once-per-period enforcement
  - Proper install/uninstall with TTL management"

# Push
git push origin main

Write-Host "=== Done ===" -ForegroundColor Green
git log --oneline -4
