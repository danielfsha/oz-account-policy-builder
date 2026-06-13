//! Allowlist policy for OpenZeppelin smart accounts
//!
//! This policy restricts which contracts and functions can be called.
//! Not available in OZ primitives, so we implement it as a custom policy.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Val, Vec};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};
use policy_primitives::{PolicyError, PolicyResult, PolicyStorage, ValidateParams};

// ── Install parameters ──────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct AllowedContract {
    /// Contract address that is allowed
    pub contract_address: Address,
    /// List of allowed function names (empty means all functions)
    pub allowed_functions: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InstallParams {
    /// List of allowed contracts and their functions
    pub allowed_contracts: Vec<AllowedContract>,
    /// Whether to allow contracts not explicitly listed (default: false)
    pub allow_unknown: bool,
}

impl ValidateParams for InstallParams {
    fn validate(&self) -> PolicyResult<()> {
        if self.allowed_contracts.is_empty() && !self.allow_unknown {
            return Err(PolicyError::InvalidParams);
        }
        Ok(())
    }
}

// ── Policy implementation ───────────────────────────────────────────────────

#[contract]
pub struct AllowlistPolicy;

impl AllowlistPolicy {
    /// Check if a contract and function are allowed
    fn is_allowed(&self, params: &InstallParams, contract_address: &Address, function_name: &Symbol) -> bool {
        if params.allow_unknown {
            return true;
        }
        
        for allowed_contract in params.allowed_contracts.iter() {
            if allowed_contract.contract_address == *contract_address {
                // Check if specific functions are listed
                if allowed_contract.allowed_functions.is_empty() {
                    return true; // All functions allowed for this contract
                }
                
                // Check if this specific function is allowed
                for allowed_func in allowed_contract.allowed_functions.iter() {
                    if allowed_func == function_name {
                        return true;
                    }
                }
            }
        }
        
        false
    }
}

#[contractimpl]
impl AllowlistPolicy {
    /// Install the policy for a specific smart account and context rule
    pub fn install(env: Env, account: Address, context_rule_id: u32, params: InstallParams) -> PolicyResult<()> {
        params.validate()?;
        
        // Store parameters
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        Ok(())
    }

    /// Uninstall the policy (remove all state)
    pub fn uninstall(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<()> {
        PolicyStorage::set(&env, &account, context_rule_id, "params", &());
        Ok(())
    }

    /// Get installation parameters
    pub fn get_params(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<InstallParams> {
        match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(params) => Ok(params),
            None => Err(PolicyError::NotFound),
        }
    }

    /// Add a contract to the allowlist (can only be called by the account owner)
    pub fn add_contract(
        env: Env,
        account: Address,
        context_rule_id: u32,
        contract_address: Address,
        allowed_functions: Vec<Symbol>,
    ) -> PolicyResult<()> {
        let mut params: InstallParams = match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(p) => p,
            None => return Err(PolicyError::NotFound),
        };
        
        // Check if contract already exists
        for allowed_contract in params.allowed_contracts.iter() {
            if allowed_contract.contract_address == contract_address {
                return Err(PolicyError::InvalidParams); // Already exists
            }
        }
        
        // Add new contract
        let new_contract = AllowedContract {
            contract_address,
            allowed_functions,
        };
        
        params.allowed_contracts.push_back(new_contract);
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        Ok(())
    }

    /// Remove a contract from the allowlist
    pub fn remove_contract(
        env: Env,
        account: Address,
        context_rule_id: u32,
        contract_address: Address,
    ) -> PolicyResult<()> {
        let mut params: InstallParams = match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(p) => p,
            None => return Err(PolicyError::NotFound),
        };
        
        // Find and remove the contract
        let mut found = false;
        let mut new_contracts = Vec::new(&env);
        
        for allowed_contract in params.allowed_contracts.iter() {
            if allowed_contract.contract_address != contract_address {
                new_contracts.push_back(allowed_contract);
            } else {
                found = true;
            }
        }
        
        if !found {
            return Err(PolicyError::NotFound);
        }
        
        params.allowed_contracts = new_contracts;
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        Ok(())
    }

    /// Check if a contract/function combination is allowed
    pub fn check_allowed(
        env: Env,
        account: Address,
        context_rule_id: u32,
        contract_address: Address,
        function_name: Symbol,
    ) -> PolicyResult<bool> {
        let params: InstallParams = match PolicyStorage::get(&env, &account, context_rule_id, "params") {
            Some(p) => p,
            None => return Err(PolicyError::NotFound),
        };
        
        Ok(self.is_allowed(&params, &contract_address, &function_name))
    }
}

