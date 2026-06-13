//! Time window policy for OpenZeppelin smart accounts
//!
//! This policy restricts transactions to specific time windows (e.g., only during business hours,
//! or once per day/week/month). Not available in OZ primitives, so we implement it as a custom policy.

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
pub enum WindowType {
    /// Allow only once per day (24 hours)
    Daily,
    /// Allow only once per week (7 days)
    Weekly,
    /// Allow only once per month (30 days)
    Monthly,
    /// Custom window in ledgers
    Custom(u32),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InstallParams {
    /// Type of time window to enforce
    pub window_type: WindowType,
    /// Offset in ledgers from the start of the window (e.g., allow transactions after ledger X)
    pub offset_ledgers: u32,
}

impl ValidateParams for InstallParams {
    fn validate(&self) -> PolicyResult<()> {
        if let WindowType::Custom(ledgers) = self.window_type {
            if ledgers == 0 {
                return Err(PolicyError::InvalidParams);
            }
        }
        Ok(())
    }
}

// ── Policy state ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct TimeWindowState {
    /// Ledger sequence of the last successful transaction
    pub last_execution: u32,
}

// ── Policy implementation ───────────────────────────────────────────────────

#[contract]
pub struct TimeWindowPolicy;

impl TimeWindowPolicy {
    /// Calculate window size in ledgers based on window type
    fn window_size(&self, params: &InstallParams) -> u32 {
        match params.window_type {
            WindowType::Daily => 24 * 60 * 6, // 24 hours * 60 minutes * 6 ledgers per minute (approx)
            WindowType::Weekly => 7 * 24 * 60 * 6, // 7 days
            WindowType::Monthly => 30 * 24 * 60 * 6, // 30 days
            WindowType::Custom(ledgers) => ledgers,
        }
    }
    
    /// Check if current ledger is within allowed window
    fn is_within_window(&self, env: &Env, params: &InstallParams, state: &TimeWindowState) -> bool {
        let current_ledger = env.ledger().sequence();
        let window_size = self.window_size(params);
        
        if state.last_execution == 0 {
            // First execution - check offset
            return current_ledger >= params.offset_ledgers;
        }
        
        // Check if we're in the next window
        let next_window_start = state.last_execution + window_size;
        current_ledger >= next_window_start
    }
}

#[contractimpl]
impl TimeWindowPolicy {
    /// Install the policy for a specific smart account and context rule
    pub fn install(env: Env, account: Address, context_rule_id: u32, params: InstallParams) -> PolicyResult<()> {
        params.validate()?;
        
        // Store parameters
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        // Initialize state (no executions yet)
        let state = TimeWindowState {
            last_execution: 0,
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

    /// Get current window state
    pub fn get_state(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<TimeWindowState> {
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

    /// Simulate whether a transaction would be allowed at the current ledger
    pub fn simulate_check(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<bool> {
        let params: InstallParams = match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(p) => p,
            None => return Err(PolicyError::NotFound),
        };
        
        let state: TimeWindowState = match PolicyStorage::get(&env, &account, context_rule_id, "state") {
            Some(s) => s,
            None => return Err(PolicyError::NotFound),
        };
        
        Ok(self.is_within_window(&env, &params, &state))
    }
}

#[contractimpl]
impl Policy for TimeWindowPolicy {
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
        
        let mut state: TimeWindowState = match PolicyStorage::get(&env, &account, rule.id, "state") {
            Some(s) => s,
            None => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        // Check if within window
        if !self.is_within_window(&env, &params, &state) {
            return Err(stellar_accounts::policies::PolicyError::Denied);
        }
        
        // Update last execution time
        state.last_execution = env.ledger().sequence();
        PolicyStorage::set(&env, &account, rule.id, "state", &state);
        
        Ok(())
    }

    fn weight(&self) -> u32 {
        5 // Lower weight than spending limit
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_window_calculations() {
        let policy = TimeWindowPolicy;
        let env = Env::default();
        
        // Test daily window size
        let params = InstallParams {
            window_type: WindowType::Daily,
            offset_ledgers: 0,
        };
        let window_size = policy.window_size(&params);
        assert_eq!(window_size, 24 * 60 * 6); // 24h * 60min * 6 ledgers/min
        
        // Test custom window
        let params = InstallParams {
            window_type: WindowType::Custom(1000),
            offset_ledgers: 0,
        };
        let window_size = policy.window_size(&params);
        assert_eq!(window_size, 1000);
    }
    
    #[test]
    fn test_window_validation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = TimeWindowPolicy;
        
        // Test invalid custom window (zero ledgers)
        let params = InstallParams {
            window_type: WindowType::Custom(0),
            offset_ledgers: 0,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test valid custom window
        let params = InstallParams {
            window_type: WindowType::Custom(1000),
            offset_ledgers: 0,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok());
        
        // Verify params stored
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        match stored_params.window_type {
            WindowType::Custom(ledgers) => assert_eq!(ledgers, 1000),
            _ => panic!("Expected Custom window type"),
        }
    }
    
    #[test]
    fn test_window_enforcement_logic() {
        let env = Env::default();
        let policy = TimeWindowPolicy;
        
        // Test is_within_window with first execution
        let params = InstallParams {
            window_type: WindowType::Daily,
            offset_ledgers: 100,
        };
        
        let state = TimeWindowState {
            last_execution: 0,
        };
        
        // Before offset - should fail
        env.ledger().with_sequence(50);
        assert!(!policy.is_within_window(&env, &params, &state));
        
        // After offset - should pass (first execution)
        env.ledger().with_sequence(150);
        assert!(policy.is_within_window(&env, &params, &state));
        
        // Test is_within_window with previous execution
        let state = TimeWindowState {
            last_execution: 1000,
        };
        
        // Still in same window - should fail
        env.ledger().with_sequence(1100);
        assert!(!policy.is_within_window(&env, &params, &state));
        
        // In next window - should pass
        let window_size = policy.window_size(&params);
        env.ledger().with_sequence(1000 + window_size + 1);
        assert!(policy.is_within_window(&env, &params, &state));
    }
}
