//! Frequency limit policy for OpenZeppelin smart accounts
//!
//! This policy limits the number of transactions that can be executed within a time window.
//! Not available in OZ primitives, so we implement it as a custom policy.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Val, Vec};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};
use policy_primitives::{PolicyError, PolicyResult, PolicyStorage, ValidateParams};

// ── Install parameters ──────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct InstallParams {
    /// Maximum number of calls allowed in the window
    pub max_calls: u32,
    /// Window size in ledgers
    pub window_ledgers: u32,
    /// Whether to reset count on window expiry (true) or use sliding window (false)
    pub reset_on_expiry: bool,
}

impl ValidateParams for InstallParams {
    fn validate(&self) -> PolicyResult<()> {
        if self.max_calls == 0 {
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
pub struct FrequencyState {
    /// Number of calls made in current window
    pub call_count: u32,
    /// Ledger sequence when the current window started
    pub window_start: u32,
    /// Ledger sequence of the last call
    pub last_call: u32,
}

// ── Policy implementation ───────────────────────────────────────────────────

#[contract]
pub struct FrequencyLimitPolicy;

impl FrequencyLimitPolicy {
    /// Check and update window based on current ledger
    fn update_window(&self, env: &Env, params: &InstallParams, mut state: FrequencyState) -> FrequencyState {
        let current_ledger = env.ledger().sequence();
        
        if params.reset_on_expiry {
            // Fixed windows: reset if we've moved beyond the window
            if current_ledger >= state.window_start + params.window_ledgers {
                state.call_count = 0;
                state.window_start = current_ledger;
            }
        } else {
            // Sliding window: remove calls that are outside the window
            // For simplicity, we use a fixed window approach but could be enhanced
            // with more sophisticated sliding window logic
            if current_ledger >= state.window_start + params.window_ledgers {
                // Reset window
                state.call_count = 0;
                state.window_start = current_ledger;
            }
        }
        
        state
    }
}

#[contractimpl]
impl FrequencyLimitPolicy {
    /// Install the policy for a specific smart account and context rule
    pub fn install(env: Env, account: Address, context_rule_id: u32, params: InstallParams) -> PolicyResult<()> {
        params.validate()?;
        
        // Store parameters
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        // Initialize state
        let state = FrequencyState {
            call_count: 0,
            window_start: env.ledger().sequence(),
            last_call: 0,
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

    /// Get current frequency state
    pub fn get_state(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<FrequencyState> {
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

    /// Simulate whether another call would be allowed at the current ledger
    pub fn simulate_check(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<bool> {
        let params: InstallParams = match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(p) => p,
            None => return Err(PolicyError::NotFound),
        };
        
        let state: FrequencyState = match PolicyStorage::get(&env, &account, context_rule_id, "state") {
            Some(s) => s,
            None => return Err(PolicyError::NotFound),
        };
        
        // Update window based on current ledger
        let updated_state = self.update_window(&env, &params, state);
        
        // Check if we've reached the limit
        Ok(updated_state.call_count < params.max_calls)
    }
}

#[contractimpl]
impl Policy for FrequencyLimitPolicy {
    fn enforce(
        &self,
        env: Env,
        rule: ContextRule,
        signer: Signer,
        context: Vec<Context>,
        auth_context: Val,
    ) -> Result<(), stellar_accounts::policies::PolicyError> {
        // Get account from context
        let account = match signer {
            Signer::SmartAccount(addr) => addr,
            _ => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        // Get stored params and state
        let params: InstallParams = match PolicyStorage::get(&env, &account, rule.id, "params") {
            Some(p) => p,
            None => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        let state: FrequencyState = match PolicyStorage::get(&env, &account, rule.id, "state") {
            Some(s) => s,
            None => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        // Update window based on current ledger
        let mut updated_state = self.update_window(&env, &params, state);
        
        // Check frequency limit
        if updated_state.call_count >= params.max_calls {
            return Err(stellar_accounts::policies::PolicyError::Denied);
        }
        
        // Increment call count and update last call time
        updated_state.call_count += 1;
        updated_state.last_call = env.ledger().sequence();
        
        // Save updated state
        PolicyStorage::set(&env, &account, rule.id, "state", &updated_state);
        
        Ok(())
    }

    fn weight(&self) -> u32 {
        5 // Similar weight to time window
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_install_and_basic_operations() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = FrequencyLimitPolicy;
        
        // Install with valid params
        let params = InstallParams {
            max_calls: 3,
            window_ledgers: 1000,
            reset_on_expiry: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok());
        
        // Verify params stored
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(stored_params.max_calls, params.max_calls);
        assert_eq!(stored_params.window_ledgers, params.window_ledgers);
        
        // Verify state initialized
        let state = policy.get_state(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(state.call_count, 0);
        assert_eq!(state.last_call, 0);
        assert_eq!(state.window_start, env.ledger().sequence());
    }
    
    #[test]
    fn test_install_validation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = FrequencyLimitPolicy;
        
        // Test with zero max_calls
        let params = InstallParams {
            max_calls: 0,
            window_ledgers: 1000,
            reset_on_expiry: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test with zero window_ledgers
        let params = InstallParams {
            max_calls: 3,
            window_ledgers: 0,
            reset_on_expiry: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
    }
    
    #[test]
    fn test_window_update_logic() {
        let env = Env::default();
        let policy = FrequencyLimitPolicy;
        
        // Test reset_on_expiry = true
        let params = InstallParams {
            max_calls: 3,
            window_ledgers: 100,
            reset_on_expiry: true,
        };
        
        let mut state = FrequencyState {
            call_count: 2,
            window_start: 1000,
            last_call: 1050,
        };
        
        // Within window - should not reset
        env.ledger().with_sequence(1050);
        let updated = policy.update_window(&env, &params, state.clone());
        assert_eq!(updated.call_count, 2);
        assert_eq!(updated.window_start, 1000);
        
        // Beyond window - should reset
        env.ledger().with_sequence(1150);
        let updated = policy.update_window(&env, &params, state);
        assert_eq!(updated.call_count, 0);
        assert_eq!(updated.window_start, 1150);
    }
    
    #[test]
    fn test_simulation_check() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = FrequencyLimitPolicy;
        
        // Install policy
        let params = InstallParams {
            max_calls: 2,
            window_ledgers: 1000,
            reset_on_expiry: true,
        };
        
        policy.install(env.clone(), account.clone(), context_rule_id, params).unwrap();
        
        // Simulate check - should allow (0 calls used)
        let result = policy.simulate_check(env.clone(), account.clone(), context_rule_id).unwrap();
        assert!(result);
        
        // Manually update state to simulate 2 calls used
        let mut state = policy.get_state(env.clone(), account.clone(), context_rule_id).unwrap();
        state.call_count = 2;
        PolicyStorage::set(&env, &account, context_rule_id, "state", &state);
        
        // Simulate check - should deny (max calls reached)
        let result = policy.simulate_check(env.clone(), account.clone(), context_rule_id).unwrap();
        assert!(!result);
    }
}
