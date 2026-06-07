//! # Soroswap Bounded Swap Policy
//!
//! Implements [`stellar_accounts::policies::Policy`] to allow a delegated
//! agent to execute swaps on Soroswap under two hard constraints:
//!
//! 1. **Daily volume cap** — max outbound token amount per rolling 24-hour window.
//! 2. **Slippage guard** — the ratio of `amount_out_min / amount_in` must not
//!    fall below `1 - max_slippage_bps / 10_000`.
//!
//! ## Soroswap function signatures (swap_exact_tokens_for_tokens)
//! ```text
//! swap_exact_tokens_for_tokens(
//!   amount_in:      i128,
//!   amount_out_min: i128,
//!   path:           Vec<Address>,
//!   to:             Address,
//!   deadline:       u64,
//! )
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
const TTL_THRESHOLD: u32 = 120_960; // ~7 days
const EXTEND_AMOUNT: u32 = 518_400; // ~30 days

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
pub struct SoroswapBoundedSwapParams {
    /// Max total `amount_in` per rolling window (raw token units).
    pub daily_volume_cap: i128,
    /// Window length in ledgers. 17_280 ≈ 24 hours.
    pub period_ledgers: u32,
    /// Max allowed slippage in basis points (100 bps = 1%).
    /// Policy panics if `amount_out_min < amount_in * (10_000 - max_slippage_bps) / 10_000`.
    pub max_slippage_bps: u32,
}

#[contracttype]
#[derive(Clone)]
struct VolumeState {
    volume: i128,
    window_start_ledger: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────────
#[contract]
pub struct SoroswapBoundedSwapPolicy;

impl Policy for SoroswapBoundedSwapPolicy {
    type AccountParams = SoroswapBoundedSwapParams;

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

        let params: SoroswapBoundedSwapParams = e
            .storage()
            .persistent()
            .get(&params_key)
            .unwrap_or_else(|| panic!("SoroswapBoundedSwapPolicy: not installed"));

        // Only evaluate on swap calls
        let (amount_in, amount_out_min) = match extract_swap_args(&context) {
            Some(args) => args,
            None => return, // non-swap context — pass through
        };

        // ── Slippage check ────────────────────────────────────────────────
        // amount_out_min must be >= amount_in * (10_000 - max_slippage_bps) / 10_000
        let min_acceptable_out =
            amount_in * (10_000_i128 - params.max_slippage_bps as i128) / 10_000;
        if amount_out_min < min_acceptable_out {
            panic!(
                "SoroswapBoundedSwapPolicy: slippage exceeds {}bps (out_min={}, required>={})",
                params.max_slippage_bps, amount_out_min, min_acceptable_out
            );
        }

        // ── Volume cap ────────────────────────────────────────────────────
        let mut state: VolumeState = e
            .storage()
            .persistent()
            .get(&state_key)
            .unwrap_or(VolumeState { volume: 0, window_start_ledger: e.ledger().sequence() });

        let cur = e.ledger().sequence();
        if cur.saturating_sub(state.window_start_ledger) >= params.period_ledgers {
            state.volume = 0;
            state.window_start_ledger = cur;
        }

        state.volume = state.volume.saturating_add(amount_in);
        if state.volume > params.daily_volume_cap {
            panic!(
                "SoroswapBoundedSwapPolicy: daily volume cap exceeded ({} > {})",
                state.volume, params.daily_volume_cap
            );
        }

        e.storage().persistent().set(&state_key, &state);
        e.storage()
            .persistent()
            .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);
    }

    fn install(
        e: &Env,
        install_params: SoroswapBoundedSwapParams,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();
        assert!(install_params.daily_volume_cap > 0, "daily_volume_cap must be > 0");
        assert!(install_params.period_ledgers > 0, "period_ledgers must be > 0");
        assert!(install_params.max_slippage_bps <= 10_000, "max_slippage_bps must be <= 10_000");

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        e.storage().persistent().set(&params_key, &install_params);
        e.storage()
            .persistent()
            .extend_ttl(&params_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        let state = VolumeState { volume: 0, window_start_ledger: e.ledger().sequence() };
        e.storage().persistent().set(&state_key, &state);
        e.storage()
            .persistent()
            .extend_ttl(&state_key, TTL_THRESHOLD, EXTEND_AMOUNT);

        e.events().publish(
            (symbol_short!("installed"),),
            (
                smart_account,
                context_rule.id,
                install_params.daily_volume_cap,
                install_params.max_slippage_bps,
            ),
        );
    }

    fn uninstall(e: &Env, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();

        let params_key = StorageKey::Params(smart_account.clone(), context_rule.id);
        let state_key = StorageKey::State(smart_account.clone(), context_rule.id);

        if !e.storage().persistent().has(&params_key) {
            panic!("SoroswapBoundedSwapPolicy: not installed");
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
impl SoroswapBoundedSwapPolicy {}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `(amount_in, amount_out_min)` from a Soroswap
/// `swap_exact_tokens_for_tokens` call context, or `None` if the context
/// is not a matching function call.
fn extract_swap_args(context: &Context) -> Option<(i128, i128)> {
    let fn_swap = soroban_sdk::Symbol::new(
        // Symbol::new is available in no_std via the SDK
        &soroban_sdk::Env::default(), // NOTE: only safe at compile-time for symbol literals
        "swap_exact_tokens_for_tokens",
    );
    // We use a byte-level comparison approach instead to stay no_std safe
    match context {
        Context::Contract(ctx) => {
            // Soroswap function: swap_exact_tokens_for_tokens
            // We match on the symbol name bytes
            let fn_name_str = ctx.fn_name.to_string();
            if fn_name_str != "swap_exact_tokens_for_tokens" {
                return None;
            }
            let amount_in: i128 = ctx.args.get(0).and_then(|v| i128::try_from(v).ok())?;
            let amount_out_min: i128 = ctx.args.get(1).and_then(|v| i128::try_from(v).ok())?;
            Some((amount_in, amount_out_min))
        }
        _ => None,
    }
}
