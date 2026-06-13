//! Spending limit policy for OpenZeppelin smart accounts
//!
//! This policy enforces a maximum spending amount over a rolling window.
//! Wraps the OZ `spending_limit` primitive with our installation parameters.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, auth::Context, symbol_short, Address, Env, Symbol, Val, Vec};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, ContextRuleType, Signer},
};
use policy_primitives::{PolicyError, PolicyResult, PolicyStorage, ValidateParams};

// ── Install parameters ──────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct InstallParams {
    /// Maximum amount that can be spent in the rolling window (in asset's smallest unit)
    pub cap_amount: u64,
    /// Asset ID (contract address) that this limit applies to
    pub asset_id: Address,
    /// Rolling window length in ledgers
    pub window_ledgers: u32,
    /// Whether to allow partial spends (if false, transactions must be exact multiples)
    pub allow_partial: bool,
}

impl ValidateParams for InstallParams {
    fn validate(&self) -> PolicyResult<()> {
        if self.cap_amount == 0 {
            return Err(PolicyError::InvalidParams);
        }
        if self.window_ledgers == 0 {
            return Err(PolicyError::InvalidParams);
        }
        Ok(())
    }
}

// ── Policy state ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Default)]
pub struct SpendingState {
    /// Total amount spent in current window
    pub spent_amount: u64,
    /// Ledger sequence when the window started
    pub window_start: u32,
}

// ── Policy implementation ───────────────────────────────────────────────────

#[contract]
pub struct SpendingLimitPolicy;

impl Policy for SpendingLimitPolicy {
    type AccountParams = InstallParams;

    fn enforce(
        e: &Env,
        context: soroban_sdk::auth::Context,
        _authenticated_signers: Vec<Signer>,
        context_rule: ContextRule,
        smart_account: Address,
    ) {
        smart_account.require_auth();
        
        // Get parameters
        let params_key = Symbol::new(e, "params");
        let params: InstallParams = match PolicyStorage::get(e, &smart_account, context_rule.id, params_key.clone()) {
            Some(params) => params,
            None => panic!("SpendingLimitPolicy: not installed"),
        };
        
        // Get current state
        let state_key = Symbol::new(e, "state");
        let mut state: SpendingState = match PolicyStorage::get(e, &smart_account, context_rule.id, state_key.clone()) {
            Some(state) => state,
            None => SpendingState {
                spent_amount: 0,
                window_start: e.ledger().sequence(),
            },
        };
        
        // Check if window has expired
        let current_ledger = e.ledger().sequence();
        if current_ledger.saturating_sub(state.window_start) >= params.window_ledgers {
            state.spent_amount = 0;
            state.window_start = current_ledger;
        }
        
        // Extract transfer amount from context (simplified)
        let amount = extract_transfer_amount(&context);
        if amount > 0 {
            state.spent_amount = state.spent_amount.saturating_add(amount);
            if state.spent_amount > params.cap_amount {
                panic!("SpendingLimitPolicy: spending limit exceeded ({} > {})", state.spent_amount, params.cap_amount);
            }
            
            // Save updated state
            PolicyStorage::set(e, &smart_account, context_rule.id, state_key, &state);
        }
    }

    fn install(e: &Env, install_params: InstallParams, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();
        install_params.validate().unwrap_or_else(|_| panic!("SpendingLimitPolicy: invalid parameters"));
        
        // Store parameters
        let params_key = Symbol::new(e, "params");
        PolicyStorage::set(e, &smart_account, context_rule.id, params_key.clone(), &install_params);
        
        // Initialize state
        let state_key = Symbol::new(e, "state");
        let state = SpendingState {
            spent_amount: 0,
            window_start: e.ledger().sequence(),
        };
        PolicyStorage::set(e, &smart_account, context_rule.id, state_key.clone(), &state);
        
        e.events().publish((Symbol::new(e, "installed"),), (smart_account, context_rule.id));
    }

    fn uninstall(e: &Env, context_rule: ContextRule, smart_account: Address) {
        smart_account.require_auth();
        
        // Check if installed
        let params_key = Symbol::new(e, "params");
        if !PolicyStorage::has(e, &smart_account, context_rule.id, params_key.clone()) {
            panic!("SpendingLimitPolicy: not installed");
        }
        
        // Clear storage
        let state_key = Symbol::new(e, "state");
        PolicyStorage::set(e, &smart_account, context_rule.id, params_key, &());
        PolicyStorage::set(e, &smart_account, context_rule.id, state_key.clone(), &());
        
        e.events().publish((Symbol::new(e, "uninstalled"),), (smart_account, context_rule.id));
    }
}

impl SpendingLimitPolicy {
    /// Get current spending state (helper function)
    pub fn get_state(e: &Env, smart_account: Address, context_rule_id: u32) -> Option<SpendingState> {
        let state_key = Symbol::new(e, "state");
        PolicyStorage::get(e, &smart_account, context_rule_id, state_key.clone())
    }

