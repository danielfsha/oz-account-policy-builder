//! Integration tests for spending-limit policy contract

use spending_limit::{InstallParams, SpendingLimitPolicy, SpendingState};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_complete_install_uninstall_cycle() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 123;
    let asset_id = Address::generate(&env);
    
    let policy = SpendingLimitPolicy;
    
    // Test 1: Install with valid parameters
    let params = InstallParams {
        cap_amount: 1000,
        asset_id: asset_id.clone(),
        window_ledgers: 1000,
        allow_partial: true,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed with valid params");
    
    // Test 2: Verify parameters stored correctly
    let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve stored params");
    assert_eq!(stored_params.cap_amount, params.cap_amount);
    assert_eq!(stored_params.asset_id, params.asset_id);
    assert_eq!(stored_params.window_ledgers, params.window_ledgers);
    assert_eq!(stored_params.allow_partial, params.allow_partial);
    
    // Test 3: Verify state initialized correctly
    let state = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve state");
    assert_eq!(state.spent_amount, 0);
    assert_eq!(state.window_start, env.ledger().sequence());
    
    // Test 4: Uninstall
    let uninstall_result = policy.uninstall(env.clone(), account.clone(), context_rule_id);
    assert!(uninstall_result.is_ok(), "Uninstall should succeed");
    
    // Test 5: Verify state cleaned up
    let state_result = policy.get_state(env.clone(), account.clone(), context_rule_id);
    assert!(state_result.is_err(), "State should be cleared after uninstall");
    assert_eq!(state_result.unwrap_err(), spending_limit::PolicyError::NotFound);
}

#[test]
fn test_parameter_validation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 456;
    let asset_id = Address::generate(&env);
    
    let policy = SpendingLimitPolicy;
    
    // Test 1: Zero cap amount should fail
    let invalid_params1 = InstallParams {
        cap_amount: 0,
        asset_id: asset_id.clone(),
        window_ledgers: 1000,
        allow_partial: true,
    };
    
    let result1 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params1);
    assert!(result1.is_err(), "Zero cap amount should fail validation");
    assert_eq!(result1.unwrap_err(), spending_limit::PolicyError::InvalidParams);
    
    // Test 2: Zero window ledgers should fail
    let invalid_params2 = InstallParams {
        cap_amount: 1000,
        asset_id: asset_id.clone(),
        window_ledgers: 0,
        allow_partial: true,
    };
    
    let result2 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params2);
    assert!(result2.is_err(), "Zero window ledgers should fail validation");
    assert_eq!(result2.unwrap_err(), spending_limit::PolicyError::InvalidParams);
    
    // Test 3: Valid parameters should succeed
    let valid_params = InstallParams {
        cap_amount: 1000,
        asset_id,
        window_ledgers: 1000,
        allow_partial: false,
    };
    
    let result3 = policy.install(env.clone(), account.clone(), context_rule_id, valid_params);
    assert!(result3.is_ok(), "Valid params should succeed");
}

#[test]
fn test_multiple_accounts_isolation() {
    let env = Env::default();
    let account1 = Address::generate(&env);
    let account2 = Address::generate(&env);
    let context_rule_id = 789;
    let asset_id = Address::generate(&env);
    
    let policy = SpendingLimitPolicy;
    
    // Install for account1
    let params1 = InstallParams {
        cap_amount: 1000,
        asset_id: asset_id.clone(),
        window_ledgers: 1000,
        allow_partial: true,
    };
    
    policy.install(env.clone(), account1.clone(), context_rule_id, params1.clone())
        .expect("Install for account1 should succeed");
    
    // Install for account2 with different params
    let params2 = InstallParams {
        cap_amount: 2000,
        asset_id: asset_id.clone(),
        window_ledgers: 2000,
        allow_partial: false,
    };
    
    policy.install(env.clone(), account2.clone(), context_rule_id, params2.clone())
        .expect("Install for account2 should succeed");
    
    // Verify each account has its own params
    let stored_params1 = policy.get_params(env.clone(), account1.clone(), context_rule_id)
        .expect("Should get account1 params");
    assert_eq!(stored_params1.cap_amount, params1.cap_amount);
    assert_eq!(stored_params1.window_ledgers, params1.window_ledgers);
    
    let stored_params2 = policy.get_params(env.clone(), account2.clone(), context_rule_id)
        .expect("Should get account2 params");
    assert_eq!(stored_params2.cap_amount, params2.cap_amount);
    assert_eq!(stored_params2.window_ledgers, params2.window_ledgers);
    
    // Verify states are separate
    let state1 = policy.get_state(env.clone(), account1.clone(), context_rule_id)
        .expect("Should get account1 state");
    let state2 = policy.get_state(env.clone(), account2.clone(), context_rule_id)
        .expect("Should get account2 state");
    
    assert_eq!(state1.spent_amount, 0);
    assert_eq!(state2.spent_amount, 0);
}

#[test]
fn test_policy_trait_implementation() {
    let env = Env::default();
    let policy = SpendingLimitPolicy;
    
    // Test weight method
    let weight = policy.weight();
    assert!(weight > 0, "Policy should have positive weight");
    
    // Test enforce method (placeholder implementation)
    // Note: This is a simplified test since enforce is a placeholder
    let result = policy.enforce(
        env,
        stellar_accounts::smart_account::ContextRule::default(),
        stellar_accounts::smart_account::Signer::default(),
        soroban_sdk::Vec::new(&Env::default()),
        soroban_sdk::Val::default(),
    );
    assert!(result.is_ok(), "Placeholder enforce should succeed");
}

#[test]
fn test_storage_keys_scoping() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 999;
    let asset_id = Address::generate(&env);
    
    let policy = SpendingLimitPolicy;
    
    // Install policy
    let params = InstallParams {
        cap_amount: 1000,
        asset_id,
        window_ledgers: 1000,
        allow_partial: true,
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params)
        .expect("Install should succeed");
    
    // Test that storage keys are properly scoped
    // This is an indirect test - we verify that operations work correctly
    // which implies storage keys are working properly
    
    let state = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should get state");
    
    // Modify ledger sequence and check window logic would work
    env.ledger().with_sequence(state.window_start + 500);
    
    // Re-fetch state to ensure it's still accessible
    let state_after = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should still get state after ledger change");
    assert_eq!(state_after.window_start, state.window_start, "Window start should not change without policy enforcement");
}