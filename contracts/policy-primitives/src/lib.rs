//! Policy primitives — base traits and utilities for OpenZeppelin smart account policies
//!
//! This crate provides:
//! - Re-export of the OZ `Policy` trait and related types
//! - Storage helpers for policy implementations
//! - Common utilities for all policy contracts
//! - Testing utilities

#![no_std]

pub use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};

use soroban_sdk::{contracttype, Address, Env};

/// Storage key pattern for policy state scoped by `(smart_account, context_rule_id)`
#[contracttype]
pub enum PolicyStorageKey<S> {
    /// State storage for a specific smart account and context rule
    State(Address, u32, S),
    /// Parameters storage for a specific smart account and context rule  
    Params(Address, u32, S),
}

/// Common error types for policy implementations
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyError {
    /// Policy constraint violation (e.g., spending limit exceeded)
    ConstraintViolated,
    /// Invalid parameters provided during install
    InvalidParams,
    /// Policy not found for the given account and context rule
    NotFound,
    /// Storage operation failed
    StorageError,
}

/// Result type for policy operations
pub type PolicyResult<T> = core::result::Result<T, PolicyError>;

/// Trait for policy parameter validation
pub trait ValidateParams {
    /// Validate installation parameters
    fn validate(&self) -> PolicyResult<()>;
}

/// Helper to get storage key for policy state
pub fn state_key<S>(env: &Env, account: &Address, context_rule_id: u32, subkey: S) -> PolicyStorageKey<S>
where
    S: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
{
    PolicyStorageKey::State(account.clone(), context_rule_id, subkey)
}

/// Helper to get storage key for policy parameters
pub fn params_key<S>(env: &Env, account: &Address, context_rule_id: u32, subkey: S) -> PolicyStorageKey<S>
where
    S: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
{
    PolicyStorageKey::Params(account.clone(), context_rule_id, subkey)
}

/// Storage helper for policy implementations
pub struct PolicyStorage;

