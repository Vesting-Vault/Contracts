#![cfg(test)]

use super::*;
use soroban_sdk::{Address, Env, U256};

#[test]
fn test_basic_grant_functionality() {
    let env = Env::default();
    let contract_id = env.register(GrantContract, ());
    let client = GrantContractClient::new(&env, &contract_id);

    let recipient = Address::generate(&env);
    let total_amount = U256::from_u64(1000000);
    let duration = 86400; // 1 day

    client.initialize_grant(&recipient, &total_amount, &duration);
    
    let claimable = client.claimable_balance();
    assert_eq!(claimable, U256::from_u64(0));
    
    env.ledger().set_timestamp(env.ledger().timestamp() + 43200); // 12 hours later
    
    let claimable = client.claimable_balance();
    assert!(claimable > U256::from_u64(0));
}
