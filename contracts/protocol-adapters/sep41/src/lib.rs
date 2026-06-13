//! SEP-41 token standard adapter for policy contracts
//!
//! This module provides interfaces for SEP-41 token contracts
//! (Stellar Asset Contract standard) for use in policy contracts.

#![no_std]

use soroban_sdk::{contracttype, Address, Env, Symbol, Val};

/// SEP-41 token interface functions
pub mod functions {
    use soroban_sdk::Symbol;
    
    pub const TRANSFER: Symbol = Symbol::new("transfer");
    pub const TRANSFER_FROM: Symbol = Symbol::new("transfer_from");
    pub const APPROVE: Symbol = Symbol::new("approve");
    pub const ALLOWANCE: Symbol = Symbol::new("allowance");
    pub const BALANCE: Symbol = Symbol::new("balance");
    pub const DECIMALS: Symbol = Symbol::new("decimals");
    pub const NAME: Symbol = Symbol::new("name");
    pub const SYMBOL: Symbol =Symbol::new("symbol");
}

/// SEP-41-specific policy utilities
pub mod policy_utils {
    use super::*;
    
    /// Extract function name from SEP-41 token call context
    pub fn extract_sep41_function(auth_context: &Val) -> Option<Symbol> {
        // TODO: Implement actual extraction from auth_context
        // For now, return common SEP-41 functions
        Some(functions::TRANSFER)
    }
    
    /// Check if a call is a token transfer
    pub fn is_transfer(function_name: &Symbol) -> bool {
        function_name == &functions::TRANSFER || function_name == &functions::TRANSFER_FROM
    }
    
    /// Check if a call is an approval
    pub fn is_approval(function_name: &Symbol) -> bool {
        function_name == &functions::APPROVE
    }
    
    /// Check if a call is a balance/allowance query
    pub fn is_query(function_name: &Symbol) -> bool {
        function_name == &functions::BALANCE || function_name == &functions::ALLOWANCE ||
        function_name == &functions::DECIMALS || function_name == &functions::NAME ||
        function_name == &functions::SYMBOL
    }
}

/// Common types for SEP-41 policy constraints
#[contracttype]
#[derive(Clone, Debug)]
pub enum Sep41Constraint {
    /// Transfer limits
    TransferLimit {
        /// Maximum transfer amount per transaction
        max_amount: u64,
        /// Daily transfer limit
        daily_limit: u64,
        /// Recipient restrictions
        allowed_recipients: Vec<Address>,
        /// Whether to allow transfers to any address
        allow_any_recipient: bool,
    },
    /// Approval limits
    ApprovalLimit {
        /// Maximum approval amount
        max_approval: u64,
        /// Maximum allowance per spender
        max_allowance: u64,
        /// Approved spenders
        allowed_spenders: Vec<Address>,
    },
    /// Subscription billing constraints
    SubscriptionBilling {
        /// Fixed billing amount
        billing_amount: u64,
        /// Billing period in ledgers
        billing_period: u32,
        /// Allowed recipient (subscription service)
        service_address: Address,
        /// Maximum number of billing cycles
        max_cycles: Option<u32>,
    },
}

/// Known SEP-41 token addresses (testnet)
pub mod addresses {
    use soroban_sdk::Address;
    
    // USDC testnet SAC
    pub const USDC: &str = "CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA";
    
    // TODO: Add more testnet tokens
}

/// Helper to identify SEP-41 token contracts
pub fn is_sep41_token(env: &Env, contract_address: &Address) -> bool {
    // TODO: Implement actual check - could try to call standard functions
    // or check against known addresses
    false
}

/// Subscription billing utilities
pub mod subscription {
    use soroban_sdk::{Address, Env};
    
    /// Calculate next billing time
    pub fn next_billing_time(last_billed: u32, billing_period: u32) -> u32 {
        last_billed + billing_period
    }
    
    /// Check if billing is due
    pub fn is_billing_due(current_ledger: u32, last_billed: u32, billing_period: u32) -> bool {
        current_ledger >= next_billing_time(last_billed, billing_period)
    }
    
    /// Calculate remaining billing cycles
    pub fn remaining_cycles(total_cycles: u32, used_cycles: u32) -> Option<u32> {
        if used_cycles >= total_cycles {
            Some(0)
        } else {
            Some(total_cycles - used_cycles)
        }
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
        let transfer_func = functions::TRANSFER;
        let transfer_from_func = functions::TRANSFER_FROM;
        let approve_func = functions::APPROVE;
        let balance_func = functions::BALANCE;
        let other_func = Symbol::new(&env, "other");
        
        assert!(policy_utils::is_transfer(&transfer_func));
        assert!(policy_utils::is_transfer(&transfer_from_func));
        assert!(!policy_utils::is_transfer(&approve_func));
        
        assert!(policy_utils::is_approval(&approve_func));
        assert!(!policy_utils::is_approval(&transfer_func));
        
        assert!(policy_utils::is_query(&balance_func));
        assert!(!policy_utils::is_query(&transfer_func));
        assert!(!policy_utils::is_query(&other_func));
    }
    
    #[test]
    fn test_subscription_utilities() {
        // Test next billing time
        assert_eq!(subscription::next_billing_time(1000, 100), 1100);
        assert_eq!(subscription::next_billing_time(0, 1000), 1000);
        
        // Test billing due check
        assert!(subscription::is_billing_due(1100, 1000, 100));
        assert!(subscription::is_billing_due(1200, 1000, 100));
        assert!(!subscription::is_billing_due(1050, 1000, 100));
        assert!(!subscription::is_billing_due(1099, 1000, 100));
        
        // Test remaining cycles
        assert_eq!(subscription::remaining_cycles(10, 0), Some(10));
        assert_eq!(subscription::remaining_cycles(10, 5), Some(5));
        assert_eq!(subscription::remaining_cycles(10, 9), Some(1));
        assert_eq!(subscription::remaining_cycles(10, 10), Some(0));
    }
    
    #[test]
    fn test_sep41_constraint_serialization() {
        let env = Env::default();
        
        // Test TransferLimit serialization
        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);
        let allowed_recipients = vec![recipient1.clone(), recipient2.clone()];
        
        let constraint = Sep41Constraint::TransferLimit {
            max_amount: 1000,
            daily_limit: 5000,
            allowed_recipients,
            allow_any_recipient: false,
        };
        
        // Should compile without errors
        let _ = constraint.clone();
        
        // Test SubscriptionBilling
        let service_addr = Address::generate(&env);
        let constraint = Sep41Constraint::SubscriptionBilling {
            billing_amount: 100,
            billing_period: 30 * 24 * 60 * 6, // ~30 days
            service_address: service_addr,
            max_cycles: Some(12), // 1 year subscription
        };
        
        let _ = constraint;
    }
}
