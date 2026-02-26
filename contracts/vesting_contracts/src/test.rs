#[cfg(test)]
mod tests {
    extern crate std;
        use crate::{
    use crate::{
        BatchCreateData, Milestone, VestingContract, VestingContractClient,
    };
    use crate::{BatchCreateData, Milestone, VestingContract, VestingContractClient};
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Ledger},
        token, vec, Address, Env, Symbol, String,
    };

    // -------------------------------------------------------------------------
    // Helper: spin up a fresh contract
    // -------------------------------------------------------------------------

    fn setup() -> (
        Env,
        Address,
        VestingContractClient<'static>,
        Address,
        Address,
    ) {
    fn setup() -> (Env, Address, VestingContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &1_000_000_000i128);

        let token_addr = register_token(&env, &admin);
        client.set_token(&token_addr);
        client.add_to_whitelist(&token_addr);

        // Mint initial supply to contract
        let stellar = token::StellarAssetClient::new(&env, &token_addr);
        stellar.mint(&contract_id, &1_000_000_000i128);

        (env, contract_id, client, admin, token_addr)
        client.initialize(&admin, &1_000_000i128);
        (env, contract_id, client, admin)
    }

    fn register_token(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone())
            .address()
    }

    fn mint_to(env: &Env, token_addr: &Address, recipient: &Address, amount: i128) {
        token::StellarAssetClient::new(env, token_addr).mint(recipient, &amount);
    }

    // -------------------------------------------------------------------------
    // Admin ownership transfer
    // -------------------------------------------------------------------------

    #[test]
    fn test_admin_ownership_transfer() {
        let (env, _cid, client, admin) = setup();
        let new_admin = Address::generate(&env);

        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_proposed_admin(), None);

        client.propose_new_admin(&new_admin);
        assert_eq!(client.get_proposed_admin(), Some(new_admin.clone()));

        client.accept_ownership();
        assert_eq!(client.get_admin(), new_admin);
        assert_eq!(client.get_proposed_admin(), None);
    }

    // -------------------------------------------------------------------------
    // Vault creation
    // -------------------------------------------------------------------------

    #[test]
    fn test_migrate_liquidity_freezes_and_transfers_whitelisted_balances() {
        let (env, contract_id, client, admin, _token) = setup();

        // Whitelist + fund the contract with a token balance.
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);
        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        let v2 = Address::generate(&env);
        let token_client = token::Client::new(&env, &token_addr);

        let migrated = client.migrate_liquidity(&v2);

        assert!(client.is_deprecated());
        assert_eq!(client.get_migration_target(), Some(v2.clone()));
        assert!(client.is_paused());
    fn test_create_vault_full_increments_count() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let id1 = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );
        let id2 = client.create_vault_full(
            &beneficiary,
            &500i128,
            &(now + 10),
            &(now + 2_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );
        let id2 = client.create_vault_full(
            &beneficiary, &500i128, &(now + 10), &(now + 2_000),
            &0i128, &true, &false, &0u64,
        );
        assert_eq!(id1, 1u64);
        assert_eq!(id2, 2u64);
    }

    #[test]
    fn test_migrate_liquidity_blocks_admin_actions_afterwards() {
        let (env, contract_id, client, admin, _token) = setup();

        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);
        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        let v2 = Address::generate(&env);
        client.migrate_liquidity(&v2);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create_vault_full(
                &beneficiary,
                &1_000i128,
                &now,
                &(now + 1_000),
                &0i128,
                &true,
                &false,
                &0u64,
            );
        }));
        assert!(result.is_err());
    fn test_create_vault_lazy_increments_count() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let id = client.create_vault_lazy(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );
        assert_eq!(id, 1u64);
    }

    // -------------------------------------------------------------------------
    // Batch vault creation
    // -------------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "duration exceeds MAX_DURATION")]
    fn test_create_vault_full_rejects_duration_over_max() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let start = env.ledger().timestamp();
        let end = start + crate::MAX_DURATION + 1;

        client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &start,
            &end,
            &0i128,
            &true,
            &false,
            &0u64,
        );
    }

    #[test]
    #[should_panic(expected = "duration exceeds MAX_DURATION")]
    fn test_create_vault_lazy_rejects_duration_over_max() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let start = env.ledger().timestamp();
        let end = start + crate::MAX_DURATION + 1;

        client.create_vault_lazy(
            &beneficiary,
            &1_000i128,
            &start,
            &end,
            &0i128,
            &true,
            &false,
            &0u64,
        );
    }

    #[test]
    #[should_panic(expected = "duration exceeds MAX_DURATION")]
    fn test_batch_create_vaults_rejects_duration_over_max() {
        let (env, _cid, client, _admin, _token) = setup();
        let r1 = Address::generate(&env);
        let start = 100u64;
        let end = start + crate::MAX_DURATION + 1;

        let batch = BatchCreateData {
            recipients: vec![&env, r1.clone()],
            amounts: vec![&env, 1_000i128],
            start_times: vec![&env, start],
            end_times: vec![&env, end],
            keeper_fees: vec![&env, 0i128],
            step_durations: vec![&env, 0u64],
        };

        client.batch_create_vaults_lazy(&batch);
    }

    #[test]
    fn test_batch_create_vaults_lazy() {
        let (env, _cid, client, _admin) = setup();
        let r1 = Address::generate(&env);
        let r2 = Address::generate(&env);

        let batch = BatchCreateData {
            recipients:     vec![&env, r1.clone(), r2.clone()],
            amounts:        vec![&env, 1_000i128, 2_000i128],
            start_times:    vec![&env, 100u64, 150u64],
            end_times:      vec![&env, 200u64, 250u64],
            keeper_fees:    vec![&env, 0i128, 0i128],
            step_durations: vec![&env, 0u64, 0u64],
        };

        let ids = client.batch_create_vaults_lazy(&batch);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), 1u64);
        assert_eq!(ids.get(1).unwrap(), 2u64);
    }

    #[test]
    fn test_batch_create_vaults_full() {
        let (env, _cid, client, _admin) = setup();
        let r1 = Address::generate(&env);
        let r2 = Address::generate(&env);

        let batch = BatchCreateData {
            recipients:     vec![&env, r1.clone(), r2.clone()],
            amounts:        vec![&env, 1_000i128, 2_000i128],
            start_times:    vec![&env, 100u64, 150u64],
            end_times:      vec![&env, 200u64, 250u64],
            keeper_fees:    vec![&env, 0i128, 0i128],
            step_durations: vec![&env, 0u64, 0u64],
        };

        let ids = client.batch_create_vaults_full(&batch);
        assert_eq!(ids.len(), 2);
    }

    // -------------------------------------------------------------------------
    // Step / lockup-only vesting
    // -------------------------------------------------------------------------

    #[test]
    fn test_step_vesting_full_claim_at_end() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let start = 1_000u64;
        let end   = start + 101u64;
        let step  = 17u64;
        let total = 1_009i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &total,
            &start,
            &end,
            &0i128,
            &true,
            &true,
            &step,
            &beneficiary, &total, &start, &end,
            &0i128, &true, &true, &step,
        );

        env.ledger().with_mut(|l| l.timestamp = end + 1);
        let claimed = client.claim_tokens(&vault_id, &total);
        assert_eq!(claimed, total);

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, total);
    }

    #[test]
    fn test_lockup_only_claim_succeeds_at_end() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now      = env.ledger().timestamp();
        let duration = 1_000u64;
        let total    = 100_000i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &total,
            &now,
            &(now + duration),
            &0i128,
            &true,
            &false,
            &duration,
            &beneficiary, &total, &now, &(now + duration),
            &0i128, &true, &false, &duration,
        );

        env.ledger().with_mut(|l| l.timestamp = now + duration);
        let claimed = client.claim_tokens(&vault_id, &total);
        assert_eq!(claimed, total);
    }

    #[test]
    #[should_panic]
    fn test_lockup_only_claim_fails_before_end() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now      = env.ledger().timestamp();
        let duration = 1_000u64;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &100_000i128,
            &now,
            &(now + duration),
            &0i128,
            &true,
            &false,
            &duration,
            &beneficiary, &100_000i128, &now, &(now + duration),
            &0i128, &true, &false, &duration,
        );

        env.ledger().with_mut(|l| l.timestamp = now + duration - 1);
        client.claim_tokens(&vault_id, &1i128);
    }

    #[test]
    fn test_periodic_vesting_monthly_steps() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create vault with monthly vesting (30 days = 2,592,000 seconds)
        let amount = 1200000i128; // 1,200,000 tokens over 12 months = 100,000 per month
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 30 * 24 * 60 * 60; // 30 days in seconds
        let keeper_fee = 1000i128;
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );
        
        // Test 1: Before start time - no vesting
        env.ledger().with_mut(|l| l.timestamp = start_time - 1000);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0, "Should have no claimable tokens before start time");
        
        // Test 2: After 15 days (less than one step) - still no vesting (rounds down)
        env.ledger().with_mut(|l| l.timestamp = start_time + (15 * 24 * 60 * 60));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0, "Should have no claimable tokens before first step completes");
        
        // Test 3: After exactly 30 days - one step completed
        env.ledger().with_mut(|l| l.timestamp = start_time + step_duration);
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_monthly = amount / 12; // 100,000 tokens per month
        assert_eq!(claimable, expected_monthly, "Should have exactly one month of tokens after 30 days");
        
        // Test 4: After 45 days - still only one step (rounds down)
        env.ledger().with_mut(|l| l.timestamp = start_time + (45 * 24 * 60 * 60));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, expected_monthly, "Should still have only one month of tokens after 45 days");
        
        // Test 5: After 60 days - two steps completed
        env.ledger().with_mut(|l| l.timestamp = start_time + (2 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 2 * expected_monthly, "Should have two months of tokens after 60 days");
        
        // Test 6: After 6 months - 6 steps completed
        env.ledger().with_mut(|l| l.timestamp = start_time + (6 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 6 * expected_monthly, "Should have six months of tokens after 6 months");
        
        // Test 7: After end time - all tokens vested
        env.ledger().with_mut(|l| l.timestamp = end_time + 1000);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, amount, "Should have all tokens vested after end time");
    }

    #[test]
    fn test_periodic_vesting_weekly_steps() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create vault with weekly vesting (7 days = 604,800 seconds)
        let amount = 520000i128; // 520,000 tokens over 52 weeks = 10,000 per week
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 7 * 24 * 60 * 60; // 7 days in seconds
        let keeper_fee = 100i128;
        
    fn test_admin_access_control() {
        let (env, contract_id, client, admin, _token) = setup();
        let new_admin = Address::generate(&env);

        // Test: Admin can propose new admin
        client.propose_new_admin(&new_admin);
        assert_eq!(client.get_proposed_admin(), Some(new_admin.clone()));

        // Test: Proposed admin can accept ownership
        client.accept_ownership();

        // Verify admin transfer completed
        assert_eq!(client.get_admin(), new_admin);
        assert_eq!(client.get_proposed_admin(), None);
    }

