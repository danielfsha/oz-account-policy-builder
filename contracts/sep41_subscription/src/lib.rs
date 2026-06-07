//! # SEP-41 Subscription Policy
//!
//! Implements [`stellar_accounts::policies::Policy`] to allow a delegated
//! billing service to collect a fixed recurring payment from a smart account
//! once per billing period.
//!
//! ## Invariants enforced
//! - At most one `transfer` call per `period_ledgers` window.
//! - The `amount` of each transfer must be <= `subscription_amount`.
//! - The `to` address must match the `recipient` set at install time.
//!
//! ## SEP-41 token transfer signature
//! ```text
//! transfer(from: Address, to: Address, amount: i128)
//! ```
//!
//! ## ⚠  REVIEW BEFORE DEPLOYING — deployment is never automatic.

#![no_std]

use soroban_sdk::{
    auth::Context, contract, contractimpl, contracttype, symbol_short, Address, Env, Val, Vec,
};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};

// ── TTL ───────────────────────────────────────────────────────────────────────
const TTL_THRESHOLD: u32 = 518_400;  // ~30 days
const EXTEND_AMOUNT: u32 = 2_592_000; // ~150 days

// ── Storage keys ──────────────────────────────────────────────────────────────
#[contracttype]
#[derive(Clone)]
enum StorageKey {
    Params(Address, u32),
    State(Address, u32),
}

// ── Data types ────────────────────────────────────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Sep41SubscriptionParams {
    /// Authorised recipient of subscription payments.
    pub recipient: Address,
    /// Max amount per payment (raw SEP-41 token units, typically 7 decimals).
    pub subscription_amount: i128,
    /// Minimum ledgers between payments. 518_400 ≈ 30 days.
    pub period_ledgers: u32,
}

#[contracttype]
#[derive(Clone)]
struct SubscriptionState {
    /// Ledger sequence of the last successful payment; 0 = never paid.
    last_payment_ledger: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────────
#[contract]
pub struct Sep41SubscriptionPolicy;

impl Policy for Sep41SubscriptionPolicy {
    type AccountParams = Sep41SubscriptionParams;

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

        let params: Sep41SubscriptionParams = e
            .storage()
            .persistent()
            .get(&params_key)
            .unwrap_or_else(|| panic!("Sep41SubscriptionPolicy: not installed"));

        // Only apply to transfer calls
        let (to, amount) = match extract_transfer_args(&context) {
            Some(v) => v,
            None => return,
        };

        // ── Recipient check ───────────────────────────────────────────────
        if to != params.recipient {
            panic!(
                "Sep41SubscriptionPolicy: recipient mismatch \
                 (expected={:?}, got={:?})",
                params.recipient, to
            );
        }

        // ── Amount check ──────────────────────────────────────────────────
        if amount > params.subscription_amount {
            panic!(
                "Sep41SubscriptionPolicy: amount exceeds subscription ({} > {})",
                amount, params.subscription_amount
            );
        }

        // ── Period check (at most once per period) ────────────────────────
        let mut state: SubscriptionState = e
            .storage()
            .persistent()
            .get(&state_key)
            .unwrap_or(SubscriptionState { last_payment_ledger: 0 });

        let cur = e.ledger().sequence();
        if state.last_payment_ledger > 0 {
            let elapsed = cur.saturating_sub(state.last_payment_ledger);
            if elapsed < params.period_ledgers {
                panic!(
                    "Sep41SubscriptionPolicy: payment too soon \
                     (elapsed={} ledgers, period={})",
                    elapsed, params.period_ledgers
                );
            }
        }

        state.last_payment_ledger = cur;
        e.storage().persistent().set(&state_key, &state);
        e.storage()
            .persistent()
            .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);
    }

    fn install(
        e: &Env,
        install_params: Sep41SubscriptionParams,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();
        assert!(install_params.subscription_amount > 0, "subscription_amount must be > 0");
        assert!(install_params.period_ledgers > 0, "period_ledgers must be > 0");

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        e.storage().persistent().set(&params_key, &install_params);
        e.storage()
            .persistent()
            .extend_ttl(&params_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        let state = SubscriptionState { last_payment_ledger: 0 };
        e.storage().persistent().set(&state_key, &state);
        e.storage()
            .persistent()
            .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        e.events().publish(
            (symbol_short!("installed"),),
            (
                smart_account,
                context_rule.id,
                install_params.recipient,
                install_params.subscription_amount,
            ),
        );
    }

    fn uninstall(e: &Env, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        if !e.storage().persistent().has(&params_key) {
            panic!("Sep41SubscriptionPolicy: not installed");
        }

        e.storage().persistent().remove(&params_key);
        e.storage().persistent().remove(&state_key);

        e.events().publish(
            (symbol_short!("uninstalled"),),
            (smart_account, context_rule.id),
        );
    }
}

#[contractimpl]
impl Sep41SubscriptionPolicy {
    /// Query current spend state for a (smart_account, context_rule_id) pair.
    /// Returns `(last_payment_ledger, subscription_amount, period_ledgers)`.
    pub fn get_state(
        e: Env,
        smart_account: Address,
        context_rule_id: u32,
    ) -> (u32, i128, u32) {
        let params: Sep41SubscriptionParams = e
            .storage()
            .persistent()
            .get(&StorageKey::Params(smart_account.clone(), context_rule_id))
            .unwrap_or_else(|| panic!("Sep41SubscriptionPolicy: not installed"));
        let state: SubscriptionState = e
            .storage()
            .persistent()
            .get(&StorageKey::State(smart_account, context_rule_id))
            .unwrap_or(SubscriptionState { last_payment_ledger: 0 });
        (state.last_payment_ledger, params.subscription_amount, params.period_ledgers)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `(to, amount)` from a SEP-41 `transfer(from, to, amount)` context.
fn extract_transfer_args(context: &Context) -> Option<(Address, i128)> {
    match context {
        Context::Contract(ctx) if ctx.fn_name == symbol_short!("transfer") => {
            let to: Address = ctx.args.get(1).and_then(|v| Address::try_from(v).ok())?;
            let amount: i128 = ctx.args.get(2).and_then(|v| i128::try_from(v).ok())?;
            Some((to, amount))
        }
        _ => None,
    }
}
