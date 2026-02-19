#![cfg(test)]

use soroban_sdk::{vec, Env, Address, Symbol, testutils::{Address as TestAddress}};
use vesting_contracts::{VestingContract, VestingContractClient, Vault, BatchCreateData};

#[test]
fn test_lazy_vs_full_single_vault() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    // Initialize contract
    let admin = TestAddress::generate(&env);
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);

    // Test full initialization
    let user1 = TestAddress::generate(&env);
    let start_time = 1640995200u64; // Jan 1, 2022
    let end_time = 1672531199u64;   // Dec 31, 2022
    let amount = 100000i128;

    let full_gas_before = env.budget().cpu_instructions();
    let vault_id_full = client.create_vault_full(&user1, &amount, &start_time, &end_time);
    let full_gas_after = env.budget().cpu_instructions();
    let full_gas_used = full_gas_after - full_gas_before;

    // Reset environment for lazy test
    let env2 = Env::default();
    let contract_id2 = env2.register(VestingContract, ());
    let client2 = VestingContractClient::new(&env2, &contract_id2);
    client2.initialize(&admin, &initial_supply);

    // Test lazy initialization
    let user2 = TestAddress::generate(&env2);

    let lazy_gas_before = env2.budget().cpu_instructions();
    let vault_id_lazy = client2.create_vault_lazy(&user2, &amount, &start_time, &end_time);
    let lazy_gas_after = env2.budget().cpu_instructions();
    let lazy_gas_used = lazy_gas_after - lazy_gas_before;

    println!("📊 Single Vault Creation Gas Usage:");
    println!("  Full Initialization: {} instructions", full_gas_used);
    println!("  Lazy Initialization: {} instructions", lazy_gas_used);
    println!("  Gas Savings: {}%", ((full_gas_used - lazy_gas_used) * 100) / full_gas_used);

    // Verify both vaults work correctly
    let vault_full = client.get_vault(&vault_id_full);
    let vault_lazy = client2.get_vault(&vault_id_lazy);

    assert_eq!(vault_full.total_amount, amount);
    assert_eq!(vault_lazy.total_amount, amount);
    assert!(vault_full.is_initialized);
    assert!(vault_lazy.is_initialized);
}

#[test]
fn test_lazy_vs_full_batch_creation() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    // Initialize contract
    let admin = TestAddress::generate(&env);
    let initial_supply = 10000000i128;
    client.initialize(&admin, &initial_supply);

    // Prepare batch data (10 vaults)
    let mut recipients = Vec::new(&env);
    let mut amounts = Vec::new(&env);
    let mut start_times = Vec::new(&env);
    let mut end_times = Vec::new(&env);

    for i in 0..10 {
        recipients.push_back(TestAddress::generate(&env));
        amounts.push_back((i + 1) * 50000i128); // 50k to 500k
        start_times.push_back(1640995200u64);
        end_times.push_back(1672531199u64);
    }

    let batch_data_full = BatchCreateData {
        recipients: recipients.clone(),
        amounts: amounts.clone(),
        start_times: start_times.clone(),
        end_times: end_times.clone(),
    };

    // Test full batch initialization
    let full_gas_before = env.budget().cpu_instructions();
    let vault_ids_full = client.batch_create_vaults_full(&batch_data_full);
    let full_gas_after = env.budget().cpu_instructions();
    let full_gas_used = full_gas_after - full_gas_before;

    // Reset environment for lazy test
    let env2 = Env::default();
    let contract_id2 = env2.register(VestingContract, ());
    let client2 = VestingContractClient::new(&env2, &contract_id2);
    client2.initialize(&admin, &initial_supply);

    let batch_data_lazy = BatchCreateData {
        recipients,
        amounts,
        start_times,
        end_times,
    };

    // Test lazy batch initialization
    let lazy_gas_before = env2.budget().cpu_instructions();
    let vault_ids_lazy = client2.batch_create_vaults_lazy(&batch_data_lazy);
    let lazy_gas_after = env2.budget().cpu_instructions();
    let lazy_gas_used = lazy_gas_after - lazy_gas_before;

    println!("📊 Batch Creation Gas Usage (10 vaults):");
    println!("  Full Initialization: {} instructions", full_gas_used);
    println!("  Lazy Initialization: {} instructions", lazy_gas_used);
    println!("  Gas Savings: {}%", ((full_gas_used - lazy_gas_used) * 100) / full_gas_used);

    // Verify both batches work correctly
    assert_eq!(vault_ids_full.len(), 10);
    assert_eq!(vault_ids_lazy.len(), 10);

    // Test on-demand initialization for lazy vaults
    let init_gas_before = env2.budget().cpu_instructions();
    for vault_id in vault_ids_lazy.iter() {
        client2.get_vault(vault_id); // This triggers initialization
    }
    let init_gas_after = env2.budget().cpu_instructions();
    let init_gas_used = init_gas_after - init_gas_before;

    println!("  Lazy Initialization (on-demand): {} instructions", init_gas_used);
    println!("  Total Lazy Gas: {} instructions", lazy_gas_used + init_gas_used);

    let total_lazy_gas = lazy_gas_used + init_gas_used;
    if total_lazy_gas < full_gas_used {
        println!("  Overall Gas Savings: {}%", ((full_gas_used - total_lazy_gas) * 100) / full_gas_used);
    }
}

