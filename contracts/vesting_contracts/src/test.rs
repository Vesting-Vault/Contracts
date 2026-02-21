#![cfg(test)]

use soroban_sdk::{testutils::*, vec, Address, Env, Vec};
use crate::{VestingContract, Vault, BatchCreateData};

#[test]
fn measure_create_vault_full() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    VestingContract::initialize(env.clone(), admin.clone(), 10_000_000_000);  // 100 XLM example

    env.budget().reset();

    let owner = Address::generate(&env);
    let start = env.ledger().timestamp();
    let end = start + 365 * 24 * 3600;

    let vault_id = VestingContract::create_vault_full(
        env.clone(),
        owner,
        1_000_000_000,  // 10 XLM
        start,
        end,
    );

    let cpu = env.budget().get_cpu_insns_count();
    let mem = env.budget().get_mem_bytes_usage();
    let events = env.events().len();  // optional

    println!("create_vault_full: CPU insns ~{}, Mem bytes ~{}, Vault ID: {}", cpu, mem, vault_id);
}

#[test]
fn measure_create_vault_lazy() {
    // Similar setup as above, but call create_vault_lazy
    // ... (copy and adapt)
}

#[test]
fn measure_claim_tokens() {
    // Setup: create vault first, then claim
    // ... (mock time advance with env.ledger().set_timestamp(...))
    // Print budget after claim
}

// Add similar for batch_* (use small batch like 3–5 recipients)