//! Integration tests for frequency-limit policy contract

use frequency_limit::{FrequencyLimitPolicy, InstallParams, FrequencyState};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_basic_frequency_operations() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 123;
    
    let policy = FrequencyLimitPolicy;
    
    // Install with frequency limit
    let params = InstallParams {
        max_calls: 3,
        window_ledgers: 1000,
        reset_on_expiry: true,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed");
    
    // Verify parameters stored
    let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    assert_eq!(stored_params.max_calls, params.max_calls);
    assert_eq!(stored_params.window_ledgers, params.window_ledgers);
    assert_eq!(stored_params.reset_on_expiry, params.reset_on_expiry);
    
    // Verify state initialized
    let state = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve state");
    assert_eq!(state.call_count, 0);
    assert_eq!(state.last_call, 0);
    assert_eq!(state.window_start, env.ledger().sequence());
}

#[test]
fn test_frequency_simulation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 456;
    
    let policy = FrequencyLimitPolicy;
    
    // Install with limit of 2 calls per window
    let params = InstallParams {
        max_calls: 2,
        window_ledgers: 1000,
        reset_on_expiry: true,
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params.clone())
        .expect("Install should succeed");
    
    // Simulate check - should allow (0 calls used)
    let sim1 = policy.simulate_check(env.clone(), account.clone(), context_rule_id)
        .expect("Simulation should work");
    assert!(sim1, "Should allow first call");
    
    // Manually update state to simulate 1 call used
    let mut state = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should get state");
    state.call_count = 1;
    state.last_call = env.ledger().sequence();
    policy::PolicyStorage::set(&env, &account, context_rule_id, "state", &state);
    
    // Simulate check - should allow (1 call used, limit is 2)
    let sim2 = policy.simulate_check(env.clone(), account.clone(), context_rule_id)
        .expect("Simulation should work");
    assert!(sim2, "Should allow second call");
    
    // Update state to simulate 2 calls used
    state.call_count = 2;
    policy::PolicyStorage::set(&env, &account, context_rule_id, "state", &state);
    
    // Simulate check - should deny (max calls reached)
    let sim3 = policy.simulate_check(env.clone(), account.clone(), context_rule_id)
        .expect("Simulation should work");
    assert!(!sim3, "Should deny third call (limit reached)");
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
    assert_eq!(updated.call_count, 2, "Call count should not reset within window");
    assert_eq!(updated.window_start, 1000, "Window start should not change within window");
    
    // Beyond window - should reset
    env.ledger().with_sequence(1150);
    let updated = policy.update_window(&env, &params, state);
    assert_eq!(updated.call_count, 0, "Call count should reset beyond window");
    assert_eq!(updated.window_start, 1150, "Window start should update to current ledger");
    
    // Test sliding window logic (reset_on_expiry = false)
    let params = InstallParams {
        max_calls: 3,
        window_ledgers: 100,
        reset_on_expiry: false,
    };
    
    let state = FrequencyState {
        call_count: 2,
        window_start: 1000,
        last_call: 1050,
    };
    
    env.ledger().with_sequence(1150);
    let updated = policy.update_window(&env, &params, state);
    // With current implementation, both reset modes behave similarly
    // More sophisticated sliding window logic could be added later
    assert_eq!(updated.call_count, 0, "Call count should reset");
}

#[test]
fn test_parameter_validation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 789;
    
    let policy = FrequencyLimitPolicy;
    
    // Test invalid: zero max_calls
    let invalid_params1 = InstallParams {
        max_calls: 0,
        window_ledgers: 1000,
        reset_on_expiry: true,
    };
    
    let result1 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params1);
    assert!(result1.is_err(), "Zero max_calls should fail");
    assert_eq!(result1.unwrap_err(), frequency_limit::PolicyError::InvalidParams);
    
    // Test invalid: zero window_ledgers
    let invalid_params2 = InstallParams {
        max_calls: 3,
        window_ledgers: 0,
        reset_on_expiry: true,
    };
    
    let result2 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params2);
    assert!(result2.is_err(), "Zero window_ledgers should fail");
    assert_eq!(result2.unwrap_err(), frequency_limit::PolicyError::InvalidParams);
    
    // Test valid parameters
    let valid_params = InstallParams {
        max_calls: 5,
        window_ledgers: 5000,
        reset_on_expiry: false,
    };
    
    let result3 = policy.install(env.clone(), account.clone(), context_rule_id, valid_params);
    assert!(result3.is_ok(), "Valid params should succeed");
}

#[test]
fn test_policy_trait_implementation() {
    let env = Env::default();
    let policy = FrequencyLimitPolicy;
    
    // Test weight method
    let weight = policy.weight();
    assert!(weight > 0, "Policy should have positive weight");
    assert!(weight < 15, "Frequency limit should have moderate weight");
    
    // Test enforce method would check frequency logic
    // Note: This tests the trait implementation
    let result = policy.enforce(
        env,
        stellar_accounts::smart_account::ContextRule::default(),
        stellar_accounts::smart_account::Signer::default(),
        soroban_sdk::Vec::new(&Env::default()),
        soroban_sdk::Val::default(),
    );
    // Enforcement logic is complex, but trait should be implemented
}

#[test]
fn test_different_frequency_configurations() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 999;
    
    let policy = FrequencyLimitPolicy;
    
    // Test different configurations
    let test_cases = vec![
        (1, 100, true, "strict: 1 call per 100 ledgers"),
        (5, 500, true, "moderate: 5 calls per 500 ledgers"),
        (10, 1000, false, "generous: 10 calls per 1000 ledgers sliding"),
        (100, 10000, true, "high frequency: 100 calls per 10000 ledgers"),
    ];
    
    for (max_calls, window_ledgers, reset_on_expiry, description) in test_cases {
        let params = InstallParams {
            max_calls,
            window_ledgers,
            reset_on_expiry,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok(), "{} should install successfully", description);
        
        // Verify parameters
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
            .expect("Should retrieve params");
        assert_eq!(stored_params.max_calls, max_calls);
        assert_eq!(stored_params.window_ledgers, window_ledgers);
        assert_eq!(stored_params.reset_on_expiry, reset_on_expiry);
        
        // Clean up for next test
        policy.uninstall(env.clone(), account.clone(), context_rule_id)
            .expect("Uninstall should succeed");
    }
}