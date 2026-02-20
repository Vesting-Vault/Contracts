#![cfg(test)]

use super::*;
use soroban_sdk::{vec, Env, Address, String};

#[test]
fn test_admin_ownership_transfer() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Verify initial admin
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_proposed_admin(), None);
    
    // Test: Unauthorized user cannot propose new admin
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    // This should fail - unauthorized user cannot propose admin
    let result = std::panic::catch_unwind(|| {
        client.propose_new_admin(&new_admin);
    });
    assert!(result.is_err());
    
    // Reset to admin context
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    // Test: Admin can propose new admin
    client.propose_new_admin(&new_admin);
    assert_eq!(client.get_proposed_admin(), Some(new_admin));
    
    // Test: Unauthorized user cannot accept ownership
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.accept_ownership();
    });
    assert!(result.is_err());
    
    // Test: Proposed admin can accept ownership
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&new_admin);
    });
    
    client.accept_ownership();
    
    // Verify admin transfer completed
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_proposed_admin(), None);
    
    // Test: Old admin cannot propose new admin anymore
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let another_admin = Address::generate(&env);
    let result = std::panic::catch_unwind(|| {
        client.propose_new_admin(&another_admin);
    });
    assert!(result.is_err());
    
    // Test: New admin can propose admin changes
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&new_admin);
    });
    
    client.propose_new_admin(&another_admin);
    assert_eq!(client.get_proposed_admin(), Some(another_admin));
}

#[test]
fn test_admin_access_control() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Test: Unauthorized user cannot create vaults
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.create_vault_full(&vault_owner, &1000i128, &100u64, &200u64);
    });
    assert!(result.is_err());
    
    let result = std::panic::catch_unwind(|| {
        client.create_vault_lazy(&vault_owner, &1000i128, &100u64, &200u64);
    });
    assert!(result.is_err());
    
    // Test: Admin can create vaults
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_id = client.create_vault_full(&vault_owner, &1000i128, &100u64, &200u64);
    assert_eq!(vault_id, 1);
    
    let vault_id2 = client.create_vault_lazy(&vault_owner, &500i128, &150u64, &250u64);
    assert_eq!(vault_id2, 2);
}

#[test]
fn test_batch_operations_admin_control() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create batch data
    let batch_data = BatchCreateData {
        recipients: vec![&env, recipient1.clone(), recipient2.clone()],
        amounts: vec![&env, 1000i128, 2000i128],
        start_times: vec![&env, 100u64, 150u64],
        end_times: vec![&env, 200u64, 250u64],
    };
    
    // Test: Unauthorized user cannot create batch vaults
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.batch_create_vaults_lazy(&batch_data);
    });
    assert!(result.is_err());
    
    let result = std::panic::catch_unwind(|| {
        client.batch_create_vaults_full(&batch_data);
    });
    assert!(result.is_err());
    
    // Test: Admin can create batch vaults
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_ids = client.batch_create_vaults_lazy(&batch_data);
    assert_eq!(vault_ids.len(), 2);
    assert_eq!(vault_ids.get(0), 1);
    assert_eq!(vault_ids.get(1), 2);
}
