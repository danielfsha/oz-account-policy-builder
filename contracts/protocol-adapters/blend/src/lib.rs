//! Blend protocol adapter for policy contracts
//!
//! This module provides typed interfaces to Blend protocol contracts
//! for use in policy contracts. It re-exports types from blend-contract-sdk
//! and adds policy-specific utilities.

#![no_std]

pub use blend_contract_sdk::*;

use soroban_sdk::{contracttype, Address, Env, Symbol, Val};

/// Blend-specific policy utilities
pub mod policy_utils {
    use super::*;
    
    /// Extract function name from Blend contract call context
    pub fn extract_blend_function(auth_context: &Val) -> Option<Symbol> {
        // TODO: Implement actual extraction from auth_context
        // For now, return a placeholder
        Some(Symbol::new(&Env::default(), "claim"))
    }
    
    /// Check if a call is a Blend yield claim
    pub fn is_yield_claim(function_name: &Symbol) -> bool {
        function_name == &Symbol::new(&Env::default(), "claim")
    }
    
    /// Check if a call is a Blend deposit
    pub fn is_deposit(function_name: &Symbol) -> bool {
        function_name == &Symbol::new(&Env::default(), "deposit")
    }
    
    /// Check if a call is a Blend withdrawal
    pub fn is_withdraw(function_name: &Symbol) -> bool {
        function_name == &Symbol::new(&Env::default(), "withdraw")
    }
}

/// Common types for Blend policy constraints
#[contracttype]
#[derive(Clone, Debug)]
pub enum BlendConstraint {
    /// Limit yield claims to specific pools
    YieldClaimLimit {
        /// Maximum claim amount per period
        max_amount: u64,
        /// Asset ID for the claim limit
        asset_id: Address,
        /// Period in ledgers
        period_ledgers: u32,
    },
    /// Restrict deposits to specific pools
    DepositRestriction {
        /// Allowed pool addresses
        allowed_pools: Vec<Address>,
        /// Maximum deposit amount
        max_deposit: u64,
    },
    /// Restrict withdrawals to specific conditions
    WithdrawalRestriction {
        /// Minimum withdrawal amount
        min_amount: u64,
        /// Cooldown period in ledgers
        cooldown_ledgers: u32,
    },
}

/// Blend protocol address registry (testnet)
pub mod addresses {
    use soroban_sdk::Address;
    
    // TODO: Replace with actual testnet addresses
    pub const BLEND_POOL_FACTORY: &str = "CBLENDPOOLFACTORY123456789012345678901234567890123456789012345";
    pub const BLEND_EMITTER: &str = "CBLENDEMITTER1234567890123456789012345678901234567890123456789";
    pub const BLEND_BACKSTOP: &str = "CBLENDBACKSTOP123456789012345678901234567890123456789012345678";
}

/// Helper to identify Blend contracts
pub fn is_blend_contract(env: &Env, contract_address: &Address) -> bool {
    // TODO: Implement actual check - could check against known addresses
    // or query contract interface
    false
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;
    
    #[test]
    fn test_policy_utils() {
        let env = Env::default();
        
        // Test function detection
        let claim_func = Symbol::new(&env, "claim");
        let deposit_func = Symbol::new(&env, "deposit");
        let withdraw_func = Symbol::new(&env, "withdraw");
        let other_func = Symbol::new(&env, "other");
        
        assert!(policy_utils::is_yield_claim(&claim_func));
        assert!(!policy_utils::is_yield_claim(&deposit_func));
        
        assert!(policy_utils::is_deposit(&deposit_func));
        assert!(!policy_utils::is_deposit(&claim_func));
        
        assert!(policy_utils::is_withdraw(&withdraw_func));
        assert!(!policy_utils::is_withdraw(&other_func));
    }
    
    #[test]
    fn test_blend_constraint_serialization() {
        let env = Env::default();
        
        // Test YieldClaimLimit serialization
        let constraint = BlendConstraint::YieldClaimLimit {
            max_amount: 1000,
            asset_id: Address::generate(&env),
            period_ledgers: 10000,
        };
        
        // Should compile without errors
        let _ = constraint.clone();
        
        // Test DepositRestriction
        let allowed_pools = vec![Address::generate(&env), Address::generate(&env)];
        let constraint = BlendConstraint::DepositRestriction {
            allowed_pools,
            max_deposit: 5000,
        };
        
        let _ = constraint;
    }
}
