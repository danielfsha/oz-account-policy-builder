//! Integration tests for time-window policy contract

use time_window::{InstallParams, TimeWindowPolicy, TimeWindowState, WindowType};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_window_type_calculations() {
    let policy = TimeWindowPolicy;
    
    // Test daily window size
    let params = InstallParams {
        window_type: WindowType::Daily,
        offset_ledgers: 0,
    };
    let window_size = policy.window_size(&params);
    assert_eq!(window_size, 24 * 60 * 6); // 24h * 60min * 6 ledgers/min
    
    // Test weekly window size  
    let params = InstallParams {
        window_type: WindowType::Weekly,
        offset_ledgers: 0,
    };
    let window_size = policy.window_size(&params);
    assert_eq!(window_size, 7 * 24 * 60 * 6); // 7 days
    
    // Test monthly window size
    let params = InstallParams {
        window_type: WindowType::Monthly,
        offset_ledgers: 0,
    };
    let window_size = policy.window_size(&params);
    assert_eq!(window_size, 30 * 24 * 60 * 6); // 30 days
    
    // Test custom window size
    let params = InstallParams {
        window_type: WindowType::Custom(5000),
        offset_ledgers: 0,
    };
    let window_size = policy.window_size(&params);
    assert_eq!(window_size, 5000);
}

#[test]
fn test_install_and_state_management() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 123;
    
    let policy = TimeWindowPolicy;
    
    // Install with daily window
    let params = InstallParams {
        window_type: WindowType::Daily,
        offset_ledgers: 100,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed");
    
    // Verify parameters stored
    let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    match (stored_params.window_type, params.window_type) {
        (WindowType::Daily, WindowType::Daily) => (),
        _ => panic!("Window type mismatch"),
    }
    assert_eq!(stored_params.offset_ledgers, params.offset_ledgers);
    
    // Verify state initialized
    let state = policy.get_state(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve state");
    assert_eq!(state.last_execution, 0, "Initial state should have no executions");
    
    // Test simulation
    env.ledger().with_sequence(50); // Before offset
    let sim_before = policy.simulate_check(env.clone(), account.clone(), context_rule_id)
        .expect("Simulation should work");
    assert!(!sim_before, "Should fail before offset");
    
    env.ledger().with_sequence(150); // After offset
    let sim_after = policy.simulate_check(env.clone(), account.clone(), context_rule_id)
        .expect("Simulation should work");
    assert!(sim_after, "Should pass after offset");
}

#[test]
fn test_window_enforcement_logic() {
    let env = Env::default();
    let policy = TimeWindowPolicy;
    
    // Test first execution (no previous execution)
    let params = InstallParams {
        window_type: WindowType::Custom(100),
        offset_ledgers: 50,
    };
    
    let state = TimeWindowState {
        last_execution: 0,
    };
    
    env.ledger().with_sequence(40); // Before offset
    assert!(!policy.is_within_window(&env, &params, &state));
    
    env.ledger().with_sequence(60); // After offset
    assert!(policy.is_within_window(&env, &params, &state));
    
    // Test subsequent executions
    let state = TimeWindowState {
        last_execution: 1000,
    };
    
    env.ledger().with_sequence(1050); // Within same window (window size = 100)
    assert!(!policy.is_within_window(&env, &params, &state));
    
    env.ledger().with_sequence(1101); // In next window (1000 + 100 + 1)
    assert!(policy.is_within_window(&env, &params, &state));
}

#[test]
fn test_parameter_validation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 456;
    
    let policy = TimeWindowPolicy;
    
    // Test invalid custom window (zero ledgers)
    let invalid_params = InstallParams {
        window_type: WindowType::Custom(0),
        offset_ledgers: 0,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params);
    assert!(result.is_err(), "Zero custom window should fail");
    assert_eq!(result.unwrap_err(), time_window::PolicyError::InvalidParams);
    
    // Test valid custom window
    let valid_params = InstallParams {
        window_type: WindowType::Custom(1000),
        offset_ledgers: 500,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, valid_params);
    assert!(result.is_ok(), "Valid custom window should succeed");
}

#[test]
fn test_policy_trait_implementation() {
    let env = Env::default();
    let policy = TimeWindowPolicy;
    
    // Test weight method
    let weight = policy.weight();
    assert!(weight > 0, "Policy should have positive weight");
    assert!(weight < 15, "Time window should have lower weight than spending limit");
    
    // Test enforce method would check window logic
    // Note: This tests the trait implementation, not actual enforcement
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
fn test_multiple_window_types() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 789;
    
    let policy = TimeWindowPolicy;
    
    // Test with different window types
    let test_cases = vec![
        (WindowType::Daily, "daily"),
        (WindowType::Weekly, "weekly"),
        (WindowType::Monthly, "monthly"),
        (WindowType::Custom(500), "custom"),
    ];
    
    for (window_type, description) in test_cases {
        let params = InstallParams {
            window_type: window_type.clone(),
            offset_ledgers: 0,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok(), "{} window should install successfully", description);
        
        // Clean up for next test
        policy.uninstall(env.clone(), account.clone(), context_rule_id)
            .expect("Uninstall should succeed");
    }
}