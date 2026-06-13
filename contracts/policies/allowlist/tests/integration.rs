//! Integration tests for allowlist policy contract

use allowlist::{AllowlistPolicy, InstallParams, AllowedContract};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol, Vec};

#[test]
fn test_basic_allowlist_operations() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 123;
    
    let policy = AllowlistPolicy;
    
    // Create test contracts and functions
    let contract1 = Address::generate(&env);
    let contract2 = Address::generate(&env);
    let func1 = symbol_short!("transfer");
    let func2 = symbol_short!("approve");
    
    let mut allowed_functions = Vec::new(&env);
    allowed_functions.push_back(func1);
    allowed_functions.push_back(func2);
    
    let allowed_contract = AllowedContract {
        contract_address: contract1.clone(),
        allowed_functions,
    };
    
    let mut allowed_contracts = Vec::new(&env);
    allowed_contracts.push_back(allowed_contract);
    
    // Install with one allowed contract
    let params = InstallParams {
        allowed_contracts: allowed_contracts.clone(),
        allow_unknown: false,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed");
    
    // Verify parameters stored
    let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    assert_eq!(stored_params.allow_unknown, params.allow_unknown);
    assert_eq!(stored_params.allowed_contracts.len(), 1);
    
    // Check allowed status
    let check1 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract1.clone(), func1)
        .expect("Check should work");
    assert!(check1, "Contract1 with func1 should be allowed");
    
    let check2 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract2.clone(), func1)
        .expect("Check should work");
    assert!(!check2, "Contract2 should not be allowed");
}

#[test]
fn test_allow_unknown_flag() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 456;
    
    let policy = AllowlistPolicy;
    
    // Install with empty allowlist but allow_unknown = true
    let empty_vec = Vec::new(&env);
    let params = InstallParams {
        allowed_contracts: empty_vec,
        allow_unknown: true,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
    assert!(result.is_ok(), "Install should succeed with allow_unknown = true");
    
    // Any contract/function should be allowed
    let contract = Address::generate(&env);
    let func = symbol_short!("any_function");
    
    let check = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract, func)
        .expect("Check should work");
    assert!(check, "All contracts/functions should be allowed with allow_unknown = true");
}

#[test]
fn test_add_remove_contract_operations() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 789;
    
    let policy = AllowlistPolicy;
    
    // Install with empty list but allow_unknown = true
    let empty_vec = Vec::new(&env);
    let params = InstallParams {
        allowed_contracts: empty_vec,
        allow_unknown: true,
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params)
        .expect("Install should succeed");
    
    // Add a contract
    let contract1 = Address::generate(&env);
    let func1 = symbol_short!("transfer");
    let mut allowed_functions1 = Vec::new(&env);
    allowed_functions1.push_back(func1);
    
    let result1 = policy.add_contract(
        env.clone(),
        account.clone(),
        context_rule_id,
        contract1.clone(),
        allowed_functions1.clone(),
    );
    assert!(result1.is_ok(), "Add contract should succeed");
    
    // Add another contract
    let contract2 = Address::generate(&env);
    let func2 = symbol_short!("approve");
    let mut allowed_functions2 = Vec::new(&env);
    allowed_functions2.push_back(func2);
    
    let result2 = policy.add_contract(
        env.clone(),
        account.clone(),
        context_rule_id,
        contract2.clone(),
        allowed_functions2.clone(),
    );
    assert!(result2.is_ok(), "Add second contract should succeed");
    
    // Verify both contracts added
    let params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    assert_eq!(params.allowed_contracts.len(), 2);
    
    // Remove first contract
    let result3 = policy.remove_contract(
        env.clone(),
        account.clone(),
        context_rule_id,
        contract1.clone(),
    );
    assert!(result3.is_ok(), "Remove contract should succeed");
    
    // Verify contract removed
    let params = policy.get_params(env.clone(), account.clone(), context_rule_id)
        .expect("Should retrieve params");
    assert_eq!(params.allowed_contracts.len(), 1);
    assert_eq!(params.allowed_contracts.get(0).unwrap().contract_address, contract2);
    
    // Try to remove non-existent contract
    let result4 = policy.remove_contract(
        env.clone(),
        account.clone(),
        context_rule_id,
        Address::generate(&env),
    );
    assert!(result4.is_err(), "Remove non-existent contract should fail");
    assert_eq!(result4.unwrap_err(), allowlist::PolicyError::NotFound);
}

