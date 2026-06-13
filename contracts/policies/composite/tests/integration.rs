//! Integration tests for composite policy contract

use composite::{CompositePolicy, InstallParams, PolicyReference};
use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, Vec};

#[test]
fn test_basic_composite_operations() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 123;
    
    let policy = CompositePolicy;
    
    // Create policy references
    let policy1_addr = Address::generate(&env);
    let policy2_addr = Address::generate(&env);
    
    let policy1_ref = PolicyReference {
        policy_address: policy1_addr.clone(),
        install_data: Bytes::from_array(&env, &[1, 2, 3, 4]),
    };
    
    let policy2_ref = PolicyReference {
        policy_address: policy2_addr.clone(),
        install_data: Bytes::from_array(&env, &[5, 6, 7, 8]),
    };
    
    let mut policies = Vec::new(&env);
    policies.push_back(policy1_ref.clone());
    policies.push_back(policy2_ref.clone());
    
    // Install composite policy
    let params = InstallParams {
        policies: policies.clone(),
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed");
    
    // Verify parameters stored
    let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    assert_eq!(stored_params.policies.len(), 2);
    
    // Verify individual policy references
    let stored_ref1 = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 0)
        .expect("Should get policy reference 0");
    assert_eq!(stored_ref1.policy_address, policy1_addr);
    assert_eq!(stored_ref1.install_data.to_vec(), vec![1, 2, 3, 4]);
    
    let stored_ref2 = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 1)
        .expect("Should get policy reference 1");
    assert_eq!(stored_ref2.policy_address, policy2_addr);
    assert_eq!(stored_ref2.install_data.to_vec(), vec![5, 6, 7, 8]);
}

#[test]
fn test_parameter_validation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 456;
    
    let policy = CompositePolicy;
    
    // Test invalid: empty policies list
    let empty_vec = Vec::new(&env);
    let invalid_params1 = InstallParams {
        policies: empty_vec,
    };
    
    let result1 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params1);
    assert!(result1.is_err(), "Empty policies list should fail");
    assert_eq!(result1.unwrap_err(), composite::PolicyError::InvalidParams);
    
    // Test invalid: too many policies (max is 5)
    let mut too_many_policies = Vec::new(&env);
    for i in 0..6 {
        let policy_ref = PolicyReference {
            policy_address: Address::generate(&env),
            install_data: Bytes::from_array(&env, &[i as u8]),
        };
        too_many_policies.push_back(policy_ref);
    }
    
    let invalid_params2 = InstallParams {
        policies: too_many_policies,
    };
    
    let result2 = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params2);
    assert!(result2.is_err(), "Too many policies should fail");
    assert_eq!(result2.unwrap_err(), composite::PolicyError::InvalidParams);
    
    // Test valid: exactly 5 policies (maximum allowed)
    let mut valid_policies = Vec::new(&env);
    for i in 0..5 {
        let policy_ref = PolicyReference {
            policy_address: Address::generate(&env),
            install_data: Bytes::from_array(&env, &[i as u8]),
        };
        valid_policies.push_back(policy_ref);
    }
    
    let valid_params = InstallParams {
        policies: valid_policies,
    };
    
    let result3 = policy.install(env.clone(), account.clone(), context_rule_id, valid_params);
    assert!(result3.is_ok(), "Exactly 5 policies should succeed");
}

#[test]
fn test_uninstall_cleans_up_all_storage() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 789;
    
    let policy = CompositePolicy;
    
    // Install with 3 policies
    let mut policies = Vec::new(&env);
    for i in 0..3 {
        let policy_ref = PolicyReference {
            policy_address: Address::generate(&env),
            install_data: Bytes::from_array(&env, &[i as u8]),
        };
        policies.push_back(policy_ref);
    }
    
    let params = InstallParams {
        policies: policies.clone(),
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params)
        .expect("Install should succeed");
    
    // Verify storage exists
    let params_before = policy.get_params(env.clone(), account.clone(), context_rule_id);
    assert!(params_before.is_ok(), "Should retrieve params before uninstall");
    
    for i in 0..3 {
        let ref_before = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, i);
        assert!(ref_before.is_ok(), "Should retrieve policy reference {} before uninstall", i);
    }
    
    // Uninstall
    let uninstall_result = policy.uninstall(env.clone(), account.clone(), context_rule_id);
    assert!(uninstall_result.is_ok(), "Uninstall should succeed");
    
    // Verify storage cleaned up
    let params_after = policy.get_params(env.clone(), account.clone(), context_rule_id);
    assert!(params_after.is_err(), "Params should be cleared after uninstall");
    assert_eq!(params_after.unwrap_err(), composite::PolicyError::NotFound);
    
    for i in 0..3 {
        let ref_after = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, i);
        assert!(ref_after.is_err(), "Policy reference {} should be cleared after uninstall", i);
        assert_eq!(ref_after.unwrap_err(), composite::PolicyError::NotFound);
    }
}

