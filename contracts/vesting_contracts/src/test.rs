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

#[test]
fn test_revoke_tokens() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Test: Unauthorized user cannot revoke tokens
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.revoke_tokens(&vault_id);
    });
    assert!(result.is_err());
    
    // Test: Admin can revoke tokens
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let initial_admin_balance = 1000000i128 - vault_amount; // After vault creation
    let revoked_amount = client.revoke_tokens(&vault_id);
    assert_eq!(revoked_amount, vault_amount);
    
    // Verify admin balance increased
    let final_admin_balance = initial_admin_balance + revoked_amount;
    // Note: We can't directly check admin balance without a getter, but we can verify through invariant
    
    // Verify vault is fully released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.released_amount, vault.total_amount);
    
    // Test: Cannot revoke tokens from already revoked vault
    let result = std::panic::catch_unwind(|| {
        client.revoke_tokens(&vault_id);
    });
    assert!(result.is_err());
}

#[test]
fn test_revoke_tokens_partial_claim() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Claim some tokens first
    let claim_amount = 300i128;
    let claimed = client.claim_tokens(&vault_id, &claim_amount);
    assert_eq!(claimed, claim_amount);
    
    // Revoke remaining tokens
    let revoked_amount = client.revoke_tokens(&vault_id);
    assert_eq!(revoked_amount, vault_amount - claim_amount);
    
    // Verify vault is fully released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.released_amount, vault.total_amount);
}

#[test]
fn test_revoke_tokens_event() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Revoke tokens and check event
    let revoked_amount = client.revoke_tokens(&vault_id);
    
    // Verify the event was emitted (this would need to be checked with event listeners in a real scenario)
    // For now, we just verify the function returns the correct amount
    assert_eq!(revoked_amount, vault_amount);
}

#[test]
fn test_revoke_nonexistent_vault() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Test: Cannot revoke tokens from nonexistent vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.revoke_tokens(&999u64);
    });
    assert!(result.is_err());
}


#[test]
fn test_transfer_beneficiary_nonexistent_vault() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);
    
    client.initialize(&admin, &1000000i128);
    
    let invalid_vault_id = 999u64;
    
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&beneficiary);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.transfer_beneficiary(&invalid_vault_id, &new_beneficiary);
    });
    assert!(result.is_err());
}

#[test]
fn test_revoke_partial_tokens() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Test: Unauthorized user cannot revoke partial tokens
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized_user);
    });
    
    let result = std::panic::catch_unwind(|| {
        client.revoke_partial(&vault_id, &100i128);
    });
    assert!(result.is_err());
    
    // Test: Admin can revoke partial tokens
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let partial_amount = 300i128;
    let revoked_amount = client.revoke_partial(&vault_id, &partial_amount);
    assert_eq!(revoked_amount, partial_amount);
    
    // Verify vault state after partial revocation
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.released_amount, partial_amount);
    assert_eq!(vault.total_amount, vault_amount);
    assert_eq!(vault.total_amount - vault.released_amount, vault_amount - partial_amount); // Remaining unvested
    
    // Test: Can revoke more tokens (but not more than available)
    let additional_amount = 400i128;
    let total_revoked_expected = partial_amount + additional_amount;
    let additional_revoked = client.revoke_partial(&vault_id, &additional_amount);
    assert_eq!(additional_revoked, additional_amount);
    
    // Verify final vault state
    let final_vault = client.get_vault(&vault_id);
    assert_eq!(final_vault.released_amount, total_revoked_expected);
}

#[test]
fn test_revoke_partial_invalid_amounts() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Test: Cannot revoke zero amount
    let result = std::panic::catch_unwind(|| {
        client.revoke_partial(&vault_id, &0i128);
    });
    assert!(result.is_err());
    
    // Test: Cannot revoke negative amount
    let result = std::panic::catch_unwind(|| {
        client.revoke_partial(&vault_id, &(-100i128));
    });
    assert!(result.is_err());
    
    // Test: Cannot revoke more than unvested balance
    let result = std::panic::catch_unwind(|| {
        client.revoke_partial(&vault_id, &(vault_amount + 1i128));
    });
    assert!(result.is_err());
}

#[test]
fn test_revoke_partial_with_claims() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Create a vault
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_amount = 1000i128;
    let vault_id = client.create_vault_full(&vault_owner, &vault_amount, &100u64, &200u64);
    
    // Claim some tokens first
    let claim_amount = 200i128;
    let claimed = client.claim_tokens(&vault_id, &claim_amount);
    assert_eq!(claimed, claim_amount);
    
    // Verify state after claim
    let vault_after_claim = client.get_vault(&vault_id);
    assert_eq!(vault_after_claim.released_amount, claim_amount);
    
    // Now partially revoke some of the remaining unvested tokens
    let remaining_unvested = vault_amount - claim_amount; // 800 tokens
    let revoke_amount = 300i128; // Less than remaining unvested
    let revoked = client.revoke_partial(&vault_id, &revoke_amount);
    assert_eq!(revoked, revoke_amount);
    
    // Verify final state
    let final_vault = client.get_vault(&vault_id);
    let expected_released = claim_amount + revoke_amount; // 200 + 300 = 500
    assert_eq!(final_vault.released_amount, expected_released);
    assert_eq!(final_vault.total_amount, vault_amount);
}