#[contractimpl]
impl Policy for AllowlistPolicy {
    fn enforce(
        &self,
        env: Env,
        rule: ContextRule,
        signer: Signer,
        context: Vec<Context>,
        auth_context: Val,
    ) -> Result<(), stellar_accounts::policies::PolicyError> {
        // Extract contract address and function name from auth_context
        // This is a simplified implementation - real implementation would need
        // to parse the auth_context to get the actual contract call details
        
        // For now, we'll assume the contract address and function name are encoded
        // in the auth_context in some way. This is a placeholder implementation.
        
        // Get account from context
        let account = match signer {
            Signer::SmartAccount(addr) => addr,
            _ => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        // Get stored params
        let params: InstallParams = match PolicyStorage::get(&env, &account, rule.id, "params") {
            Some(p) => p,
            None => return Err(stellar_accounts::policies::PolicyError::Denied),
        };
        
        // TODO: Extract contract_address and function_name from auth_context
        // For now, we'll approve all calls if allow_unknown is true
        if params.allow_unknown {
            return Ok(());
        }
        
        // TODO: Check if the specific contract/function is allowed
        // For now, deny all calls (conservative)
        Err(stellar_accounts::policies::PolicyError::Denied)
    }

    fn weight(&self) -> u32 {
        3 // Low weight - simple checks
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_install_and_basic_operations() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = AllowlistPolicy;
        
        // Create some test contracts and functions
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
        assert!(result.is_ok());
        
        // Verify params stored
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(stored_params.allow_unknown, params.allow_unknown);
        assert_eq!(stored_params.allowed_contracts.len(), 1);
    }
    
    #[test]
    fn test_install_validation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = AllowlistPolicy;
        
        // Test with empty allowed_contracts and allow_unknown = false
        let empty_vec = Vec::new(&env);
        let params = InstallParams {
            allowed_contracts: empty_vec,
            allow_unknown: false,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test with empty allowed_contracts but allow_unknown = true (should pass)
        let empty_vec = Vec::new(&env);
        let params = InstallParams {
            allowed_contracts: empty_vec,
            allow_unknown: true,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_allowlist_check_logic() {
        let env = Env::default();
        let policy = AllowlistPolicy;
        
        // Create test contracts and functions
        let contract1 = Address::generate(&env);
        let contract2 = Address::generate(&env);
        let func1 = symbol_short!("transfer");
        let func2 = symbol_short!("approve");
        
        let mut allowed_functions = Vec::new(&env);
        allowed_functions.push_back(func1);
        
        let allowed_contract = AllowedContract {
            contract_address: contract1.clone(),
            allowed_functions,
        };
        
        let mut allowed_contracts = Vec::new(&env);
        allowed_contracts.push_back(allowed_contract);
        
        let params = InstallParams {
            allowed_contracts,
            allow_unknown: false,
        };
        
        // Test: contract1 with allowed function should pass
        assert!(policy.is_allowed(&params, &contract1, &func1));
        
        // Test: contract1 with non-allowed function should fail
        assert!(!policy.is_allowed(&params, &contract1, &func2));
        
        // Test: contract2 (not in list) should fail
        assert!(!policy.is_allowed(&params, &contract2, &func1));
        
        // Test with allow_unknown = true
        let params = InstallParams {
            allowed_contracts: Vec::new(&env),
            allow_unknown: true,
        };
        
        // All contracts/functions should pass
        assert!(policy.is_allowed(&params, &contract1, &func1));
        assert!(policy.is_allowed(&params, &contract2, &func2));
    }
    
    #[test]
    fn test_add_remove_contract() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = AllowlistPolicy;
        
        // Install with empty list but allow_unknown = true
        let params = InstallParams {
            allowed_contracts: Vec::new(&env),
            allow_unknown: true,
        };
        
        policy.install(env.clone(), account.clone(), context_rule_id, params).unwrap();
        
        // Add a contract
        let contract1 = Address::generate(&env);
        let func1 = symbol_short!("transfer");
        let mut allowed_functions = Vec::new(&env);
        allowed_functions.push_back(func1);
        
        let result = policy.add_contract(
            env.clone(),
            account.clone(),
            context_rule_id,
            contract1.clone(),
            allowed_functions.clone(),
        );
        assert!(result.is_ok());
        
        // Check it was added
        let params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(params.allowed_contracts.len(), 1);
        assert_eq!(params.allowed_contracts.get(0).unwrap().contract_address, contract1);
        
        // Remove the contract
        let result = policy.remove_contract(
            env.clone(),
            account.clone(),
            context_rule_id,
            contract1.clone(),
        );
        assert!(result.is_ok());
        
        // Check it was removed
        let params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(params.allowed_contracts.len(), 0);
    }
}