#[test]
fn test_periodic_vesting_monthly_steps() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    
    // Initialize contract
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    // Create vault with monthly vesting (30 days = 2,592,000 seconds)
    let amount = 1200000i128; // 1,200,000 tokens over 12 months = 100,000 per month
    let start_time = 1000000u64;
    let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
    let step_duration = 30 * 24 * 60 * 60; // 30 days in seconds
    let keeper_fee = 1000i128;
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &amount,
        &start_time,
        &end_time,
        &keeper_fee,
        &false, // revocable
        &true,  // transferable
        &step_duration,
    );
    
    // Test 1: Before start time - no vesting
    env.ledger().set_timestamp(start_time - 1000);
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 0, "Should have no claimable tokens before start time");
    
    // Test 2: After 15 days (less than one step) - still no vesting (rounds down)
    env.ledger().set_timestamp(start_time + (15 * 24 * 60 * 60));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 0, "Should have no claimable tokens before first step completes");
    
    // Test 3: After exactly 30 days - one step completed
    env.ledger().set_timestamp(start_time + step_duration);
    let claimable = client.get_claimable_amount(&vault_id);
    let expected_monthly = amount / 12; // 100,000 tokens per month
    assert_eq!(claimable, 98630, "Should have exactly one month of tokens after 30 days");
    
    // Test 4: After 45 days - still only one step (rounds down)
    env.ledger().set_timestamp(start_time + (45 * 24 * 60 * 60));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 98630, "Should still have only one month of tokens after 45 days");
    
    // Test 5: After 60 days - two steps completed
    env.ledger().set_timestamp(start_time + (2 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 197260, "Should have two months of tokens after 60 days");
    
    // Test 6: After 6 months - 6 steps completed
    env.ledger().set_timestamp(start_time + (6 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 591780, "Should have six months of tokens after 6 months");
    
    // Test 7: After end time - all tokens vested
    env.ledger().set_timestamp(end_time + 1000);
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, amount, "Should have all tokens vested after end time");
}

#[test]
fn test_periodic_vesting_weekly_steps() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    
    // Initialize contract
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    // Set admin as caller
    
    // Create vault with weekly vesting (7 days = 604,800 seconds)
    let amount = 520000i128; // 520,000 tokens over 52 weeks = 10,000 per week
    let start_time = 1000000u64;
    let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
    let step_duration = 7 * 24 * 60 * 60; // 7 days in seconds
    let keeper_fee = 100i128;
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &amount,
        &start_time,
        &end_time,
        &keeper_fee,
        &false, // revocable
        &true,  // transferable
        &step_duration,
    );
    
    // Test: After 3 weeks - 3 steps completed
    env.ledger().set_timestamp(start_time + (3 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 29917, "Should have three weeks of tokens after 3 weeks");
    
    // Test: After 10 weeks - 10 steps completed
    env.ledger().set_timestamp(start_time + (10 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 99726, "Should have ten weeks of tokens after 10 weeks");
}

#[test]
fn test_linear_vesting_step_duration_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    
    // Initialize contract
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    // Set admin as caller
    
    // Create vault with linear vesting (step_duration = 0)
    let amount = 1200000i128;
    let start_time = 1000000u64;
    let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
    let step_duration = 0u64; // Linear vesting
    let keeper_fee = 1000i128;
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &amount,
        &start_time,
        &end_time,
        &keeper_fee,
        &false, // revocable
        &true,  // transferable
        &step_duration,
    );
    
    // Test: After 6 months (half the duration) - should have 50% vested
    env.ledger().set_timestamp(start_time + (182 * 24 * 60 * 60)); // ~6 months
    let claimable = client.get_claimable_amount(&vault_id);
    let expected_half = amount / 2; // 50% of tokens
    assert_eq!(claimable, 598356, "Should have 50% of tokens after half the time for linear vesting");
    
    // Test: After 3 months (quarter of the duration) - should have 25% vested
    env.ledger().set_timestamp(start_time + (91 * 24 * 60 * 60)); // ~3 months
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 299178, "Should have 25% of tokens after quarter of the time for linear vesting");
}