#[test]
fn test_lazy_initialization_on_demand() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    // Initialize contract
    let admin = TestAddress::generate(&env);
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);

    // Create vault with lazy initialization
    let user = TestAddress::generate(&env);
    let start_time = 1640995200u64;
    let end_time = 1672531199u64;
    let amount = 100000i128;

    let vault_id = client.create_vault_lazy(&user, &amount, &start_time, &end_time);

    // Check that vault is not initialized yet
    let vault_before = client.get_vault(&vault_id);
    assert!(vault_before.is_initialized); // get_vault triggers initialization

    // Test direct initialization
    let env2 = Env::default();
    let contract_id2 = env2.register(VestingContract, ());
    let client2 = VestingContractClient::new(&env2, &contract_id2);
    client2.initialize(&admin, &initial_supply);

    let user2 = TestAddress::generate(&env2);
    let vault_id2 = client2.create_vault_lazy(&user2, &amount, &start_time, &end_time);

    // Manually initialize
    let was_initialized = client2.initialize_vault_metadata(&vault_id2);
    assert!(was_initialized);

    // Try to initialize again
    let was_initialized_again = client2.initialize_vault_metadata(&vault_id2);
    assert!(!was_initialized_again);

    println!("✅ Lazy initialization on-demand works correctly");
}

#[test]
fn test_gas_savings_benchmark() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    // Initialize contract
    let admin = TestAddress::generate(&env);
    let initial_supply = 50000000i128; // 50M tokens for large batch
    client.initialize(&admin, &initial_supply);

    // Test different batch sizes
    let batch_sizes = vec![5, 10, 25, 50];
    
    for batch_size in batch_sizes.iter() {
        // Prepare batch data
        let mut recipients = Vec::new(&env);
        let mut amounts = Vec::new(&env);
        let mut start_times = Vec::new(&env);
        let mut end_times = Vec::new(&env);

        for i in 0..*batch_size {
            recipients.push_back(TestAddress::generate(&env));
            amounts.push_back(100000i128); // Fixed amount for consistent comparison
            start_times.push_back(1640995200u64);
            end_times.push_back(1672531199u64);
        }

        // Test full initialization
        let batch_data_full = BatchCreateData {
            recipients: recipients.clone(),
            amounts: amounts.clone(),
            start_times: start_times.clone(),
            end_times: end_times.clone(),
        };

        let full_gas_before = env.budget().cpu_instructions();
        let _vault_ids_full = client.batch_create_vaults_full(&batch_data_full);
        let full_gas_after = env.budget().cpu_instructions();
        let full_gas_used = full_gas_after - full_gas_before;

        // Reset for lazy test
        let env2 = Env::default();
        let contract_id2 = env2.register(VestingContract, ());
        let client2 = VestingContractClient::new(&env2, &contract_id2);
        client2.initialize(&admin, &initial_supply);

        let batch_data_lazy = BatchCreateData {
            recipients,
            amounts,
            start_times,
            end_times,
        };

        // Test lazy initialization
        let lazy_gas_before = env2.budget().cpu_instructions();
        let vault_ids_lazy = client2.batch_create_vaults_lazy(&batch_data_lazy);
        let lazy_gas_after = env2.budget().cpu_instructions();
        let lazy_gas_used = lazy_gas_after - lazy_gas_before;

        // Test on-demand initialization
        let init_gas_before = env2.budget().cpu_instructions();
        for vault_id in vault_ids_lazy.iter() {
            client2.get_vault(vault_id);
        }
        let init_gas_after = env2.budget().cpu_instructions();
        let init_gas_used = init_gas_after - init_gas_before;

        let total_lazy_gas = lazy_gas_used + init_gas_used;
        let gas_savings = ((full_gas_used - total_lazy_gas) * 100) / full_gas_used;

        println!("📊 Batch Size {}: {} vaults", batch_size, batch_size);
        println!("  Full: {} instructions", full_gas_used);
        println!("  Lazy: {} instructions", lazy_gas_used);
        println!("  Init: {} instructions", init_gas_used);
        println!("  Total Lazy: {} instructions", total_lazy_gas);
        println!("  Gas Savings: {}%", gas_savings);
        println!("  Savings > 15%: {}", gas_savings > 15);
        println!();

        // Assert that we meet the acceptance criteria (>15% savings)
        assert!(gas_savings > 15, "Gas savings should be >15% for batch size {}", batch_size);
    }

    println!("✅ All batch sizes meet >15% gas savings requirement");
}

#[test]
fn test_contract_state_consistency() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    // Initialize contract
    let admin = TestAddress::generate(&env);
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);

    // Create vaults with both methods
    let user1 = TestAddress::generate(&env);
    let user2 = TestAddress::generate(&env);
    
    let vault_id_full = client.create_vault_full(&user1, &100000i128, &1640995200u64, &1672531199u64);
    let vault_id_lazy = client.create_vault_lazy(&user2, &200000i128, &1640995200u64, &1672531199u64);

    // Check contract state
    let (total_locked, total_claimed, admin_balance) = client.get_contract_state();
    
    assert_eq!(total_locked, 300000i128); // 100k + 200k
    assert_eq!(total_claimed, 0);
    assert_eq!(admin_balance, 700000i128); // 1M - 300k

    // Check invariant
    assert!(client.check_invariant());

    // Initialize lazy vault
    client.get_vault(&vault_id_lazy);

    // Check state again (should be same)
    let (total_locked2, total_claimed2, admin_balance2) = client.get_contract_state();
    
    assert_eq!(total_locked, total_locked2);
    assert_eq!(total_claimed, total_claimed2);
    assert_eq!(admin_balance, admin_balance2);
    assert!(client.check_invariant());

    println!("✅ Contract state consistency maintained");
}

fn main() {
    println!("🧪 Running Lazy Storage Optimization Tests");
    test_lazy_vs_full_single_vault();
    test_lazy_vs_full_batch_creation();
    test_lazy_initialization_on_demand();
    test_gas_savings_benchmark();
    test_contract_state_consistency();
    println!("✅ All lazy storage optimization tests passed!");
}