#[test]
fn test_function_specific_allowlisting() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 999;
    
    let policy = AllowlistPolicy;
    
    // Create contract with specific functions allowed
    let contract = Address::generate(&env);
    let allowed_func1 = symbol_short!("transfer");
    let allowed_func2 = symbol_short!("approve");
    let disallowed_func = symbol_short!("mint");
    
    let mut allowed_functions = Vec::new(&env);
    allowed_functions.push_back(allowed_func1);
    allowed_functions.push_back(allowed_func2);
    
    let allowed_contract = AllowedContract {
        contract_address: contract.clone(),
        allowed_functions,
    };
    
    let mut allowed_contracts = Vec::new(&env);
    allowed_contracts.push_back(allowed_contract);
    
    // Install
    let params = InstallParams {
        allowed_contracts,
        allow_unknown: false,
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params)
        .expect("Install should succeed");
    
    // Check allowed functions
    let check1 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), allowed_func1)
        .expect("Check should work");
    assert!(check1, "Allowed function 1 should pass");
    
    let check2 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), allowed_func2)
        .expect("Check should work");
    assert!(check2, "Allowed function 2 should pass");
    
    let check3 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), disallowed_func)
        .expect("Check should work");
    assert!(!check3, "Disallowed function should fail");
}

#[test]
fn test_parameter_validation() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 111;
    
    let policy = AllowlistPolicy;
    
    // Test invalid: empty allowed_contracts and allow_unknown = false
    let empty_vec = Vec::new(&env);
    let invalid_params = InstallParams {
        allowed_contracts: empty_vec,
        allow_unknown: false,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, invalid_params);
    assert!(result.is_err(), "Empty allowlist with allow_unknown=false should fail");
    assert_eq!(result.unwrap_err(), allowlist::PolicyError::InvalidParams);
    
    // Test valid: empty allowed_contracts but allow_unknown = true
    let empty_vec = Vec::new(&env);
    let valid_params1 = InstallParams {
        allowed_contracts: empty_vec,
        allow_unknown: true,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, valid_params1);
    assert!(result.is_ok(), "Empty allowlist with allow_unknown=true should succeed");
    
    // Clean up
    policy.uninstall(env.clone(), account.clone(), context_rule_id)
        .expect("Uninstall should succeed");
    
    // Test valid: non-empty allowed_contracts
    let contract = Address::generate(&env);
    let mut allowed_functions = Vec::new(&env);
    allowed_functions.push_back(symbol_short!("transfer"));
    
    let allowed_contract = AllowedContract {
        contract_address: contract,
        allowed_functions,
    };
    
    let mut allowed_contracts = Vec::new(&env);
    allowed_contracts.push_back(allowed_contract);
    
    let valid_params2 = InstallParams {
        allowed_contracts,
        allow_unknown: false,
    };
    
    let result = policy.install(env.clone(), account.clone(), context_rule_id, valid_params2);
    assert!(result.is_ok(), "Non-empty allowlist should succeed");
}

#[test]
fn test_policy_trait_implementation() {
    let env = Env::default();
    let policy = AllowlistPolicy;
    
    // Test weight method
    let weight = policy.weight();
    assert!(weight > 0, "Policy should have positive weight");
    assert!(weight < 10, "Allowlist should have low weight");
    
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
fn test_empty_function_list_allows_all() {
    let env = Env::default();
    let account = Address::generate(&env);
    let context_rule_id = 222;
    
    let policy = AllowlistPolicy;
    
    // Create contract with empty function list (allows all functions)
    let contract = Address::generate(&env);
    let empty_functions = Vec::new(&env);
    
    let allowed_contract = AllowedContract {
        contract_address: contract.clone(),
        allowed_functions: empty_functions,
    };
    
    let mut allowed_contracts = Vec::new(&env);
    allowed_contracts.push_back(allowed_contract);
    
    // Install
    let params = InstallParams {
        allowed_contracts,
        allow_unknown: false,
    };
    
    policy.install(env.clone(), account.clone(), context_rule_id, params)
        .expect("Install should succeed");
    
    // Any function on this contract should be allowed
    let check1 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), symbol_short!("transfer"))
        .expect("Check should work");
    assert!(check1, "Any function should be allowed with empty function list");
    
    let check2 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), symbol_short!("approve"))
        .expect("Check should work");
    assert!(check2, "Any function should be allowed with empty function list");
    
    let check3 = policy.check_allowed(env.clone(), account.clone(), context_rule_id, contract.clone(), symbol_short!("mint"))
        .expect("Check should work");
    assert!(check3, "Any function should be allowed with empty function list");
}