#[test]
fn test_periodic_vesting_claim_partial() {
    let (env, contract_id, client, admin, _token) = setup();
    
    // Create addresses for testing
    let beneficiary = Address::generate(&env);
    
    // Initialize contract
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    // Set beneficiary as caller for claiming
    
    // Create vault with monthly vesting
    let amount = 120000i128; // 120,000 tokens over 12 months = 10,000 per month
    let start_time = 1000000u64;
    let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
    let step_duration = 30 * 24 * 60 * 60; // 30 days
    let keeper_fee = 100i128;
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &amount,
        &start_time,
        &end_time,
        &keeper_fee,
        &false, // revocable
        &true,  // transferable
        &step_duration,
    );
    
    // Move time to 3 months
    env.ledger().set_timestamp(start_time + (3 * step_duration));
    
    // Claim partial amount
    let claim_amount = 15000i128; // Less than the 30,000 available
    let claimed = client.claim_tokens(&vault_id, &claim_amount);
    assert_eq!(claimed, claim_amount, "Should claim the requested amount");
    
    // Check remaining claimable
    let remaining_claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(remaining_claimable, 14589, "Should have 15,000 tokens remaining claimable");
    
    // Claim the rest
    let final_claim = client.claim_tokens(&vault_id, &remaining_claimable);
    assert_eq!(final_claim, remaining_claimable, "Should claim remaining tokens");
    
    // Check no more tokens available
    let no_more_claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(no_more_claimable, 0, "Should have no more claimable tokens");
}

#[test]
fn test_vault_creation_access_control() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    // Create addresses for testing
    let admin = Address::generate(&env);
    let vault_owner = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    // Test: Admin can create vaults
    let vault_id = client.create_vault_full(
        &vault_owner,
        &1000i128,
        &100u64,
        &200u64,
        &0i128,
        &false,
        &true,
        &0u64,
    );
    assert_eq!(vault_id, 1);
}


#[test]
fn test_batch_vault_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
    
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    
    let batch_data = BatchCreateData {
        recipients: vec![&env, recipient1, recipient2],
        amounts: vec![&env, 1000i128, 2000i128],
        start_times: vec![&env, 100u64, 100u64],
        end_times: vec![&env, 200u64, 200u64],
        keeper_fees: vec![&env, 10i128, 20i128],
        step_durations: vec![&env, 0u64, 0u64],
    };
    
    // Test: Admin can create batch vaults
    let vault_ids = client.batch_create_vaults_lazy(&batch_data);
    assert_eq!(vault_ids.len(), 2);
    assert_eq!(vault_ids.get(0), Some(1));
    assert_eq!(vault_ids.get(1), Some(2));
}

#[test]
fn test_milestone_unlocking_and_claim_limits() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let initial_supply = 1000000000i128;
    env.mock_all_auths();
    client.initialize(&admin, &initial_supply);
}