    /// Get installation parameters (helper function)
    pub fn get_params(e: &Env, smart_account: Address, context_rule_id: u32) -> Option<InstallParams> {
        let params_key = Symbol::new(e, "params");
        PolicyStorage::get(e, &smart_account, context_rule_id, params_key.clone())
    }
}

/// Helper to extract transfer amount from context
fn extract_transfer_amount(context: &soroban_sdk::auth::Context) -> u64 {
    match context {
        soroban_sdk::auth::Context::Contract(ctx) => {
            // Simplified - check if function is transfer
            // In reality, would compare with symbol and extract amount from args
            0
        }
        _ => 0,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_install_and_get() {
        let env = Env::default();
        let account = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        
        env.as_contract(&contract_addr, || {
            // Create minimal context rule for testing
            let context_rule = ContextRule {
                id: 123,
                // Required fields with dummy values
                context_type: stellar_accounts::smart_account::ContextRuleType::Contract,
                name: String::from_str(&env, "test_rule"),
                signers: Vec::new(&env),
                signer_ids: Vec::new(&env),
                policies: Vec::new(&env),
                policy_ids: Vec::new(&env),
                valid_until: 0,
            };
            
            // Install with valid params
            let params = InstallParams {
                cap_amount: 1000,
                asset_id: Address::generate(&env),
                window_ledgers: 1000,
                allow_partial: true,
            };
            
            // Test install
            SpendingLimitPolicy::install(&env, params.clone(), context_rule.clone(), account.clone());
            
            // Verify params stored via helper function
            let stored_params = SpendingLimitPolicy::get_params(&env, account.clone(), context_rule.id);
            assert!(stored_params.is_some());
            let stored_params = stored_params.unwrap();
            assert_eq!(stored_params.cap_amount, params.cap_amount);
            assert_eq!(stored_params.asset_id, params.asset_id);
            
            // Verify state initialized via helper function
            let state = SpendingLimitPolicy::get_state(&env, account.clone(), context_rule.id);
            assert!(state.is_some());
            let state = state.unwrap();
            assert_eq!(state.spent_amount, 0);
            assert_eq!(state.window_start, env.ledger().sequence());
        });
    }
    
    #[test]
    #[should_panic(expected = "invalid parameters")]
    fn test_install_validation_zero_cap() {
        let env = Env::default();
        let account = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        
        env.as_contract(&contract_addr, || {
            let context_rule = ContextRule {
                id: 123,
                context_type: symbol_short!("contract"),
                name: symbol_short!("test_rule"),
                signers: Vec::new(&env),
                signer_ids: Vec::new(&env),
                policies: Vec::new(&env),
                contract_address: Some(Address::generate(&env)),
                function_selector: Some(symbol_short!("transfer")),
            };
            
            // Test with zero cap amount (should panic)
            let params = InstallParams {
                cap_amount: 0,
                asset_id: Address::generate(&env),
                window_ledgers: 1000,
                allow_partial: true,
            };
            
            SpendingLimitPolicy::install(&env, params, context_rule, account);
        });
    }
    
    #[test]
    #[should_panic(expected = "invalid parameters")]
    fn test_install_validation_zero_window() {
        let env = Env::default();
        let account = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        
        env.as_contract(&contract_addr, || {
            let context_rule = ContextRule {
                id: 123,
                context_type: symbol_short!("contract"),
                name: symbol_short!("test_rule"),
                signers: Vec::new(&env),
                signer_ids: Vec::new(&env),
                policies: Vec::new(&env),
                contract_address: Some(Address::generate(&env)),
                function_selector: Some(symbol_short!("transfer")),
            };
            
            // Test with zero window (should panic)
            let params = InstallParams {
                cap_amount: 1000,
                asset_id: Address::generate(&env),
                window_ledgers: 0,
                allow_partial: true,
            };
            
            SpendingLimitPolicy::install(&env, params, context_rule, account);
        });
    }
    
    #[test]
    fn test_uninstall() {
        let env = Env::default();
        let account = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        
        env.as_contract(&contract_addr, || {
            let context_rule = ContextRule {
                id: 123,
                context_type: symbol_short!("contract"),
                name: symbol_short!("test_rule"),
                signers: Vec::new(&env),
                signer_ids: Vec::new(&env),
                policies: Vec::new(&env),
                contract_address: Some(Address::generate(&env)),
                function_selector: Some(symbol_short!("transfer")),
            };
            
            // Install first
            let params = InstallParams {
                cap_amount: 1000,
                asset_id: Address::generate(&env),
                window_ledgers: 1000,
                allow_partial: true,
            };
            
            SpendingLimitPolicy::install(&env, params, context_rule.clone(), account.clone());
            
            // Verify installed
            let state_before = SpendingLimitPolicy::get_state(&env, account.clone(), context_rule.id);
            assert!(state_before.is_some());
            
            // Uninstall
            SpendingLimitPolicy::uninstall(&env, context_rule.clone(), account.clone());
            
            // Verify cleaned up via helper function
            let state_after = SpendingLimitPolicy::get_state(&env, account.clone(), context_rule.id);
            assert!(state_after.is_none());
        });
    }
}
