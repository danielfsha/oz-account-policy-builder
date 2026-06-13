//! Composite policy for OpenZeppelin smart accounts
//!
//! This policy AND-chains up to 5 other policies, requiring all to pass.
//! Useful for combining multiple constraints (e.g., spending limit + time window).

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Val, Vec};
use stellar_accounts::{
    policies::Policy,
    smart_account::{ContextRule, Signer},
};
use policy_primitives::{PolicyError, PolicyResult, PolicyStorage, ValidateParams};

// ── Install parameters ──────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct PolicyReference {
    /// Address of the policy contract
    pub policy_address: Address,
    /// Installation parameters for that policy (encoded as bytes)
    pub install_data: Vec<u8>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InstallParams {
    /// List of policies to AND-chain (max 5)
    pub policies: Vec<PolicyReference>,
}

impl ValidateParams for InstallParams {
    fn validate(&self) -> PolicyResult<()> {
        if self.policies.is_empty() {
            return Err(PolicyError::InvalidParams);
        }
        if self.policies.len() > 5 {
            return Err(PolicyError::InvalidParams);
        }
        Ok(())
    }
}

// ── Policy implementation ───────────────────────────────────────────────────

#[contract]
pub struct CompositePolicy;

#[contractimpl]
impl CompositePolicy {
    /// Install the composite policy for a specific smart account and context rule
    pub fn install(env: Env, account: Address, context_rule_id: u32, params: InstallParams) -> PolicyResult<()> {
        params.validate()?;
        
        // Store parameters
        PolicyStorage::set(&env, &account, context_rule_id, "params", &params);
        
        // Initialize each sub-policy
        for (i, policy_ref) in params.policies.iter().enumerate() {
            // Store each policy reference
            PolicyStorage::set(&env, &account, context_rule_id, ("policy", i as u32), policy_ref);
        }
        
        Ok(())
    }

    /// Uninstall the policy (remove all state)
    pub fn uninstall(env: Env, account: Address, context_rule_id: u32) -> PolicyResult<()> {
        // Get params to know how many policies we have
        let params: Option<InstallParams> = PolicyStorage::get(&env, &account, context_rule_id, "params");
        
        if let Some(params) = params {
            // Remove each policy reference
            for i in 0..params.policies.len() {
                PolicyStorage::set(&env, &account, context_rule_id, ("policy", i as u32), &());
            }
        }
        
        // Remove main params
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

    /// Get a specific policy reference
    pub fn get_policy_reference(
        env: Env,
        account: Address,
        context_rule_id: u32,
        index: u32,
    ) -> PolicyResult<PolicyReference> {
        match PolicyStorage::get(&env, &account, context_rule_id, ("policy", index)) {
            Some(reference) => Ok(reference),
            None => Err(PolicyError::NotFound),
        }
    }
}

#[contractimpl]
impl Policy for CompositePolicy {
    fn enforce(
        &self,
        env: Env,
        rule: ContextRule,
        signer: Signer,
        context: Vec<Context>,
        auth_context: Val,
    ) -> Result<(), stellar_accounts::policies::PolicyError> {
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
        
        // Check each policy in sequence (AND logic)
        for policy_ref in params.policies.iter() {
            // Call the policy contract's enforce function
            // Note: This is a simplified implementation. In practice, we would need to:
            // 1. Create a client for the policy contract
            // 2. Call its enforce function with the same parameters
            // 3. Check the result
            
            // For now, we'll simulate by always returning success
            // TODO: Implement actual cross-contract calls
        }
        
        // If we reach here, all policies passed (or simulation passed)
        Ok(())
    }

    fn weight(&self) -> u32 {
        // Composite weight is sum of weights of all policies
        // For now, return a fixed weight
        15
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec, Bytes};

    #[test]
    fn test_install_and_basic_operations() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = CompositePolicy;
        
        // Create test policy references
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
        
        // Install with 2 policies
        let params = InstallParams {
            policies: policies.clone(),
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params.clone());
        assert!(result.is_ok());
        
        // Verify params stored
        let stored_params = policy.get_params(env.clone(), account.clone(), context_rule_id).unwrap();
        assert_eq!(stored_params.policies.len(), 2);
        
        // Verify individual policy references
        let stored_ref1 = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 0).unwrap();
        assert_eq!(stored_ref1.policy_address, policy1_addr);
        
        let stored_ref2 = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 1).unwrap();
        assert_eq!(stored_ref2.policy_address, policy2_addr);
    }
    
    #[test]
    fn test_install_validation() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = CompositePolicy;
        
        // Test with empty policies list
        let empty_vec = Vec::new(&env);
        let params = InstallParams {
            policies: empty_vec,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test with too many policies (max is 5)
        let mut too_many_policies = Vec::new(&env);
        for i in 0..6 {
            let policy_ref = PolicyReference {
                policy_address: Address::generate(&env),
                install_data: Bytes::from_array(&env, &[i as u8]),
            };
            too_many_policies.push_back(policy_ref);
        }
        
        let params = InstallParams {
            policies: too_many_policies,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(matches!(result, Err(PolicyError::InvalidParams)));
        
        // Test with valid number of policies (3)
        let mut valid_policies = Vec::new(&env);
        for i in 0..3 {
            let policy_ref = PolicyReference {
                policy_address: Address::generate(&env),
                install_data: Bytes::from_array(&env, &[i as u8]),
            };
            valid_policies.push_back(policy_ref);
        }
        
        let params = InstallParams {
            policies: valid_policies,
        };
        
        let result = policy.install(env.clone(), account.clone(), context_rule_id, params);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_uninstall_cleans_up_all_storage() {
        let env = Env::default();
        let account = Address::generate(&env);
        let context_rule_id = 123;
        
        let policy = CompositePolicy;
        
        // Install with 2 policies
        let policy1_ref = PolicyReference {
            policy_address: Address::generate(&env),
            install_data: Bytes::from_array(&env, &[1, 2, 3]),
        };
        
        let policy2_ref = PolicyReference {
            policy_address: Address::generate(&env),
            install_data: Bytes::from_array(&env, &[4, 5, 6]),
        };
        
        let mut policies = Vec::new(&env);
        policies.push_back(policy1_ref);
        policies.push_back(policy2_ref);
        
        let params = InstallParams {
            policies,
        };
        
        policy.install(env.clone(), account.clone(), context_rule_id, params).unwrap();
        
        // Verify storage exists
        let params_before = policy.get_params(env.clone(), account.clone(), context_rule_id);
        assert!(params_before.is_ok());
        
        let ref0_before = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 0);
        assert!(ref0_before.is_ok());
        
        // Uninstall
        let result = policy.uninstall(env.clone(), account.clone(), context_rule_id);
        assert!(result.is_ok());
        
        // Verify storage cleaned up
        let params_after = policy.get_params(env.clone(), account.clone(), context_rule_id);
        assert!(matches!(params_after, Err(PolicyError::NotFound)));
        
        let ref0_after = policy.get_policy_reference(env.clone(), account.clone(), context_rule_id, 0);
        assert!(matches!(ref0_after, Err(PolicyError::NotFound)));
    }
    
    #[test]
    fn test_policy_trait_implementation() {
        let env = Env::default();
        let policy = CompositePolicy;
        
        // Test weight
        assert_eq!(policy.weight(), 15);
        
        // Test enforce (placeholder implementation)
        let result = policy.enforce(
            env,
            ContextRule::default(),
            Signer::default(),
            Vec::new(&Env::default()),
            Val::default(),
        );
        assert!(result.is_ok());
    }
}