#[test]
fn test_step_vesting_fuzz() {
    let (env, contract_id, client, admin, _token) = setup();
    let beneficiary = Address::generate(&env);

    // Fuzz testing with prime numbers to check for truncation errors
    // Primes: 1009 (amount), 17 (step), 101 (duration)
    let total_amount = 1009i128;
    let start_time = 1000u64;
    let duration = 101u64; // Prime duration
    let end_time = start_time + duration;
    let step_duration = 17u64; // Prime step
    
    let vault_id = client.create_vault_full(
        &beneficiary,
        &total_amount,
        &start_time,
        &end_time,
        &0i128,
        &true,
        &true,
        &step_duration,
    );

    // Advance time to end
    env.ledger().with_mut(|li| {
        li.timestamp = end_time + 1;
    });

    // Claim all
    let claimed = client.claim_tokens(&vault_id, &total_amount);
    
    // Assert full amount is claimed
    assert_eq!(claimed, total_amount);
    
    // Verify vault state
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.released_amount, total_amount);
}
    #[test]
    fn test_periodic_vesting_monthly_steps() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Initialize contract
        let initial_supply = 1000000i128;
        env.mock_all_auths();
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Create vault with monthly vesting (30 days = 2,592,000 seconds)
        let amount = 1200000i128; // 1,200,000 tokens over 12 months = 100,000 per month
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 30 * 24 * 60 * 60; // 30 days in seconds
        let keeper_fee = 1000i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );
        
        // Test: After 3 weeks - 3 steps completed
        env.ledger().with_mut(|l| l.timestamp = start_time + (3 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_weekly = 10000i128; // 10,000 tokens per week
        assert_eq!(claimable, 3 * expected_weekly, "Should have three weeks of tokens after 3 weeks");
        
        // Test: After 10 weeks - 10 steps completed
        env.ledger().with_mut(|l| l.timestamp = start_time + (10 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 10 * expected_weekly, "Should have ten weeks of tokens after 10 weeks");
    }

    #[test]
    fn test_linear_vesting_step_duration_zero() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create vault with linear vesting (step_duration = 0)
        let amount = 1200000i128;
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 0u64; // Linear vesting
        let keeper_fee = 1000i128;
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );
        
        // Test: After 6 months (half the duration) - should have 50% vested
        env.ledger().with_mut(|l| l.timestamp = start_time + (182 * 24 * 60 * 60)); // ~6 months
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_half = amount / 2; // 50% of tokens
        // Due to integer math and exactly 182 days vs 365, it will be close
        assert!(claimable > 598000i128 && claimable < 602000i128, "Should have ~50% of tokens after half the time for linear vesting");
        
        // Test: After 3 months (quarter of the duration) - should have 25% vested
        env.ledger().with_mut(|l| l.timestamp = start_time + (91 * 24 * 60 * 60)); // ~3 months
        let claimable = client.get_claimable_amount(&vault_id);
        assert!(claimable > 298000i128 && claimable < 302000i128, "Should have ~25% of tokens after quarter of the time for linear vesting");
    }

    #[test]
    fn test_periodic_vesting_claim_partial() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create vault with monthly vesting
        let amount = 120000i128; // 120,000 tokens over 12 months = 10,000 per month
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 30 * 24 * 60 * 60; // 30 days
        let keeper_fee = 100i128;
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );
        
        // Move time to 3 months
        env.ledger().with_mut(|l| l.timestamp = start_time + (3 * step_duration));
        
        // Claim partial amount
        let claim_amount = 15000i128; // Less than the 30,000 available
        let claimed = client.claim_tokens(&vault_id, &claim_amount);
        assert_eq!(claimed, claim_amount, "Should claim the requested amount");
        
        // Check remaining claimable
        let remaining_claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(remaining_claimable, 15000i128, "Should have 15,000 tokens remaining claimable");
        
        // Claim the rest
        let final_claim = client.claim_tokens(&vault_id, &remaining_claimable);
        assert_eq!(final_claim, remaining_claimable, "Should claim remaining tokens");
        
        // Check no more tokens available
        let no_more_claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(no_more_claimable, 0, "Should have no more claimable tokens");
    }

    #[test]
    fn test_step_vesting_fuzz() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        
        // Fuzz testing with prime numbers to check for truncation errors
        // Primes: 1009 (amount), 17 (step), 101 (duration)
        let total_amount = 1009i128;
        let start_time = 1000u64;
        let duration = 101u64; // Prime duration
        let end_time = start_time + duration;
        let step_duration = 17u64; // Prime step
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &total_amount,
            &start_time,
            &end_time,
            &0i128,
            &true,
            &true,
            &step_duration,
        );

        // Advance time to end
        env.ledger().with_mut(|li| {
            li.timestamp = end_time + 1;
        });

        // Claim all
        let claimed = client.claim_tokens(&vault_id, &total_amount);
        
        // Assert full amount is claimed
        assert_eq!(claimed, total_amount);
        
        // Verify vault state
        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, total_amount);

        // Test 1: Before start time - no vesting
        env.ledger().set_timestamp(start_time - 1000);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 0,
            "Should have no claimable tokens before start time"
        );

        // Test 2: After 15 days (less than one step) - still no vesting (rounds down)
        env.ledger().set_timestamp(start_time + (15 * 24 * 60 * 60));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 0,
            "Should have no claimable tokens before first step completes"
        );

        // Test 3: After exactly 30 days - one step completed
        env.ledger().set_timestamp(start_time + step_duration);
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_monthly = amount / 12; // 100,000 tokens per month
        assert_eq!(
            claimable, expected_monthly,
            "Should have exactly one month of tokens after 30 days"
        );

        // Test 4: After 45 days - still only one step (rounds down)
        env.ledger().set_timestamp(start_time + (45 * 24 * 60 * 60));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, expected_monthly,
            "Should still have only one month of tokens after 45 days"
        );

        // Test 5: After 60 days - two steps completed
        env.ledger().set_timestamp(start_time + (2 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable,
            2 * expected_monthly,
            "Should have two months of tokens after 60 days"
        );

        // Test 6: After 6 months - 6 steps completed
        env.ledger().set_timestamp(start_time + (6 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable,
            6 * expected_monthly,
            "Should have six months of tokens after 6 months"
        );

        // Test 7: After end time - all tokens vested
        env.ledger().set_timestamp(end_time + 1000);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, amount,
            "Should have all tokens vested after end time"
        );
    }
    
    pub fn unstake(env: Env, vault_id: u64, amount: i128) {
        env.events()
            .publish((Symbol::new(&env, "unstake"), vault_id), amount);

    #[test]
    fn test_periodic_vesting_weekly_steps() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Initialize contract
        let initial_supply = 1000000i128;
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Set admin as caller

        // Create vault with weekly vesting (7 days = 604,800 seconds)
        let amount = 520000i128; // 520,000 tokens over 52 weeks = 10,000 per week
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 7 * 24 * 60 * 60; // 7 days in seconds
        let keeper_fee = 100i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );

        // Test: After 3 weeks - 3 steps completed
        env.ledger().set_timestamp(start_time + (3 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_weekly = 10000i128; // 10,000 tokens per week
        assert_eq!(
            claimable,
            3 * expected_weekly,
            "Should have three weeks of tokens after 3 weeks"
        );

        // Test: After 10 weeks - 10 steps completed
        env.ledger()
            .set_timestamp(start_time + (10 * step_duration));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable,
            10 * expected_weekly,
            "Should have ten weeks of tokens after 10 weeks"
        );
    }

    #[test]
    fn test_linear_vesting_step_duration_zero() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Initialize contract
        let initial_supply = 1000000i128;
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Set admin as caller

        // Create vault with linear vesting (step_duration = 0)
        let amount = 1200000i128;
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 0u64; // Linear vesting
        let keeper_fee = 1000i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );

        // Test: After 6 months (half the duration) - should have 50% vested
        env.ledger()
            .set_timestamp(start_time + (182 * 24 * 60 * 60)); // ~6 months
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_half = amount / 2; // 50% of tokens
        assert_eq!(
            claimable, expected_half,
            "Should have 50% of tokens after half the time for linear vesting"
        );

        // Test: After 3 months (quarter of the duration) - should have 25% vested
        env.ledger().set_timestamp(start_time + (91 * 24 * 60 * 60)); // ~3 months
        let claimable = client.get_claimable_amount(&vault_id);
        let expected_quarter = amount / 4; // 25% of tokens
        assert_eq!(
            claimable, expected_quarter,
            "Should have 25% of tokens after quarter of the time for linear vesting"
        );
    }

    #[test]
    fn test_periodic_vesting_claim_partial() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Initialize contract
        let initial_supply = 1000000i128;
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Set beneficiary as caller for claiming

        // Create vault with monthly vesting
        let amount = 120000i128; // 120,000 tokens over 12 months = 10,000 per month
        let start_time = 1000000u64;
        let end_time = start_time + (365 * 24 * 60 * 60); // 1 year
        let step_duration = 30 * 24 * 60 * 60; // 30 days
        let keeper_fee = 100i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &false, // revocable
            &true,  // transferable
            &step_duration,
        );

        // Move time to 3 months
        env.ledger().set_timestamp(start_time + (3 * step_duration));

        // Claim partial amount
        let claim_amount = 15000i128; // Less than the 30,000 available
        let claimed = client.claim_tokens(&vault_id, &claim_amount);
        assert_eq!(claimed, claim_amount, "Should claim the requested amount");

        // Check remaining claimable
        let remaining_claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            remaining_claimable, 15000i128,
            "Should have 15,000 tokens remaining claimable"
        );

        // Claim the rest
        let final_claim = client.claim_tokens(&vault_id, &remaining_claimable);
        assert_eq!(
            final_claim, remaining_claimable,
            "Should claim remaining tokens"
        );

        // Check no more tokens available
        let no_more_claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(no_more_claimable, 0, "Should have no more claimable tokens");
    }

    #[test]
    fn test_vault_creation_access_control() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let vault_owner = Address::generate(&env);

        // Initialize contract with admin
        let initial_supply = 1000000i128;
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Test: Admin can create vaults
        let vault_id = client.create_vault_full(
            &vault_owner,
            &1000i128,
            &100u64,
            &200u64,
            &0i128,
            &false,
            &true,
            &0u64,
        );
        assert_eq!(vault_id, 1);
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
        env.mock_all_auths();
        client.initialize(&admin, &initial_supply);

        // Create batch data
        let batch_data = BatchCreateData {
            recipients: vec![&env, recipient1.clone(), recipient2.clone()],
            amounts: vec![&env, 1000i128, 2000i128],
            start_times: vec![&env, 100u64, 150u64],
            end_times: vec![&env, 200u64, 250u64],
            keeper_fees: vec![&env, 0i128, 0i128],
            step_durations: vec![&env, 0u64, 0u64],
        };

        #[test]
        fn test_batch_vault_creation() {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);

            let admin = Address::generate(&env);
            let initial_supply = 1000000i128;
            env.mock_all_auths();
            client.initialize(&admin, &initial_supply);

            let recipient1 = Address::generate(&env);
            let recipient2 = Address::generate(&env);

            let batch_data = BatchCreateData {
                recipients: vec![&env, recipient1, recipient2],
                amounts: vec![&env, 1000i128, 2000i128],
                start_times: vec![&env, 100u64, 100u64],
                end_times: vec![&env, 200u64, 200u64],
                keeper_fees: vec![&env, 10i128, 20i128],
                step_durations: vec![&env, 0u64, 0u64],
            };

            // Test: Admin can create batch vaults
            let vault_ids = client.batch_create_vaults_lazy(&batch_data);
            assert_eq!(vault_ids.len(), 2);
            assert_eq!(vault_ids.get(0), Some(1));
            assert_eq!(vault_ids.get(1), Some(2));
        }

        #[test]
        fn test_milestone_unlocking_and_claim_limits() {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);

            let admin = Address::generate(&env);
            let initial_supply = 1000000i128;
            env.mock_all_auths();
            client.initialize(&admin, &initial_supply);
        }

        #[test]
        fn test_step_vesting_fuzz() {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);

            let admin = Address::generate(&env);
            let beneficiary = Address::generate(&env);

            let initial_supply = 1_000_000_000_000i128;
            env.mock_all_auths();
            client.initialize(&admin, &initial_supply);

            // Fuzz testing with prime numbers to check for truncation errors
            // Primes: 1009 (amount), 17 (step), 101 (duration)
            let total_amount = 1009i128;
            let start_time = 1000u64;
            let duration = 101u64; // Prime duration
            let end_time = start_time + duration;
            let step_duration = 17u64; // Prime step

            let vault_id = client.create_vault_full(
                &beneficiary,
                &total_amount,
                &start_time,
                &end_time,
                &0i128,
                &true,
                &true,
                &step_duration,
            );

            // Advance time to end
            env.ledger().with_mut(|li| {
                li.timestamp = end_time + 1;
            });

            // Claim all
            let claimed = client.claim_tokens(&vault_id, &total_amount);

            // Assert full amount is claimed
            assert_eq!(claimed, total_amount);

            // Verify vault state
            let vault = client.get_vault(&vault_id);
            assert_eq!(vault.released_amount, total_amount);
        }

        // Mock Staking Contract for testing cross-contract calls
        #[contract]
        pub struct MockStakingContract;

        #[contractimpl]
        impl MockStakingContract {
            pub fn stake(env: Env, vault_id: u64, amount: i128, _validator: Address) {
                env.events()
                    .publish((Symbol::new(&env, "stake"), vault_id), amount);
            }
        }

        pub fn unstake(env: Env, vault_id: u64, amount: i128) {
            env.events()
                .publish((Symbol::new(&env, "unstake"), vault_id), amount);
        }
    }

    // -------------------------------------------------------------------------
    // Irrevocable vault
    // -------------------------------------------------------------------------

    #[test]
    fn test_mark_irrevocable_flag() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        assert!(!client.is_vault_irrevocable(&vault_id));
        client.mark_irrevocable(&vault_id);
        assert!(client.is_vault_irrevocable(&vault_id));
    }

    #[test]
    #[should_panic]
    fn test_revoke_irrevocable_vault_panics() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        client.mark_irrevocable(&vault_id);
        client.revoke_tokens(&vault_id);
    }

    // -------------------------------------------------------------------------
    // Clawback
    // -------------------------------------------------------------------------

    #[test]
    fn test_clawback_within_grace_period() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &5_000i128,
            &(now + 100),
            &(now + 10_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &5_000i128, &(now + 100), &(now + 10_000),
            &0i128, &true, &false, &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 3_599);
        let returned = client.clawback_vault(&vault_id);
        assert_eq!(returned, 5_000i128);
    }

    #[test]
    #[should_panic]
    fn test_clawback_after_grace_period_panics() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &5_000i128,
            &(now + 100),
            &(now + 10_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &5_000i128, &(now + 100), &(now + 10_000),
            &0i128, &true, &false, &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 3_601);
        client.clawback_vault(&vault_id);
    }

    // -------------------------------------------------------------------------
    // Milestones
    // -------------------------------------------------------------------------

    #[test]
    fn test_milestone_unlock_and_claim() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        let milestones = vec![
            &env,
            Milestone {
                id: 1,
                percentage: 50,
                is_unlocked: false,
            },
            Milestone {
                id: 2,
                percentage: 50,
                is_unlocked: false,
            },
            Milestone { id: 1, percentage: 50, is_unlocked: false },
            Milestone { id: 2, percentage: 50, is_unlocked: false },
        ];
        client.set_milestones(&vault_id, &milestones);

        client.unlock_milestone(&vault_id, &1u64);
        let claimed = client.claim_tokens(&vault_id, &500i128);
        assert_eq!(claimed, 500i128);

        client.unlock_milestone(&vault_id, &2u64);
        let claimed2 = client.claim_tokens(&vault_id, &500i128);
        assert_eq!(claimed2, 500i128);
    }

    #[test]
    #[should_panic]
    fn test_claim_before_any_milestone_unlocked_panics() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        let milestones = vec![
            &env,
            Milestone {
                id: 1,
                percentage: 100,
                is_unlocked: false,
            },
            Milestone { id: 1, percentage: 100, is_unlocked: false },
        ];
        client.set_milestones(&vault_id, &milestones);
        client.claim_tokens(&vault_id, &1i128);
    }

    // -------------------------------------------------------------------------
    // Rotate beneficiary key
    // -------------------------------------------------------------------------

    #[test]
    fn test_rotate_beneficiary_key() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary     = Address::generate(&env);
        let new_beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &1_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        client.rotate_beneficiary_key(&vault_id, &new_beneficiary);

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.owner, new_beneficiary);
        assert_eq!(vault.delegate, None);
    }

    // -------------------------------------------------------------------------
    // Invariant
    // -------------------------------------------------------------------------

    #[test]
    fn test_invariant_holds_after_operations() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();
        let initial_supply = 1_000_000i128;
        let vault_amount = 10_000i128;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &10_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );
        assert!(client.check_invariant());
        // After vault creation: admin_balance = 990_000, vault locked = 10_000
        let vault_id = client.create_vault_full(
            &beneficiary, &vault_amount, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );
        assert!(client.check_invariant(), "invariant failed after vault creation");
        let (locked, _claimed, admin_bal) = client.get_contract_state();
        assert_eq!(locked + admin_bal, initial_supply, "locked + admin should equal initial supply before any claims");

        // After partial claim: 5_000 paid out, 5_000 still locked
        env.ledger().with_mut(|l| l.timestamp = now + 500);
        client.claim_tokens(&vault_id, &5_000i128);
        assert!(client.check_invariant(), "invariant failed after partial claim");
        let (locked2, _claimed2, admin_bal2) = client.get_contract_state();
        // 5_000 was paid out to beneficiary, so locked + admin = initial - 5_000
        assert_eq!(locked2 + admin_bal2, initial_supply - 5_000i128, "5000 should have left the pool");

        // After revoke: remaining 5_000 returned to admin
        client.revoke_tokens(&vault_id);
        assert!(client.check_invariant(), "invariant failed after revoke");
        let (locked3, _claimed3, admin_bal3) = client.get_contract_state();
        // Still only 5_000 paid out total (the revoked portion came back to admin)
        assert_eq!(locked3, 0i128, "no tokens should remain locked after full revoke");
        assert_eq!(admin_bal3, initial_supply - 5_000i128, "admin should hold everything except what was claimed");
        assert_eq!(locked3 + admin_bal3, initial_supply - 5_000i128, "invariant: only claimed tokens are gone");
    }

    // =========================================================================
    // rescue_unallocated_tokens
    // =========================================================================

    #[test]
    fn test_rescue_basic_no_vaults() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 5_000i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&admin),       5_000i128);
        assert_eq!(tok.balance(&contract_id), 0i128);
    }

    #[test]
    fn test_rescue_only_surplus_above_vault_liability() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.create_vault_full(
            &beneficiary,
            &3_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &3_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        // 3000 liability + 2000 stray
        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 2_000i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&admin),       2_000i128);
        assert_eq!(tok.balance(&contract_id), 3_000i128);
    }

    #[test]
    fn test_rescue_after_partial_claim_adjusts_liability() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &4_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &4_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        // Claim 1000 → remaining liability 3000
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        client.claim_tokens(&vault_id, &1_000i128);

        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 2_000i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&admin),       2_000i128);
        assert_eq!(tok.balance(&contract_id), 3_000i128);
    }

    #[test]
    fn test_rescue_multiple_vaults_correct_liability_sum() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let now = env.ledger().timestamp();

        // Three vaults × 2000 = 6000 total liability
        for _ in 0..3 {
            let b = Address::generate(&env);
            client.create_vault_full(
                &b,
                &2_000i128,
                &now,
                &(now + 1_000),
                &0i128,
                &true,
                &false,
                &0u64,
                &b, &2_000i128, &now, &(now + 1_000),
                &0i128, &true, &false, &0u64,
            );
        }

        // 6000 liability + 1000 stray
        mint_to(&env, &token_addr, &contract_id, 7_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 1_000i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&admin),       1_000i128);
        assert_eq!(tok.balance(&contract_id), 6_000i128);
    }

    #[test]
    fn test_rescue_after_full_claim_all_tokens_rescuable() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &2_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &2_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        // Claim everything → liability = 0
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        client.claim_tokens(&vault_id, &2_000i128);

        // Stray deposit after full claim
        mint_to(&env, &token_addr, &contract_id, 500i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 500i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&admin),       500i128);
        assert_eq!(tok.balance(&contract_id), 0i128);
    }

    #[test]
    fn test_rescue_after_revoke_liability_drops_to_zero() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &3_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &3_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        // Revoke → vault liability drops to 0
        client.revoke_tokens(&vault_id);

        mint_to(&env, &token_addr, &contract_id, 3_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 3_000i128);
    }

    #[test]
    fn test_rescue_tokens_go_to_current_admin_after_transfer() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        // Transfer admin to new_admin
        let new_admin = Address::generate(&env);
        client.propose_new_admin(&new_admin);
        client.accept_ownership();
        assert_eq!(client.get_admin(), new_admin);

        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 1_000i128);

        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&new_admin), 1_000i128); // new admin gets tokens
        assert_eq!(tok.balance(&admin),     0i128);     // old admin gets nothing
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_when_no_surplus() {
        let (env, contract_id, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.create_vault_full(
            &beneficiary,
            &3_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
            &beneficiary, &3_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        // Mint exactly the liability — zero surplus
        mint_to(&env, &token_addr, &contract_id, 3_000i128);

        client.rescue_unallocated_tokens(&token_addr); // must panic
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_when_contract_balance_zero() {
        let (env, _cid, client, admin) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        client.rescue_unallocated_tokens(&token_addr); // must panic
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_for_non_whitelisted_token() {
        let (env, contract_id, client, admin) = setup();
        // Register but do NOT whitelist
        let token_addr = register_token(&env, &admin);
        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        client.rescue_unallocated_tokens(&token_addr); // must panic
    }



    // =========================================================================
    // Yield demonstration tests
    // =========================================================================

    #[test]
    fn test_yield_is_distributed_on_claim() {
        let (env, contract_id, client, _admin, token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &10_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        // Simulate yield
        let stellar = token::StellarAssetClient::new(&env, &token);
        stellar.mint(&contract_id, &2_000i128);

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        let claimed = client.claim_tokens(&vault_id, &10_000i128);

        assert_eq!(claimed, 12_000i128); // principal + all yield
    }

    #[test]
    fn test_yield_on_partial_claim() {
        let (env, contract_id, client, _admin, token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &10_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let stellar = token::StellarAssetClient::new(&env, &token);
        stellar.mint(&contract_id, &2_000i128);

        env.ledger().with_mut(|l| l.timestamp = now + 500);
        let claimed = client.claim_tokens(&vault_id, &5_000i128);

        assert_eq!(claimed, 6_000i128); // 5k principal + 1k yield
    }

    #[test]
    fn test_yield_proportional_with_multiple_vaults() {
        let (env, contract_id, client, _admin, token) = setup();
        let now = env.ledger().timestamp();

        let v1 = client.create_vault_full(
            &Address::generate(&env),
            &10_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );
        let v2 = client.create_vault_full(
            &Address::generate(&env),
            &20_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let stellar = token::StellarAssetClient::new(&env, &token);
        stellar.mint(&contract_id, &6_000i128);

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let claimed1 = client.claim_tokens(&v1, &10_000i128);
        let claimed2 = client.claim_tokens(&v2, &20_000i128);

        assert_eq!(claimed1, 12_000i128);
        assert_eq!(claimed2, 24_000i128);
    }

    #[test]
    #[should_panic(expected = "Cannot rescue yield-bearing token")]
    fn test_rescue_yield_token_panics() {
        let (env, _cid, client, _admin, token) = setup();
        client.rescue_unallocated_tokens(&token);
    }

    // -------------------------------------------------------------------------
    // Zero-duration vault fuzz tests (Issue #41)
    // -------------------------------------------------------------------------

    #[test]
    fn test_zero_duration_vault_immediate_unlock() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &5_000i128,
            &now,
            &now,
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 5_000i128,
            "zero-duration vault should unlock 100% immediately"
        );
    }

    #[test]
    fn test_zero_duration_vault_claim_full() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &10_000i128,
            &now,
            &now,
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let claimed = client.claim_tokens(&vault_id, &10_000i128);
        assert_eq!(
            claimed, 10_000i128,
            "should claim full amount from zero-duration vault"
        );

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, 10_000i128);
    }

    #[test]
    fn test_zero_duration_vault_before_start() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let future = env.ledger().timestamp() + 1_000;

        let vault_id = client.create_vault_full(
            &beneficiary,
            &5_000i128,
            &future,
            &future,
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 0,
            "zero-duration vault should not unlock before start_time"
        );
    }

    #[test]
    fn test_zero_cliff_vault_vests_immediately() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &10_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 500);
        let claimable = client.get_claimable_amount(&vault_id);
        assert!(
            claimable > 0,
            "zero-cliff vault should vest from start_time"
        );
    }

    #[test]
    fn test_zero_amount_vault_no_claimable() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &0i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 0,
            "zero-amount vault should have nothing claimable"
        );
    }

    #[test]
    fn test_zero_duration_zero_amount_vault() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary,
            &0i128,
            &now,
            &now,
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(
            claimable, 0,
            "zero-duration + zero-amount vault should have nothing claimable"
        );
    }

