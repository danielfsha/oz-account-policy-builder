//! # Blend Yield-Claim Policy
//!
//! Implements [`stellar_accounts::policies::Policy`] to allow a delegated
//! agent to claim yield from a Blend lending pool, with a rolling spending cap.
//!
//! ## Storage scoping
//! Every `(smart_account, context_rule.id)` pair gets its own isolated storage
//! bucket — no cross-account data leakage.
//!
//! ## Rolling window
//! Matches OZ `spending_limit` semantics: ledgers older than `period_ledgers`
//! are evicted before evaluating new transfers.
//!
//! ## ⚠  REVIEW BEFORE DEPLOYING — deployment is never automatic.

#![no_std]

use soroban_sdk::{
    auth::Context, contract, contractimpl, contracttype, symbol_short, Address, Env, Val, Vec, TryIntoVal,
};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};

// ── TTL ───────────────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 120_960; // ~7 days in ledgers
const EXTEND_AMOUNT: u32 = 518_400; // ~30 days in ledgers

// ── Storage keys ──────────────────────────────────────────────────────────────
#[contracttype]
#[derive(Clone)]
enum StorageKey {
    /// Install params: limit + period — (smart_account, rule_id)
    Params(Address, u32),
    /// Rolling window state — (smart_account, rule_id)
    State(Address, u32),
}

// ── Data types ────────────────────────────────────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BlendYieldClaimParams {
    /// Max outbound amount per rolling window (raw token units, 7 decimals).
    /// e.g. 1_000_000_000 = 100 USDC
    pub spending_limit: i128,
    /// Window length in ledgers (~5 s each). 120_960 ≈ 1 week.
    pub period_ledgers: u32,
}

#[contracttype]
#[derive(Clone)]
struct SpendState {
    spent: i128,
    window_start_ledger: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────────
#[contract]
pub struct BlendYieldClaimPolicy;

impl Policy for BlendYieldClaimPolicy {
    type AccountParams = BlendYieldClaimParams;

    fn enforce(
        e: &Env,
        context: Context,
        _authenticated_signers: Vec<Signer>,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        let params: BlendYieldClaimParams = e
            .storage()
            .persistent()
            .get(&params_key)
            .unwrap_or_else(|| panic!("BlendYieldClaimPolicy: not installed"));

        let mut state: SpendState = e
            .storage()
            .persistent()
            .get(&state_key)
            .unwrap_or(SpendState { spent: 0, window_start_ledger: e.ledger().sequence() });

        // Roll window if expired
        let cur = e.ledger().sequence();
        if cur.saturating_sub(state.window_start_ledger) >= params.period_ledgers {
            state.spent = 0;
            state.window_start_ledger = cur;
        }

        let amount = sac_transfer_amount(e, &context);
        if amount > 0 {
            state.spent = state.spent.saturating_add(amount);
            if state.spent > params.spending_limit {
                panic!(
                    "BlendYieldClaimPolicy: spending limit exceeded ({} > {})",
                    state.spent, params.spending_limit
                );
            }
            e.storage().persistent().set(&state_key, &state);
            e.storage()
                .persistent()
                .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);
        }
    }

    fn install(
        e: &Env,
        install_params: BlendYieldClaimParams,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();
        assert!(install_params.spending_limit > 0, "spending_limit must be > 0");
        assert!(install_params.period_ledgers > 0, "period_ledgers must be > 0");

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        e.storage().persistent().set(&params_key, &install_params);
        e.storage()
            .persistent()
            .extend_ttl(&params_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        let state = SpendState { spent: 0, window_start_ledger: e.ledger().sequence() };
        e.storage().persistent().set(&state_key, &state);
        e.storage()
            .persistent()
            .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        e.events().publish(
            (symbol_short!("installed"),),
            (smart_account, context_rule.id, install_params.spending_limit, install_params.period_ledgers),
        );
    }

    fn uninstall(e: &Env, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        // Panic if not installed — matches OZ convention
        if !e.storage().persistent().has(&params_key) {
            panic!("BlendYieldClaimPolicy: not installed");
        }

        e.storage().persistent().remove(&params_key);
        e.storage().persistent().remove(&state_key);

        e.events().publish(
            (soroban_sdk::Symbol::new(e, "uninstalled"),),
            (smart_account, context_rule.id),
        );
    }
}

#[contractimpl]
impl BlendYieldClaimPolicy {}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the amount from a SAC `transfer(from, to, amount: i128)` call.
/// Returns 0 for any other context type or function name.
fn sac_transfer_amount(e: &Env, context: &Context) -> i128 {
    match context {
        Context::Contract(ctx) if ctx.fn_name == symbol_short!("transfer") => ctx
            .args
            .get(2)
            .and_then(|v| v.try_into_val(e).ok())
            .unwrap_or(0),
        _ => 0,
    }
}
