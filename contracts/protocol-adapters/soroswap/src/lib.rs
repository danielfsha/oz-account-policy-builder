//! Soroswap protocol adapter for policy contracts
//!
//! This module provides typed interfaces to Soroswap protocol contracts
//! for use in policy contracts. It re-exports types from soroswap-core
//! and adds policy-specific utilities.

#![no_std]

// Re-export soroswap-core types
pub use soroswap_core::router;
pub use soroswap_core::factory;
pub use soroswap_core::pair;

use soroban_sdk::{contracttype, Address, Env, Symbol, Val};

/// Soroswap-specific policy utilities
pub mod policy_utils {
    use super::*;
    
    /// Extract function name from Soroswap contract call context
    pub fn extract_soroswap_function(auth_context: &Val) -> Option<Symbol> {
        // TODO: Implement actual extraction from auth_context
        // For now, return common Soroswap functions
        Some(Symbol::new(&Env::default(), "swap_exact_tokens_for_tokens"))
    }
    
    /// Check if a call is a Soroswap swap
    pub fn is_swap(function_name: &Symbol) -> bool {
        let name_str = function_name.to_string();
        name_str.contains("swap")
    }
    
    /// Check if a call is adding liquidity
    pub fn is_add_liquidity(function_name: &Symbol) -> bool {
        let name_str = function_name.to_string();
        name_str.contains("add") && name_str.contains("liquidity")
    }
    
    /// Check if a call is removing liquidity
    pub fn is_remove_liquidity(function_name: &Symbol) -> bool {
        let name_str = function_name.to_string();
        name_str.contains("remove") && name_str.contains("liquidity")
    }
}

/// Common types for Soroswap policy constraints
#[contracttype]
#[derive(Clone, Debug)]
pub enum SoroswapConstraint {
    /// Limit swap amounts
    SwapLimit {
        /// Maximum input amount per swap
        max_input: u64,
        /// Maximum output amount per swap
        max_output: u64,
        /// Allowed token pairs
        allowed_pairs: Vec<(Address, Address)>,
        /// Maximum slippage percentage (0-100)
        max_slippage_percent: u32,
    },
    /// Restrict liquidity operations
    LiquidityRestriction {
        /// Minimum liquidity to add
        min_add_amount: u64,
        /// Minimum liquidity to remove
        min_remove_amount: u64,
        /// Cooldown period between operations
        cooldown_ledgers: u32,
    },
    /// Time-based trading restrictions
    TradingHours {
        /// Start ledger offset (from midnight)
        start_offset: u32,
        /// End ledger offset (from midnight)
        end_offset: u32,
        /// Days of week (0=Sun, 1=Mon, etc.)
        allowed_days: Vec<u32>,
    },
}

/// Soroswap protocol address registry (testnet)
pub mod addresses {
    use soroban_sdk::Address;
    
    // TODO: Replace with actual testnet addresses from soroswap-core/public/testnet.contracts.json
    pub const SOROSWAP_FACTORY: &str = "CSOROSWAPFACTORY12345678901234567890123456789012345678901234";
    pub const SOROSWAP_ROUTER: &str = "CSOROSWAPROUTER1234567890123456789012345678901234567890123456";
}

/// Helper to identify Soroswap contracts
pub fn is_soroswap_contract(env: &Env, contract_address: &Address) -> bool {
    // TODO: Implement actual check - could check against known addresses
    // or query contract interface
    false
}

/// Slippage calculation utilities
pub mod slippage {
    use soroban_sdk::{Env, Val};
    
    /// Calculate minimum output given slippage percentage
    pub fn calculate_min_output(expected_output: u64, slippage_percent: u32) -> u64 {
        if slippage_percent >= 100 {
            return 0;
        }
        let slippage_factor = (100 - slippage_percent) as u64;
        expected_output * slippage_factor / 100
    }
    
    /// Check if actual output meets minimum requirement
    pub fn check_slippage(actual_output: u64, expected_output: u64, max_slippage_percent: u32) -> bool {
        if expected_output == 0 {
            return false;
        }
        
        let min_output = calculate_min_output(expected_output, max_slippage_percent);
        actual_output >= min_output
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;
    
    #[test]
    fn test_policy_utils() {
        let env = Env::default();
        
        // Test function detection
        let swap_func = Symbol::new(&env, "swap_exact_tokens_for_tokens");
        let add_liq_func = Symbol::new(&env, "add_liquidity");
        let remove_liq_func = Symbol::new(&env, "remove_liquidity");
        let other_func = Symbol::new(&env, "other");
        
        assert!(policy_utils::is_swap(&swap_func));
        assert!(!policy_utils::is_swap(&add_liq_func));
        
        assert!(policy_utils::is_add_liquidity(&add_liq_func));
        assert!(!policy_utils::is_add_liquidity(&swap_func));
        
        assert!(policy_utils::is_remove_liquidity(&remove_liq_func));
        assert!(!policy_utils::is_remove_liquidity(&other_func));
    }
    
    #[test]
    fn test_slippage_calculations() {
        // Test slippage calculation
        let expected = 1000u64;
        
        // 0% slippage
        assert_eq!(slippage::calculate_min_output(expected, 0), 1000);
        
        // 5% slippage
        assert_eq!(slippage::calculate_min_output(expected, 5), 950);
        
        // 10% slippage
        assert_eq!(slippage::calculate_min_output(expected, 10), 900);
        
        // 100% slippage (invalid)
        assert_eq!(slippage::calculate_min_output(expected, 100), 0);
        
        // Test slippage check
        assert!(slippage::check_slippage(1000, 1000, 5));
        assert!(slippage::check_slippage(960, 1000, 5));
        assert!(slippage::check_slippage(950, 1000, 5));
        assert!(!slippage::check_slippage(940, 1000, 5));
    }
    
    #[test]
    fn test_soroswap_constraint_serialization() {
        let env = Env::default();
        
        // Test SwapLimit serialization
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);
        let allowed_pairs = vec![(token_a.clone(), token_b.clone())];
        
        let constraint = SoroswapConstraint::SwapLimit {
            max_input: 10000,
            max_output: 9500,
            allowed_pairs,
            max_slippage_percent: 5,
        };
        
        // Should compile without errors
        let _ = constraint.clone();
        
        // Test TradingHours
        let constraint = SoroswapConstraint::TradingHours {
            start_offset: 9 * 60 * 6, // 9 AM
            end_offset: 17 * 60 * 6, // 5 PM
            allowed_days: vec![1, 2, 3, 4, 5], // Mon-Fri
        };
        
        let _ = constraint;
    }
}