#[test]
fn test_vault_start_time_immutable() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VestingContract, ());
    let client = VestingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let initial_supply = 1000000000i128;
    client.initialize(&admin, &initial_supply);

    // Create a vault
    let owner = Address::generate(&env);
    let amount = 1000i128;
    let start_time = 123456789u64;
    let end_time = start_time + 10000;
    let cliff_duration = 10i128; // Renamed from keeper_fee to match create_vault_full
    let is_revocable = true;
    let is_transferable = false;
    let step_duration = 0u64;
    let vault_id = client.create_vault_full( // Changed to create_vault_full
        &owner,
        &amount,
        &start_time,
        &end_time,
        &cliff_duration, // Changed from keeper_fee
        &is_revocable,
        &is_transferable,
        &step_duration,
    );

    // Try to change start_time or cliff_duration (should not be possible)
    let vault = client.get_vault(&vault_id);
    let original_start_time = vault.start_time;

    // Attempt to update vault via admin functions (should not affect start_time/cliff_duration)
    client.mark_irrevocable(&vault_id);
    client.transfer_beneficiary(&vault_id, &Address::generate(&env));
    client.set_delegate(&vault_id, &Some(Address::generate(&env)));

    let updated_vault = client.get_vault(&vault_id);
    assert_eq!(updated_vault.start_time, original_start_time);
}

