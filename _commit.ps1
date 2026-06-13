Set-Location "C:\Users\ROG\Desktop\oz-account-policy-builder"
git add -A
git commit -m "feat: clone soroswap-core, soroswap-aggregator, blend-contract-sdk, superpeach

- contracts/soroswap-core: router, factory, pair, library (Soroban AMM)
- contracts/soroswap-aggregator: adapter interface + multi-DEX aggregator
- contracts/blend-contract-sdk: pool/backstop interfaces + WASMs
- apps/wallet: superpeach passkey smart wallet (to extend with policy UI)
- Removed: scripts, node deps, CI, audit PDFs, deploy tooling (not needed)
- Kept: Rust contract source, Cargo.toml, WASMs, interfaces, licenses"
git push origin main