#[test]
fn test_policy_trait_implementation() {
    let env = Env::default();
    let policy = CompositePolicy;
    
    // Test weight method
    let weight = policy.weight();
    assert!(weight > 0, "Policy should have positive weight");
    assert!(weight >= 15, "Composite should have higher weight than individual policies");
    
    // Test enforce method
    // Note: This tests the trait implementation, not actual enforcement
    let result = policy.enforce(
        env,
        stellar_accounts::smart_account::ContextRule::default(),
        stellar_accounts::smart_account::Signer::default(),
        soroban_sdk::Vec::new(&Env::default()),
        soroban_sdk::Val::default(),
    );
    // Enforcement logic is placeholder, but trait should be implemented
}

#[test]
fn test_different_policy_counts() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 999;
    
    let policy = CompositePolicy;
    
    // Test different numbers of policies
    let test_cases = vec![1, 2, 3, 4, 5];
    
    for policy_count in test_cases {
        // Clean up from previous test
        policy.uninstall(env.clone(), account.clone(), context_rule_id).ok();
        
        // Create policies
        let mut policies = Vec::new(&env);
        for i in 0..policy_count {
            let policy_ref = PolicyReference {
                policy_address: Address::generate(&env),
                install_data: Bytes::from_array(&env, &[i as u8]),
            };
            policies.push_back(policy_ref);
        }
        
        let params = InstallParams {
            policies: policies.clone(),
        };
        
        // Install
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok(), "Install with {} policies should succeed", policy_count);
        
        // Verify
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
            .expect("Should retrieve params");
        assert_eq!(stored_params.policies.len(), policy_count);
        
        for i in 0..policy_count {
            let stored_ref = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, i)
                .expect("Should get policy reference");
            assert_eq!(stored_ref.install_data.to_vec(), vec![i as u8]);
        }
    }
}

#[test]
fn test_storage_isolation_by_account() {
    let env = Env::default();
    let account1 = Address::generate(&env);
    let account2 = Address::generate(&env);
    let context_rule_id = 111;
    
    let policy = CompositePolicy;
    
    // Install for account1
    let policy1_addr = Address::generate(&env);
    let policy1_ref = PolicyReference {
        policy_address: policy1_addr.clone(),
        install_data: Bytes::from_array(&env, &[1, 2, 3]),
    };
    
    let mut policies1 = Vec::new(&env);
    policies1.push_back(policy1_ref);
    
    let params1 = InstallParams {
        policies: policies1,
    };
    
    policy.install(env.clone(), account1.clone(), context_rule_id, params1)
        .expect("Install for account1 should succeed");
    
    // Install for account2 with different policy
    let policy2_addr = Address::generate(&env);
    let policy2_ref = PolicyReference {
        policy_address: policy2_addr.clone(),
        install_data: Bytes::from_array(&env, &[4, 5, 6]),
    };
    
    let mut policies2 = Vec::new(&env);
    policies2.push_back(policy2_ref);
    
    let params2 = InstallParams {
        policies: policies2,
    };
    
    policy.install(env.clone(), account2.clone(), context_rule_id, params2)
        .expect("Install for account2 should succeed");
    
    // Verify each account has its own policies
    let stored_ref1 = policy.get_policy_reference(env.clone(), account1.clone(), context_rule_id, 0)
        .expect("Should get account1 policy reference");
    assert_eq!(stored_ref1.policy_address, policy1_addr);
    assert_eq!(stored_ref1.install_data.to_vec(), vec![1, 2, 3]);
    
    let stored_ref2 = policy.get_policy_reference(env.clone(), account2.clone(), context_rule_id, 0)
        .expect("Should get account2 policy reference");
    assert_eq!(stored_ref2.policy_address, policy2_addr);
    assert_eq!(stored_ref2.install_data.to_vec(), vec![4, 5, 6]);
}