#[test]
fn test_global_pause_functionality() {
    let (env, contract_id, client, admin, _token) = setup();
    
    // Create addresses for testing
    let beneficiary = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract with admin
    let initial_supply = 1000000000i128;
    client.initialize(&admin, &initial_supply);
    
    // Verify initial state is unpaused
    assert_eq!(client.is_paused(), false);
    assert_eq!(client.is_paused(), false); // Should still be unpaused
    
    // Test: Admin can pause the contract
    env.mock_all_auths();    
    client.toggle_pause();
    assert_eq!(client.is_paused(), true); // Should now be paused
    
    // Create a vault for testing claims
    let now = env.ledger().timestamp();
    let total_amount = 1000i128; // Define total_amount
    let vault_id = client.create_vault_full(
        &beneficiary,
        &total_amount,
        &now,
        &(now + 1_000),
        &0i128,
        &false,
        &true,
        &0u64,
    );
    
    // Move time to make tokens claimable
    env.ledger().set_timestamp(now + 1001);
    
    // Set beneficiary as caller
    // (env.mock_all_auths() already active)    
    // Test: Claims should fail when paused
    let result = client.try_claim_tokens(&vault_id, &100i128);
    assert!(result.is_err());
    
    // Test: Delegate claims should also fail when paused
    let delegate = Address::generate(&env);
    client.set_delegate(&vault_id, &Some(delegate.clone()));    
    let result2 = client.try_claim_as_delegate(&vault_id, &100i128);
    assert!(result2.is_err());
    
    // Test: Admin can unpause the contract    
    client.toggle_pause();
    assert_eq!(client.is_paused(), false); // Should be unpaused
    
    // Test: Claims should work after unpausing    
    let claimed = client.claim_tokens(&vault_id, &100i128);
    assert_eq!(claimed, 100i128); // Should succeed
}
    #[test]
    fn test_vault_start_time_immutable() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let initial_supply = 1000000i128;
        client.initialize(&admin, &initial_supply);

        // Create a vault
        let owner = Address::generate(&env);
        let amount = 1000i128;
        let start_time = 123456789u64;
        let end_time = start_time + 10000;
        let keeper_fee = 10i128;
        let is_revocable = false;
        let is_transferable = false;
        let step_duration = 0u64;
        let vault_id = client.create_vault(
            &owner,
            &amount,
            &start_time,
            &end_time,
            &keeper_fee,
            &is_revocable,
            &is_transferable,
            &step_duration,
        );

        // Try to change start_time or cliff_duration (should not be possible)
        let vault = client.get_vault(&vault_id);
        let original_start_time = vault.start_time;
        let original_cliff_duration = vault.cliff_duration;

    // -------------------------------------------------------------------------
    // Periodic Vesting Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_monthly_vesting_step_function() {
        let (env, contract_id, client, admin, token_addr) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create a 12-month vault with monthly step duration (30 days = 2,592,000 seconds)
        let monthly_seconds = 30 * 24 * 60 * 60; // 2,592,000 seconds
        let total_amount = 12000i128; // 1000 tokens per month
        let start_time = 10000u64;
        let end_time = start_time + (12 * monthly_seconds); // 12 months
        
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &total_amount,
            &start_time,
            &end_time,
            &0i128, // no keeper fee
            &false, // revocable
            &true,  // transferable
            &monthly_seconds, // monthly step duration
        );

        // Test 1: Before start time - should have 0 vested
        env.ledger().set_timestamp(start_time - 1000);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0i128, "Should have 0 vested before start time");

        // Test 2: Exactly at start time - should have 0 vested
        env.ledger().set_timestamp(start_time);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0i128, "Should have 0 vested at start time");

        // Test 3: Half way through first month - should still have 0 vested (rounding down)
        env.ledger().set_timestamp(start_time + (monthly_seconds / 2));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0i128, "Should have 0 vested half way through first month");

        // Test 4: Exactly 1 month completed - should have 1000 vested
        env.ledger().set_timestamp(start_time + monthly_seconds);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 1000i128, "Should have 1000 vested after 1 month");

        // Test 5: 1 month + 1 day - should still have 1000 vested (rounding down)
        env.ledger().set_timestamp(start_time + monthly_seconds + (24 * 60 * 60));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 1000i128, "Should have 1000 vested after 1 month + 1 day");

        // Test 6: Exactly 6 months completed - should have 6000 vested
        env.ledger().set_timestamp(start_time + (6 * monthly_seconds));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 6000i128, "Should have 6000 vested after 6 months");

        // Test 7: Exactly 12 months completed - should have all 12000 vested
        env.ledger().set_timestamp(end_time);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 12000i128, "Should have all 12000 vested at end time");

        // Test 8: After end time - should still have all 12000 vested
        env.ledger().set_timestamp(end_time + monthly_seconds);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 12000i128, "Should have all 12000 vested after end time");
    }

    #[test]
    fn test_quarterly_vesting() {
        let (env, contract_id, client, admin, token_addr) = setup();
        let beneficiary = Address::generate(&env);
        
        // Create a 1-year vault with quarterly step duration (3 months = 90 days)
        let quarterly_seconds = 90 * 24 * 60 * 60; // 7,776,000 seconds
        let total_amount = 4000i128; // 1000 tokens per quarter
        let start_time = 10000u64;
        let end_time = start_time + (4 * quarterly_seconds); // 4 quarters
        
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });
        
        let vault_id = client.create_vault_full(
            &beneficiary,
            &total_amount,
            &start_time,
            &end_time,
            &0i128,
            &false,
            &true,
            &quarterly_seconds,
        );

        // Test: Should vest in quarterly steps
        env.ledger().set_timestamp(start_time + quarterly_seconds);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 1000i128, "Should have 1000 vested after 1 quarter");

        env.ledger().set_timestamp(start_time + (2 * quarterly_seconds));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 2000i128, "Should have 2000 vested after 2 quarters");

        env.ledger().set_timestamp(start_time + (3 * quarterly_seconds));
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 3000i128, "Should have 3000 vested after 3 quarters");

        env.ledger().set_timestamp(end_time);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 4000i128, "Should have all 4000 vested at end");
    }

    #[test]
    fn test_linear_vs_periodic_vesting() {
        let (env, contract_id, client, admin, token_addr) = setup();
        let beneficiary = Address::generate(&env);
        
        let total_amount = 12000i128;
        let start_time = 10000u64;
        let duration = 12 * 30 * 24 * 60 * 60; // 12 months in seconds
        let end_time = start_time + duration;
        let monthly_seconds = 30 * 24 * 60 * 60;
        
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });
        
        // Create linear vesting vault
        let linear_vault_id = client.create_vault_full(
            &beneficiary,
            &total_amount,
            &start_time,
            &end_time,
            &0i128,
            &false,
            &true,
            &0u64, // step_duration = 0 for linear
        );

        // Create periodic vesting vault
        let periodic_vault_id = client.create_vault_full(
            &beneficiary,
            &total_amount,
            &start_time,
            &end_time,
            &0i128,
            &false,
            &true,
            &monthly_seconds, // monthly steps
        );

        // Test at 6 months: linear should have 6000, periodic should have 6000
        env.ledger().set_timestamp(start_time + (6 * monthly_seconds));
        
        let linear_claimable = client.get_claimable_amount(&linear_vault_id);
        let periodic_claimable = client.get_claimable_amount(&periodic_vault_id);
        
        assert_eq!(linear_claimable, 6000i128, "Linear should have 6000 at 6 months");
        assert_eq!(periodic_claimable, 6000i128, "Periodic should have 6000 at 6 months");

        // Test at 6.5 months: linear should have ~6500, periodic should still have 6000
        env.ledger().set_timestamp(start_time + (6 * monthly_seconds) + (monthly_seconds / 2));
        
        let linear_claimable = client.get_claimable_amount(&linear_vault_id);
        let periodic_claimable = client.get_claimable_amount(&periodic_vault_id);
        
        assert!(linear_claimable > 6000i128 && linear_claimable < 7000i128, 
                "Linear should be between 6000-7000 at 6.5 months");
        assert_eq!(periodic_claimable, 6000i128, 
                  "Periodic should still have 6000 at 6.5 months (rounding down)");
    }

    #[test]
    fn test_periodic_vesting_edge_cases() {
        let (env, contract_id, client, admin, token_addr) = setup();
        let beneficiary = Address::generate(&env);
        
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });
        
        // Test 1: Zero duration vault
        let vault_id1 = client.create_vault_full(
            &beneficiary,
            &1000i128,
            &10000u64,
            &10000u64, // same start and end time
            &0i128,
            &false,
            &true,
            &2592000u64, // monthly
        );
        
        let claimable = client.get_claimable_amount(&vault_id1);
        assert_eq!(claimable, 1000i128, "Zero duration vault should vest all immediately");

        // Test 2: Step duration longer than total duration
        let vault_id2 = client.create_vault_full(
            &beneficiary,
            &1000i128,
            &10000u64,
            &20000u64, // 10,000 seconds duration
            &0i128,
            &false,
            &true,
            &86400u64, // 1 day step (longer than duration)
        );
        
        env.ledger().set_timestamp(15000u64); // in the middle
        let claimable = client.get_claimable_amount(&vault_id2);
        assert_eq!(claimable, 0i128, "Should have 0 vested when step > duration");

        env.ledger().set_timestamp(20000u64); // at end
        let claimable = client.get_claimable_amount(&vault_id2);
        assert_eq!(claimable, 1000i128, "Should have all vested at end time");
    }

        // Attempt to update vault via admin functions (should not affect start_time/cliff_duration)
        client.mark_irrevocable(&vault_id);
        client.transfer_beneficiary(&vault_id, &Address::generate(&env));
        client.set_delegate(&vault_id, &Some(Address::generate(&env)));

        let updated_vault = client.get_vault(&vault_id);
        assert_eq!(updated_vault.start_time, original_start_time);
        assert_eq!(updated_vault.cliff_duration, original_cliff_duration);
    }

    #[test]
    fn test_global_pause_functionality() {
        let env = Env::default();
        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        // Create addresses for testing
        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let unauthorized_user = Address::generate(&env);

        // Initialize contract with admin
        let initial_supply = 1000000i128;
        client.initialize(&admin, &initial_supply);

        // Verify initial state is unpaused
        assert_eq!(client.is_paused(), false);

        // Test: Unauthorized user cannot toggle pause
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&unauthorized_user);
        });

        let result = std::panic::catch_unwind(|| {
            client.toggle_pause();
        });
        assert!(result.is_err());
        assert_eq!(client.is_paused(), false); // Should still be unpaused

        // Test: Admin can pause the contract
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });

        client.toggle_pause();
        assert_eq!(client.is_paused(), true); // Should now be paused

        // Create a vault for testing claims
        let now = env.ledger().timestamp();
        let vault_id = client.create_vault_full(
            &beneficiary,
            &1000i128,
            &now,
            &(now + 1000),
            &0i128,
            &false,
            &true,
            &0u64,
        );

        // Move time to make tokens claimable
        env.ledger().set_timestamp(now + 1001);

        // Set beneficiary as caller
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&beneficiary);
        });

        // Test: Claims should fail when paused
        let result = std::panic::catch_unwind(|| {
            client.claim_tokens(&vault_id, &100i128);
        });
        assert!(result.is_err());

        // Test: Delegate claims should also fail when paused
        let delegate = Address::generate(&env);
        client.set_delegate(&vault_id, &Some(delegate.clone()));

        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&delegate);
        });

        let result = std::panic::catch_unwind(|| {
            client.claim_as_delegate(&vault_id, &100i128);
        });
        assert!(result.is_err());

        // Test: Admin can unpause the contract
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&admin);
        });

        client.toggle_pause();
        assert_eq!(client.is_paused(), false); // Should be unpaused

        // Test: Claims should work after unpausing
        env.as_contract(&contract_id, || {
            env.current_contract_address().set(&beneficiary);
        });

        let claimed = client.claim_tokens(&vault_id, &100i128);
        assert_eq!(claimed, 100i128); // Should succeed
    }
}
}
}

