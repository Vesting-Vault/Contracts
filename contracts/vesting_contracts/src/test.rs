 #[cfg(test)]
mod tests {
        use crate::{
        BatchCreateData, Milestone, VestingContract, VestingContractClient,
    };
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Ledger},
        token, vec, Address, Env, IntoVal, Symbol,
    };

    // -------------------------------------------------------------------------
    // Helper: fresh contract + yield-bearing token + tokens actually in contract
    // -------------------------------------------------------------------------

    fn setup() -> (Env, Address, VestingContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VestingContract, ());
        let client = VestingContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &1_000_000i128);

        let token_addr = register_token(&env, &admin);
        client.set_token(&token_addr);
        client.add_to_whitelist(&token_addr);

        // Mint initial supply to contract
        let stellar = token::StellarAssetClient::new(&env, &token_addr);
        stellar.mint(&contract_id, &1_000_000i128);

        (env, contract_id, client, admin, token_addr)
    }

    fn register_token(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone())
            .address()
    }

    fn mint_to(env: &Env, token_addr: &Address, recipient: &Address, amount: i128) {
        token::StellarAssetClient::new(env, token_addr).mint(recipient, &amount);
    }

    // -------------------------------------------------------------------------
    // Original tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_admin_ownership_transfer() {
        let (env, _cid, client, admin, _token) = setup();
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
    // Migration / deprecation (Issue #43)
    // -------------------------------------------------------------------------

    #[test]
    fn test_migrate_liquidity_freezes_and_transfers_whitelisted_balances() {
        let (env, contract_id, client, admin) = setup();

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

        assert_eq!(migrated.get(token_addr.clone()).unwrap_or(0), 1_000i128);
        assert_eq!(token_client.balance(&contract_id), 0);
        assert_eq!(token_client.balance(&v2), 1_000i128);
    }

    #[test]
    fn test_migrate_liquidity_blocks_admin_actions_afterwards() {
        let (env, contract_id, client, admin) = setup();

        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);
        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        let v2 = Address::generate(&env);
        client.migrate_liquidity(&v2);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let result = std::panic::catch_unwind(|| {
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
        });
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // Vault creation
    // -------------------------------------------------------------------------

    #[test]
    fn test_create_vault_full_increments_count() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let id1 = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);
        let id2 = client.create_vault_full(&beneficiary, &500i128, &(now + 10), &(now + 2_000), &0i128, &true, &false, &0u64);
        assert_eq!(id1, 1u64);
        assert_eq!(id2, 2u64);
    }

    #[test]
    fn test_create_vault_lazy_increments_count() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let id = client.create_vault_lazy(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);
        assert_eq!(id, 1u64);
    }

    #[test]
    fn test_batch_create_vaults_lazy() {
        let (env, _cid, client, _admin, _token) = setup();
        let r1 = Address::generate(&env);
        let r2 = Address::generate(&env);

        let batch = BatchCreateData {
            recipients: vec![&env, r1.clone(), r2.clone()],
            amounts: vec![&env, 1_000i128, 2_000i128],
            start_times: vec![&env, 100u64, 150u64],
            end_times: vec![&env, 200u64, 250u64],
            keeper_fees: vec![&env, 0i128, 0i128],
            step_durations: vec![&env, 0u64, 0u64],
        };

        let ids = client.batch_create_vaults_lazy(&batch);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_batch_create_vaults_full() {
        let (env, _cid, client, _admin, _token) = setup();
        let r1 = Address::generate(&env);
        let r2 = Address::generate(&env);

        let batch = BatchCreateData {
            recipients: vec![&env, r1.clone(), r2.clone()],
            amounts: vec![&env, 1_000i128, 2_000i128],
            start_times: vec![&env, 100u64, 150u64],
            end_times: vec![&env, 200u64, 250u64],
            keeper_fees: vec![&env, 0i128, 0i128],
            step_durations: vec![&env, 0u64, 0u64],
        };

        let ids = client.batch_create_vaults_full(&batch);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_step_vesting_full_claim_at_end() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let start = 1_000u64;
        let end = start + 101u64;
        let step = 17u64;
        let total = 1_009i128;

        let vault_id = client.create_vault_full(&beneficiary, &total, &start, &end, &0i128, &true, &true, &step);

        env.ledger().with_mut(|l| l.timestamp = end + 1);
        let claimed = client.claim_tokens(&vault_id, &total);
        assert_eq!(claimed, total);

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, total);
    }

    #[test]
    fn test_lockup_only_claim_succeeds_at_end() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();
        let duration = 1_000u64;
        let total = 100_000i128;

        let vault_id = client.create_vault_full(&beneficiary, &total, &now, &(now + duration), &0i128, &true, &false, &duration);

        env.ledger().with_mut(|l| l.timestamp = now + duration);
        let claimed = client.claim_tokens(&vault_id, &total);
        assert_eq!(claimed, total);
    }

    #[test]
    #[should_panic]
    fn test_lockup_only_claim_fails_before_end() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();
        let duration = 1_000u64;

        let vault_id = client.create_vault_full(&beneficiary, &100_000i128, &now, &(now + duration), &0i128, &true, &false, &duration);

        env.ledger().with_mut(|l| l.timestamp = now + duration - 1);
        client.claim_tokens(&vault_id, &1i128);
    }
    
    #[test]
    fn test_admin_access_control() {
        let (env, contract_id, client, admin) = setup();
        let new_admin = Address::generate(&env);
    
        // Test: Admin can propose new admin
        client.propose_new_admin(&new_admin);
        assert_eq!(client.get_proposed_admin(), Some(new_admin.clone()));
    
        // Test: Proposed admin can accept ownership
        new_admin.require_auth();
        client.accept_ownership();
    
        // Verify admin transfer completed
        assert_eq!(client.get_admin(), new_admin);
        assert_eq!(client.get_proposed_admin(), None);
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
    assert_eq!(claimable, expected_monthly, "Should have exactly one month of tokens after 30 days");
    
    // Test 4: After 45 days - still only one step (rounds down)
    env.ledger().set_timestamp(start_time + (45 * 24 * 60 * 60));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, expected_monthly, "Should still have only one month of tokens after 45 days");
    
    // Test 5: After 60 days - two steps completed
    env.ledger().set_timestamp(start_time + (2 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 2 * expected_monthly, "Should have two months of tokens after 60 days");
    
    // Test 6: After 6 months - 6 steps completed
    env.ledger().set_timestamp(start_time + (6 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 6 * expected_monthly, "Should have six months of tokens after 6 months");
    
    // Test 7: After end time - all tokens vested
    env.ledger().set_timestamp(end_time + 1000);
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, amount, "Should have all tokens vested after end time");
}

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
    assert_eq!(claimable, 3 * expected_weekly, "Should have three weeks of tokens after 3 weeks");
    
    // Test: After 10 weeks - 10 steps completed
    env.ledger().set_timestamp(start_time + (10 * step_duration));
    let claimable = client.get_claimable_amount(&vault_id);
    assert_eq!(claimable, 10 * expected_weekly, "Should have ten weeks of tokens after 10 weeks");
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
    env.ledger().set_timestamp(start_time + (182 * 24 * 60 * 60)); // ~6 months
    let claimable = client.get_claimable_amount(&vault_id);
    let expected_half = amount / 2; // 50% of tokens
    assert_eq!(claimable, expected_half, "Should have 50% of tokens after half the time for linear vesting");
    
    // Test: After 3 months (quarter of the duration) - should have 25% vested
    env.ledger().set_timestamp(start_time + (91 * 24 * 60 * 60)); // ~3 months
    let claimable = client.get_claimable_amount(&vault_id);
    let expected_quarter = amount / 4; // 25% of tokens
    assert_eq!(claimable, expected_quarter, "Should have 25% of tokens after quarter of the time for linear vesting");
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
    assert_eq!(remaining_claimable, 15000i128, "Should have 15,000 tokens remaining claimable");
    
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
        env.events().publish((Symbol::new(&env, "stake"), vault_id), amount);
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
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        assert!(!client.is_vault_irrevocable(&vault_id));
        client.mark_irrevocable(&vault_id);
        assert!(client.is_vault_irrevocable(&vault_id));
    }

    #[test]
    #[should_panic]
    fn test_revoke_irrevocable_vault_panics() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        client.mark_irrevocable(&vault_id);
        client.revoke_tokens(&vault_id);
    }

    #[test]
    fn test_clawback_within_grace_period() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &5_000i128, &(now + 100), &(now + 10_000), &0i128, &true, &false, &0u64);

        env.ledger().with_mut(|l| l.timestamp = now + 3_599);
        let returned = client.clawback_vault(&vault_id);
        assert_eq!(returned, 5_000i128);
    }

    #[test]
    #[should_panic]
    fn test_clawback_after_grace_period_panics() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &5_000i128, &(now + 100), &(now + 10_000), &0i128, &true, &false, &0u64);

        env.ledger().with_mut(|l| l.timestamp = now + 3_601);
        client.clawback_vault(&vault_id);
    }

    #[test]
    fn test_milestone_unlock_and_claim() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        let milestones = vec![&env, Milestone { id: 1, percentage: 50, is_unlocked: false }, Milestone { id: 2, percentage: 50, is_unlocked: false }];
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
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        let milestones = vec![&env, Milestone { id: 1, percentage: 100, is_unlocked: false }];
        client.set_milestones(&vault_id, &milestones);
        client.claim_tokens(&vault_id, &1i128);
    }

    #[test]
    fn test_rotate_beneficiary_key() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let new_beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &1_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        client.rotate_beneficiary_key(&vault_id, &new_beneficiary);

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.owner, new_beneficiary);
    }

    #[test]
    fn test_invariant_holds_after_operations() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &10_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);
        assert!(client.check_invariant());

        env.ledger().with_mut(|l| l.timestamp = now + 500);
        client.claim_tokens(&vault_id, &5_000i128);
        assert!(client.check_invariant());

        client.revoke_tokens(&vault_id);
        assert!(client.check_invariant());
    }

    // =========================================================================
    // rescue tests
    // =========================================================================

    #[test]
    fn test_rescue_basic_no_vaults() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 5_000i128);
    }

    #[test]
    fn test_rescue_only_surplus_above_vault_liability() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.create_vault_full(&beneficiary, &3_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 2_000i128);
    }

    #[test]
    fn test_rescue_after_partial_claim_adjusts_liability() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &4_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        client.claim_tokens(&vault_id, &1_000i128);

        mint_to(&env, &token_addr, &contract_id, 5_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 2_000i128);
    }

    #[test]
    fn test_rescue_multiple_vaults_correct_liability_sum() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let now = env.ledger().timestamp();

        for _ in 0..3 {
            let b = Address::generate(&env);
            client.create_vault_full(&b, &2_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);
        }

        mint_to(&env, &token_addr, &contract_id, 7_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 1_000i128);
    }

    #[test]
    fn test_rescue_after_full_claim_all_tokens_rescuable() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &2_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        client.claim_tokens(&vault_id, &2_000i128);

        mint_to(&env, &token_addr, &contract_id, 500i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 500i128);
    }

    #[test]
    fn test_rescue_after_revoke_liability_drops_to_zero() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(&beneficiary, &3_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        client.revoke_tokens(&vault_id);

        mint_to(&env, &token_addr, &contract_id, 3_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 3_000i128);
    }

    #[test]
    fn test_rescue_tokens_go_to_current_admin_after_transfer() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let new_admin = Address::generate(&env);
        client.propose_new_admin(&new_admin);
        client.accept_ownership();

        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        let rescued = client.rescue_unallocated_tokens(&token_addr);
        assert_eq!(rescued, 1_000i128);
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_when_no_surplus() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        client.add_to_whitelist(&token_addr);

        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        client.create_vault_full(&beneficiary, &3_000i128, &now, &(now + 1_000), &0i128, &true, &false, &0u64);

        mint_to(&env, &token_addr, &contract_id, 3_000i128);

        client.rescue_unallocated_tokens(&token_addr);
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_when_contract_balance_zero() {
        let (env, _cid, client, _admin, _token) = setup();
        let token_addr = register_token(&env, &_admin);
        client.add_to_whitelist(&token_addr);

        client.rescue_unallocated_tokens(&token_addr);
    }

    #[test]
    #[should_panic]
    fn test_rescue_panics_for_non_whitelisted_token() {
        let (env, contract_id, client, admin, _main_token) = setup();
        let token_addr = register_token(&env, &admin);
        mint_to(&env, &token_addr, &contract_id, 1_000i128);

        client.rescue_unallocated_tokens(&token_addr);
    }
}
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
            &beneficiary, &10_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
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
            &beneficiary, &10_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
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
            &Address::generate(&env), &10_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );
        let v2 = client.create_vault_full(
            &Address::generate(&env), &20_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
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
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary, &5_000i128, &now, &now,
            &0i128, &true, &false, &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 5_000i128, "zero-duration vault should unlock 100% immediately");
    }

    #[test]
    fn test_zero_duration_vault_claim_full() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary, &10_000i128, &now, &now,
            &0i128, &true, &false, &0u64,
        );

        let claimed = client.claim_tokens(&vault_id, &10_000i128);
        assert_eq!(claimed, 10_000i128, "should claim full amount from zero-duration vault");

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, 10_000i128);
    }

    #[test]
    fn test_zero_duration_vault_before_start() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let future = env.ledger().timestamp() + 1_000;

        let vault_id = client.create_vault_full(
            &beneficiary, &5_000i128, &future, &future,
            &0i128, &true, &false, &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0, "zero-duration vault should not unlock before start_time");
    }

    #[test]
    fn test_zero_cliff_vault_vests_immediately() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary, &10_000i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 500);
        let claimable = client.get_claimable_amount(&vault_id);
        assert!(claimable > 0, "zero-cliff vault should vest from start_time");
    }

    #[test]
    fn test_zero_amount_vault_no_claimable() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary, &0i128, &now, &(now + 1_000),
            &0i128, &true, &false, &0u64,
        );

        env.ledger().with_mut(|l| l.timestamp = now + 1_001);
        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0, "zero-amount vault should have nothing claimable");
    }

    #[test]
    fn test_zero_duration_zero_amount_vault() {
        let (env, _cid, client, _admin) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_full(
            &beneficiary, &0i128, &now, &now,
            &0i128, &true, &false, &0u64,
        );

        let claimable = client.get_claimable_amount(&vault_id);
        assert_eq!(claimable, 0, "zero-duration + zero-amount vault should have nothing claimable");
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

    // =========================================================================
    // claim_and_call tests (beneficiary claim hooks)
    // =========================================================================

    /// Mock receiver contract that simulates a DEX or downstream handler.
    /// Tokens are transferred to this contract *before* the callback fires,
    /// so it can inspect its own balance and act accordingly.
    #[contract]
    pub struct MockReceiverContract;

    #[contractimpl]
    impl MockReceiverContract {
        /// Simulates a swap callback invoked by `claim_and_call`.
        pub fn swap(env: Env, beneficiary: Address, min_amount_out: i128) {
            env.events().publish(
                Symbol::new(&env, "SwapExecuted"),
                (beneficiary, min_amount_out),
            );
        }
    }

    #[test]
    fn test_claim_and_call_transfers_to_target_and_invokes() {
        let (env, contract_id, client, _admin, token_addr) = setup();
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

        // Register mock receiver (simulates a DEX)
        let receiver_id = env.register(MockReceiverContract, ());

        // Advance time past end so all tokens vest
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        let transferred = client.claim_and_call(
            &vault_id,
            &10_000i128,
            &receiver_id,
            &function,
            &args,
        );

        // Tokens went to the receiver, not the beneficiary
        let token_client = token::Client::new(&env, &token_addr);
        assert_eq!(token_client.balance(&beneficiary), 0);
        assert!(token_client.balance(&receiver_id) >= transferred);

        // Vault accounting updated
        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, 10_000i128);
    }

    #[test]
    fn test_claim_and_call_partial_claim() {
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

        let receiver_id = env.register(MockReceiverContract, ());
        env.ledger().with_mut(|l| l.timestamp = now + 500);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        let transferred = client.claim_and_call(
            &vault_id,
            &5_000i128,
            &receiver_id,
            &function,
            &args,
        );
        assert_eq!(transferred, 5_000i128);

        let vault = client.get_vault(&vault_id);
        assert_eq!(vault.released_amount, 5_000i128);
    }

    #[test]
    #[should_panic(expected = "Vault is frozen")]
    fn test_claim_and_call_frozen_vault_panics() {
        let (env, _cid, client, _admin, _token) = setup();
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
        );
        client.freeze_vault(&vault_id);

        let receiver_id = env.register(MockReceiverContract, ());
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        client.claim_and_call(
            &vault_id,
            &1_000i128,
            &receiver_id,
            &function,
            &args,
        );
    }

    #[test]
    #[should_panic(expected = "Vault not initialized")]
    fn test_claim_and_call_uninitialized_vault_panics() {
        let (env, _cid, client, _admin, _token) = setup();
        let beneficiary = Address::generate(&env);
        let now = env.ledger().timestamp();

        let vault_id = client.create_vault_lazy(
            &beneficiary,
            &1_000i128,
            &now,
            &(now + 1_000),
            &0i128,
            &true,
            &false,
            &0u64,
        );

        let receiver_id = env.register(MockReceiverContract, ());
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        client.claim_and_call(
            &vault_id,
            &1_000i128,
            &receiver_id,
            &function,
            &args,
        );
    }

    #[test]
    #[should_panic(expected = "Claim amount must be positive")]
    fn test_claim_and_call_zero_amount_panics() {
        let (env, _cid, client, _admin, _token) = setup();
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
        );

        let receiver_id = env.register(MockReceiverContract, ());
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        client.claim_and_call(
            &vault_id,
            &0i128,
            &receiver_id,
            &function,
            &args,
        );
    }

    #[test]
    fn test_claim_and_call_with_yield_distribution() {
        let (env, contract_id, client, _admin, token_addr) = setup();
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

        // Simulate yield accrual by minting extra tokens to the contract
        let stellar = token::StellarAssetClient::new(&env, &token_addr);
        stellar.mint(&contract_id, &2_000i128);

        let receiver_id = env.register(MockReceiverContract, ());
        env.ledger().with_mut(|l| l.timestamp = now + 1_001);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        let transferred = client.claim_and_call(
            &vault_id,
            &10_000i128,
            &receiver_id,
            &function,
            &args,
        );

        // Should include yield: 10k principal + 2k yield = 12k
        assert_eq!(transferred, 12_000i128);

        // Tokens landed on the receiver, not the beneficiary
        let token_client = token::Client::new(&env, &token_addr);
        assert_eq!(token_client.balance(&receiver_id), 12_000i128);
        assert_eq!(token_client.balance(&beneficiary), 0);
    }

    #[test]
    #[should_panic(expected = "Insufficient unlocked tokens to claim")]
    fn test_claim_and_call_exceeding_vested_panics() {
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

        let receiver_id = env.register(MockReceiverContract, ());
        // Only half the time has passed — 5k vested
        env.ledger().with_mut(|l| l.timestamp = now + 500);

        let function = Symbol::new(&env, "swap");
        let args = vec![
            &env,
            beneficiary.clone().into_val(&env),
            0i128.into_val(&env),
        ];

        // Try to claim 8k but only 5k available
        client.claim_and_call(
            &vault_id,
            &8_000i128,
            &receiver_id,
            &function,
            &args,
        );
    }

}
