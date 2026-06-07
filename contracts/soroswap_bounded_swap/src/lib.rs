//! Soroswap bounded swap policy — generated example by OZ Policy Builder.
//!
//! ⚠  REVIEW BEFORE DEPLOYING. Never deploy automatically.
//!
//! Allows a delegated agent to swap on Soroswap with:
//! - A max slippage of 5% (500 bps)
//! - A max swap amount of 500 USDC per day

#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    SpendState(Address, Symbol),
}

#[contracttype]
#[derive(Clone)]
pub struct SpendState {
    pub spent: i128,
    pub window_start_ledger: u32,
}

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PolicyError {
    SpendingLimitExceeded = 1,
    SlippageExceeded = 2,
    NotInstalled = 3,
}

/// 500 USDC per day (7 decimals)
const SPENDING_CAP: i128 = 5_000_000_000;
/// 1 day in ledgers
const WINDOW_LEDGERS: u32 = 17_280;
/// 5% slippage = 500 bps
const MAX_SLIPPAGE_BPS: u32 = 500;

#[contract]
pub struct SoroswapBoundedSwapPolicy;

#[contractimpl]
impl SoroswapBoundedSwapPolicy {
    pub fn install(env: Env, account: Address, context_rule_id: Symbol) {
        account.require_auth();
        env.storage().persistent().set(
            &DataKey::SpendState(account, context_rule_id),
            &SpendState { spent: 0, window_start_ledger: env.ledger().sequence() },
        );
    }

    pub fn can_enforce(
        env: Env,
        account: Address,
        context_rule_id: Symbol,
        _contract: Address,
        _function: Symbol,
    ) -> Result<(), PolicyError> {
        env.storage()
            .persistent()
            .get::<_, SpendState>(&DataKey::SpendState(account, context_rule_id))
            .map(|_| ())
            .ok_or(PolicyError::NotInstalled)
    }

    pub fn enforce(
        env: Env,
        account: Address,
        context_rule_id: Symbol,
        _contract: Address,
        _function: Symbol,
        amount: i128,
    ) -> Result<(), PolicyError> {
        let key = DataKey::SpendState(account.clone(), context_rule_id);
        let mut state: SpendState = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(PolicyError::NotInstalled)?;

        let cur = env.ledger().sequence();
        if cur.saturating_sub(state.window_start_ledger) >= WINDOW_LEDGERS {
            state.spent = 0;
            state.window_start_ledger = cur;
        }

        state.spent = state.spent.saturating_add(amount);
        if state.spent > SPENDING_CAP {
            return Err(PolicyError::SpendingLimitExceeded);
        }
        env.storage().persistent().set(&key, &state);
        Ok(())
    }

    pub fn uninstall(env: Env, account: Address, context_rule_id: Symbol) {
        env.storage()
            .persistent()
            .remove(&DataKey::SpendState(account, context_rule_id));
    }

    /// Check slippage: amount_out must be >= amount_in * (1 - max_slippage_bps/10000)
    pub fn check_slippage(_env: Env, amount_in: i128, amount_out: i128) -> Result<(), PolicyError> {
        let min_out = amount_in - (amount_in * MAX_SLIPPAGE_BPS as i128 / 10_000);
        if amount_out < min_out {
            return Err(PolicyError::SlippageExceeded);
        }
        Ok(())
    }
}