#[test]
fn test_usdc_integration_mock_token() {
    use soroban_sdk::{token, Address, Env, Vec};
    use soroban_sdk::token::{TokenClient};

    let env = Env::default();
    
    // Deploy a mock token contract that simulates USDC (18 decimals)
    let token_contract_id = env.register_stellar_asset_contract(None);
    let token_client = TokenClient::new(&env, &token_contract_id);
    
    // Initialize the token with 18 decimals (USDC style)
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    token_client.initialize(admin.clone(), &7, &String::from_str(&env, "USDC"), &String::from_str(&env, "USDC")); // 7 decimals for test
    
    // Mint some tokens to the user (1,000,000 USDC * 10^7 = 100,000,000,000,000)
    let usdc_amount = 1_000_000_i128; // 1M USDC
    let scaled_amount = usdc_amount * 10_i128.pow(7); // Scale to include 7 decimals
    token_client.mint(&user, &scaled_amount);
    
    // Verify the user has the tokens
    assert_eq!(token_client.balance(&user), scaled_amount);
    
    // Now deploy the vesting contract
    let vesting_contract_id = env.register(VestingContract, ());
    let vesting_client = VestingContractClient::new(&env, &vesting_contract_id);
    
    // Initialize the vesting contract with a large enough supply to cover our vault
    let initial_supply = scaled_amount * 2; // 2x to be safe
    vesting_client.initialize(&admin, &initial_supply);
    
    // Create a vault with 1,000,000 USDC worth of tokens (using the scaled amount)
    env.as_contract(&vesting_contract_id, || {
        env.current_contract_address().set(&admin);
    });
    
    let vault_id = vesting_client.create_vault_full(&user, &scaled_amount, &100u64, &200u64);
    
    // Verify the vault was created correctly
    let vault = vesting_client.get_vault(&vault_id);
    assert_eq!(vault.total_amount, scaled_amount);
    assert_eq!(vault.owner, user);
    
    // Test that claims work correctly with the scaled amounts
    // First, advance the ledger time to make some tokens claimable
    env.ledger().set_timestamp(150); // Halfway through the vesting period
    
    // Check claimable balance
    let claimable = scaled_amount / 2; // Should be roughly half since we're halfway through
    let claimed = vesting_client.claim_tokens(&vault_id, &claimable);
    assert_eq!(claimed, claimable);
    
    // Verify vault state after claim
    let updated_vault = vesting_client.get_vault(&vault_id);
    assert_eq!(updated_vault.released_amount, claimable);
}

// -------------------------------------------------------------------------
// Additional beneficiary-transfer tests added for coverage
// -------------------------------------------------------------------------

#[test]
fn test_transfer_beneficiary_unauthorized_user() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);

    client.initialize(&admin, &1000000i128);

    // create a normal vault as admin
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    let vault_id = client.create_vault_full(&beneficiary, &1000i128, &0u64, &100u64);

    // switch to unauthorized address and attempt transfer
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&unauthorized);
    });
    let result = std::panic::catch_unwind(|| {
        client.transfer_beneficiary(&vault_id, &new_beneficiary);
    });
    assert!(result.is_err());
}

#[test]
fn test_transfer_beneficiary_successful_full() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);

    client.initialize(&admin, &1000000i128);

    // create and verify vault ownership
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    let vault_id = client.create_vault_full(&beneficiary, &500i128, &50u64, &150u64);
    assert_eq!(client.get_vault(&vault_id).owner, beneficiary);

    // ensure user vault lists are correct prior to transfer
    let list_before = client.get_user_vaults(&beneficiary);
    assert_eq!(list_before.len(), 1);
    assert_eq!(list_before.get(0), vault_id);
    assert_eq!(client.get_user_vaults(&new_beneficiary).len(), 0);

    // perform transfer as admin
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    client.transfer_beneficiary(&vault_id, &new_beneficiary);

    // verify vault owner changed
    let updated_vault = client.get_vault(&vault_id);
    assert_eq!(updated_vault.owner, new_beneficiary);

    // check user vault lists update accordingly
    let old_list = client.get_user_vaults(&beneficiary);
    assert_eq!(old_list.len(), 0);
    let new_list = client.get_user_vaults(&new_beneficiary);
    assert_eq!(new_list.len(), 1);
    assert_eq!(new_list.get(0), vault_id);
}

#[test]
fn test_transfer_beneficiary_lazy_behaviour() {
    let env = Env::default();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);

    client.initialize(&admin, &1000000i128);

    // create vault lazily
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    let vault_id = client.create_vault_lazy(&beneficiary, &750i128, &10u64, &20u64);

    // before initialization, none of the owners should have the vault listed
    assert_eq!(client.get_user_vaults(&beneficiary).len(), 0);
    assert_eq!(client.get_user_vaults(&new_beneficiary).len(), 0);

    // transfer beneficiary while vault is still lazy
    env.as_contract(&contract_id, || {
        env.current_contract_address().set(&admin);
    });
    client.transfer_beneficiary(&vault_id, &new_beneficiary);

    // reading the vault will auto-initialize metadata and update lists
    let vault_after = client.get_vault(&vault_id);
    assert_eq!(vault_after.owner, new_beneficiary);
    assert!(vault_after.is_initialized);

    // verify the index moved to the new beneficiary only
    assert_eq!(client.get_user_vaults(&beneficiary).len(), 0);
    let final_list = client.get_user_vaults(&new_beneficiary);
    assert_eq!(final_list.len(), 1);
    assert_eq!(final_list.get(0), vault_id);
}