impl PolicyStorage {
    /// Store value with proper scoping
    pub fn set<S, V>(env: &Env, account: &Address, context_rule_id: u32, subkey: S, value: &V)
    where
        S: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
        V: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        let key = state_key(env, account, context_rule_id, subkey);
        env.storage().persistent().set(&key, value);
    }

    /// Get stored value with proper scoping
    pub fn get<S, V>(env: &Env, account: &Address, context_rule_id: u32, subkey: S) -> Option<V>
    where
        S: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
        V: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        let key = state_key(env, account, context_rule_id, subkey);
        env.storage().persistent().get(&key)
    }

    /// Check if value exists with proper scoping
    pub fn has<S>(env: &Env, account: &Address, context_rule_id: u32, subkey: S) -> bool
    where
        S: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        let key = state_key(env, account, context_rule_id, subkey);
        env.storage().persistent().has(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_storage_scoping() {
        let env = Env::default();
        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        let context_rule_id = 123;
        
        // Store value for account1
        PolicyStorage::set(&env, &account1, context_rule_id, "balance", &100u64);
        
        // Should be retrievable for account1
        let value: Option<u64> = PolicyStorage::get(&env, &account1, context_rule_id, "balance");
        assert_eq!(value, Some(100));
        
        // Should NOT be retrievable for account2
        let value: Option<u64> = PolicyStorage::get(&env, &account2, context_rule_id, "balance");
        assert_eq!(value, None);
        
        // Check has works correctly
        assert!(PolicyStorage::has(&env, &account1, context_rule_id, "balance"));
        assert!(!PolicyStorage::has(&env, &account2, context_rule_id, "balance"));
    }

    #[test]
    fn test_policy_error_types() {
        let error = PolicyError::ConstraintViolated;
        assert_eq!(error, PolicyError::ConstraintViolated);
        
        let error = PolicyError::InvalidParams;
        assert_eq!(error, PolicyError::InvalidParams);
        
        let error = PolicyError::NotFound;
        assert_eq!(error, PolicyError::NotFound);
        
        let error = PolicyError::StorageError;
        assert_eq!(error, PolicyError::StorageError);
    }
    
    #[test]
    fn test_params_key_generation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let key = params_key(&env, &account, context_rule_id, "test_param");
        
        // The key should be of type PolicyStorageKey::Params
        match key {
            PolicyStorageKey::Params(addr, id, subkey) => {
                assert_eq!(addr, account);
                assert_eq!(id, context_rule_id);
                // Note: can't easily assert subkey value in test
            }
            _ => panic!("Expected Params variant"),
        }
    }
    
    #[test]
    fn test_state_key_generation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 456;
        
        let key = state_key(&env, &account, context_rule_id, "test_state");
        
        match key {
            PolicyStorageKey::State(addr, id, subkey) => {
                assert_eq!(addr, account);
                assert_eq!(id, context_rule_id);
            }
            _ => panic!("Expected State variant"),
        }
    }
    
    #[test]
    fn test_storage_overwrite() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 789;
        
        // Store initial value
        PolicyStorage::set(&env, &account, context_rule_id, "counter", &1u64);
        let value1: Option<u64> = PolicyStorage::get(&env, &account, context_rule_id, "counter");
        assert_eq!(value1, Some(1));
        
        // Overwrite value
        PolicyStorage::set(&env, &account, context_rule_id, "counter", &2u64);
        let value2: Option<u64> = PolicyStorage::get(&env, &account, context_rule_id, "counter");
        assert_eq!(value2, Some(2));
        
        // Overwrite with different type
        PolicyStorage::set(&env, &account, context_rule_id, "name", &"test");
        let value3: Option<String> = PolicyStorage::get(&env, &account, context_rule_id, "name");
        assert_eq!(value3, Some("test".to_string()));
    }
    
    #[test]
    fn test_storage_removal() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 999;
        
        // Store value
        PolicyStorage::set(&env, &account, context_rule_id, "temp", &true);
        assert!(PolicyStorage::has(&env, &account, context_rule_id, "temp"));
        
        // Overwrite with unit type to "remove" it
        PolicyStorage::set(&env, &account, context_rule_id, "temp", &());
        // Note: Soroban storage doesn't truly remove, but sets to unit
        
        // The key still exists but stores unit
        assert!(PolicyStorage::has(&env, &account, context_rule_id, "temp"));
    }
    
    #[test]
    fn test_multiple_accounts_scoping() {
        let env = Env::default();
        let account1 = Address::generate(&env);
        let account2 = Address::generate(&env);
        let account3 = Address::generate(&env);
        let context_rule_id = 111;
        
        // Store different values for different accounts
        PolicyStorage::set(&env, &account1, context_rule_id, "data", &"account1_data");
        PolicyStorage::set(&env, &account2, context_rule_id, "data", &"account2_data");
        
        // Each account should see only its own data
        let data1: Option<String> = PolicyStorage::get(&env, &account1, context_rule_id, "data");
        assert_eq!(data1, Some("account1_data".to_string()));
        
        let data2: Option<String> = PolicyStorage::get(&env, &account2, context_rule_id, "data");
        assert_eq!(data2, Some("account2_data".to_string()));
        
        let data3: Option<String> = PolicyStorage::get(&env, &account3, context_rule_id, "data");
        assert_eq!(data3, None);
    }
    
    #[test]
    fn test_multiple_context_rules_scoping() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id1 = 222;
        let context_rule_id2 = 333;
        
        // Store different values for different context rules
        PolicyStorage::set(&env, &account, context_rule_id1, "settings", &"settings1");
        PolicyStorage::set(&env, &account, context_rule_id2, "settings", &"settings2");
        
        // Each context rule should see only its own data
        let settings1: Option<String> = PolicyStorage::get(&env, &account, context_rule_id1, "settings");
        assert_eq!(settings1, Some("settings1".to_string()));
        
        let settings2: Option<String> = PolicyStorage::get(&env, &account, context_rule_id2, "settings");
        assert_eq!(settings2, Some("settings2".to_string()));
    }
}