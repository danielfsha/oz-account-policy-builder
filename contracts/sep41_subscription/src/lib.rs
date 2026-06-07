//! SEP-41 subscription policy — generated example by OZ Policy Builder.
//!
//! ⚠  REVIEW BEFORE DEPLOYING. Never deploy automatically.
//!
//! Allows a delegated service to collect a fixed recurring payment of
//! up to 10 USDC per month from the user's smart account.

#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    SubState(Address, Symbol),
}

#[contracttype]
#[derive(Clone)]
pub struct SubState {
    pub paid_this_period: i128,
    pub period_start_ledger: u32,
}

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PolicyError {
    SubscriptionLimitExceeded = 1,
    NotInstalled = 2,
}

/// 10 USDC per month (7 decimals)
const SUBSCRIPTION_CAP: i128 = 100_000_000;
/// 30 days in ledgers (2_592_000 s / 5 s per ledger)
const PERIOD_LEDGERS: u32 = 518_400;

#[contract]
pub struct Sep41SubscriptionPolicy;

#[contractimpl]
impl Sep41SubscriptionPolicy {
    pub fn install(env: Env, account: Address, context_rule_id: Symbol) {
        account.require_auth();
        env.storage().persistent().set(
            &DataKey::SubState(account, context_rule_id),
            &SubState { paid_this_period: 0, period_start_ledger: env.ledger().sequence() },
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
            .get::<_, SubState>(&DataKey::SubState(account, context_rule_id))
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
        let key = DataKey::SubState(account.clone(), context_rule_id);
        let mut state: SubState = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(PolicyError::NotInstalled)?;

        let cur = env.ledger().sequence();
        if cur.saturating_sub(state.period_start_ledger) >= PERIOD_LEDGERS {
            state.paid_this_period = 0;
            state.period_start_ledger = cur;
        }

        state.paid_this_period = state.paid_this_period.saturating_add(amount);
        if state.paid_this_period > SUBSCRIPTION_CAP {
            return Err(PolicyError::SubscriptionLimitExceeded);
        }
        env.storage().persistent().set(&key, &state);
        Ok(())
    }

    pub fn uninstall(env: Env, account: Address, context_rule_id: Symbol) {
        env.storage()
            .persistent()
            .remove(&DataKey::SubState(account, context_rule_id));
    }
}
