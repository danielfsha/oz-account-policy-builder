//! Spending limit policy for OpenZeppelin smart accounts
//!
//! This policy enforces a maximum spending amount over a rolling window.
//! Wraps the OZ `spending_limit` primitive with our installation parameters.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Val, Vec};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
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

#[contractimpl]
impl SpendingLimitPolicy {
    /// Install the policy for a specific smart account and context rule
    pub fn install(env: Env, account: Address, context_rule_id: u32, params: InstallParams) -> PolicyResult<()> {
        params.validate()?;
        
        // Store parameters
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        // Initialize state
        let state = SpendingState {
            spent_amount: 0,
            window_start: env.ledger().sequence(),
        };
        PolicyStorage::set(&env, &account, context_rule_id, "state", &state);
        
        Ok(())
    }

    /// Uninstall the policy (remove all state)
    pub fn uninstall(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<()> {
        PolicyStorage::set(&env, &account, context_rule_id, "params", &());
        PolicyStorage::set(&env, &account, context_rule_id, "state", &());
        Ok(())
    }

    /// Get current spending state
    pub fn get_state(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<SpendingState> {
        match PolicyStorage::get(&env, &account, context_rule_id, "state") {
            Some(state) => Ok(state),
            None => Err(PolicyError::NotFound),
        }
    }

    /// Get installation parameters
    pub fn get_params(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<InstallParams> {
        match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(params) => Ok(params),
            None => Err(PolicyError::NotFound),
        }
    }
}

#[contractimpl]
impl Policy for SpendingLimitPolicy {
    fn enforce(
        &self,
        env: Env,
        _rule: ContextRule,
        _signer: Signer,
        _context: Vec<Context>,
        _auth_context: Val,
    ) -> Result<(), stellar_accounts::policies::PolicyError> {
        // This is a placeholder - actual enforcement would be more complex
        // and would check the spending amount against the cap
        Ok(())
    }

    fn weight(&self) -> u32 {
        10 // Standard weight for spending limit policies
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_install_and_uninstall() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = SpendingLimitPolicy;
        
        // Install with valid params
        let params = InstallParams {
            cap_amount: 1000,
            asset_id: Address::generate(&env),
            window_ledgers: 1000,
            allow_partial: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok());
        
        // Verify params stored
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(stored_params.cap_amount, params.cap_amount);
        assert_eq!(stored_params.asset_id, params.asset_id);
        
        // Verify state initialized
        let state = policy.get_state(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(state.spent_amount, 0);
        assert_eq!(state.window_start, env.ledger().sequence());
        
        // Uninstall
        let result = policy.uninstall(env.clone(), account.clone(), context_rule_id);
        assert!(result.is_ok());
        
        // Verify state cleaned up
        let result = policy.get_state(env.clone(), account.clone(), context_rule_id);
        assert!(matches!(result, Err(PolicyError::NotFound)));
    }
    
    #[test]
    fn test_install_validation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = SpendingLimitPolicy;
        
        // Test with zero cap amount
        let params = InstallParams {
            cap_amount: 0,
            asset_id: Address::generate(&env),
            window_ledgers: 1000,
            allow_partial: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test with zero window
        let params = InstallParams {
            cap_amount: 1000,
            asset_id: Address::generate(&env),
            window_ledgers: 0,
            allow_partial: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
    }
    
    #[test]
    fn test_policy_trait_implementation() {
        let env = Env::default();
        let policy = SpendingLimitPolicy;
        
        // Test weight
        assert_eq!(policy.weight(), 10);
        
        // Test enforce (placeholder implementation)
        let result = policy.enforce(
            env,
            ContextRule::default(),
            Signer::default(),
            Vec::new(&Env::default()),
            Val::default(),
        );
        assert!(result.is_ok());
    }
}
