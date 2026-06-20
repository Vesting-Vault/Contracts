#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, Vec, token};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AMMPoolPosition {
    pub pool_address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub base_token: Address,
    pub deposited_base: i128,
    pub deposited_other: i128,
    pub pool_shares: i128,
    pub deposited_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CollateralBufferConfig {
    pub buffer_bps: u32,
    pub dao_admin: Address,
    pub max_pool_allocation_bps: u32,
    pub rebalance_threshold_bps: u32,
    pub min_buffer_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VaultBufferState {
    pub vault_id: u64,
    pub total_promised_base: i128,
    pub total_collateral_value: i128,
    pub buffer_amount: i128,
    pub last_rebalance: u64,
    pub positions: Vec<AMMPoolPosition>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BufferDataKey {
    Admin,
    VaultContract,
    BufferConfig,
    VaultState(u64),
    WhitelistedPool(Address),
    PoolCounter,
    IsPaused,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct BufferConfigured {
    pub buffer_bps: u32,
    pub dao_admin: Address,
    pub configured_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CollateralDeposited {
    pub vault_id: u64,
    pub pool_address: Address,
    pub base_amount: i128,
    pub pool_shares: i128,
    pub deposited_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct BufferRebalanced {
    pub vault_id: u64,
    pub total_promised: i128,
    pub total_collateral: i128,
    pub buffer_amount: i128,
    pub buffer_pct: u32,
    pub rebalanced_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct InsufficientBuffer {
    pub vault_id: u64,
    pub total_promised: i128,
    pub total_collateral: i128,
    pub shortfall: i128,
    pub detected_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct PositionWithdrawn {
    pub vault_id: u64,
    pub pool_address: Address,
    pub base_withdrawn: i128,
    pub other_withdrawn: i128,
    pub withdrawn_at: u64,
}

pub const DEFAULT_BUFFER_BPS: u32 = 500;
pub const MAX_BUFFER_BPS: u32 = 2000;
pub const BASIS_POINTS_DENOM: u32 = 10000;

#[contract]
pub struct CollateralBuffer;

#[contractimpl]
impl CollateralBuffer {
    pub fn initialize(env: Env, admin: Address, vault_contract: Address, dao_admin: Address) {
        if env.storage().instance().has(&BufferDataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&BufferDataKey::Admin, &admin);
        env.storage().instance().set(&BufferDataKey::VaultContract, &vault_contract);
        env.storage().instance().set(&BufferDataKey::PoolCounter, &0u64);
        env.storage().instance().set(&BufferDataKey::IsPaused, &false);

        let config = CollateralBufferConfig {
            buffer_bps: DEFAULT_BUFFER_BPS,
            dao_admin,
            max_pool_allocation_bps: 3000,
            rebalance_threshold_bps: 100,
            min_buffer_seconds: 86400,
        };
        env.storage().instance().set(&BufferDataKey::BufferConfig, &config);

        let stored_admin: Address = env.storage().instance().get(&BufferDataKey::Admin).unwrap();
        BufferConfigured {
            buffer_bps: DEFAULT_BUFFER_BPS,
            dao_admin: stored_admin,
            configured_at: env.ledger().timestamp(),
        }.publish(&env);
    }

    pub fn whitelist_pool(env: Env, admin: Address, pool_address: Address) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&BufferDataKey::WhitelistedPool(pool_address), &true);

        let mut counter: u64 = env.storage().instance().get(&BufferDataKey::PoolCounter).unwrap_or(0);
        counter += 1;
        env.storage().instance().set(&BufferDataKey::PoolCounter, &counter);
    }

    pub fn is_pool_whitelisted(env: Env, pool_address: Address) -> bool {
        env.storage().instance().get(&BufferDataKey::WhitelistedPool(pool_address)).unwrap_or(false)
    }

    pub fn deposit_to_amm_pool(
        env: Env,
        admin: Address,
        vault_id: u64,
        pool_address: Address,
        base_token: Address,
        other_token: Address,
        base_amount: i128,
        other_amount: i128,
    ) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        if !Self::is_pool_whitelisted(env.clone(), pool_address.clone()) {
            panic!("Pool not whitelisted");
        }
        if base_amount <= 0 || other_amount <= 0 {
            panic!("Amounts must be positive");
        }

        let mut vault_state: VaultBufferState = env.storage().instance()
            .get(&BufferDataKey::VaultState(vault_id))
            .unwrap_or(VaultBufferState {
                vault_id,
                total_promised_base: 0,
                total_collateral_value: 0,
                buffer_amount: 0,
                last_rebalance: 0,
                positions: Vec::new(&env),
            });

        let pool_shares = Self::calculate_pool_shares(base_amount, other_amount);

        let position = AMMPoolPosition {
            pool_address: pool_address.clone(),
            token_a: base_token.clone(),
            token_b: other_token.clone(),
            base_token: base_token.clone(),
            deposited_base: base_amount,
            deposited_other: other_amount,
            pool_shares,
            deposited_at: env.ledger().timestamp(),
        };

        vault_state.total_promised_base += base_amount;
        vault_state.total_collateral_value += base_amount + other_amount;
        vault_state.buffer_amount += other_amount;
        vault_state.positions.push_back(position);

        env.storage().instance().set(&BufferDataKey::VaultState(vault_id), &vault_state);

        let vault_contract: Address = env.storage().instance().get(&BufferDataKey::VaultContract).expect("Vault contract not set");
        let base_token_client = token::Client::new(&env, &base_token);
        base_token_client.transfer(&vault_contract, &env.current_contract_address(), &base_amount);

        let other_token_client = token::Client::new(&env, &other_token);
        other_token_client.transfer(&vault_contract, &env.current_contract_address(), &other_amount);

        CollateralDeposited {
            vault_id,
            pool_address: pool_address.clone(),
            base_amount,
            pool_shares,
            deposited_at: env.ledger().timestamp(),
        }.publish(&env);

        Self::check_buffer_health(&env, vault_id);
    }

    pub fn withdraw_from_pool(
        env: Env,
        admin: Address,
        vault_id: u64,
        position_index: u32,
    ) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        let mut vault_state: VaultBufferState = env.storage().instance()
            .get(&BufferDataKey::VaultState(vault_id))
            .expect("Vault state not found");

        if position_index >= vault_state.positions.len() {
            panic!("Position not found");
        }

        let position = vault_state.positions.get(position_index).unwrap();

        let (base_withdrawn, other_withdrawn) = Self::withdraw_from_amm(&position);

        vault_state.total_collateral_value -= base_withdrawn + other_withdrawn;
        vault_state.buffer_amount -= other_withdrawn;
        vault_state.total_promised_base -= base_withdrawn;
        vault_state.positions.remove(position_index);

        env.storage().instance().set(&BufferDataKey::VaultState(vault_id), &vault_state);

        let vault_contract: Address = env.storage().instance().get(&BufferDataKey::VaultContract).expect("Vault contract not set");
        let base_token_client = token::Client::new(&env, &position.base_token);
        base_token_client.transfer(&env.current_contract_address(), &vault_contract, &base_withdrawn);

        PositionWithdrawn {
            vault_id,
            pool_address: position.pool_address,
            base_withdrawn,
            other_withdrawn,
            withdrawn_at: env.ledger().timestamp(),
        }.publish(&env);
    }

    pub fn rebalance_buffer(env: Env, admin: Address, vault_id: u64) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        let config: CollateralBufferConfig = env.storage().instance()
            .get(&BufferDataKey::BufferConfig)
            .expect("Buffer config not set");
        let mut vault_state: VaultBufferState = env.storage().instance()
            .get(&BufferDataKey::VaultState(vault_id))
            .expect("Vault state not found");

        let now = env.ledger().timestamp();
        if now < vault_state.last_rebalance + config.min_buffer_seconds {
            panic!("Rebalance cooldown not elapsed");
        }

        let total_collateral = vault_state.total_collateral_value;

        if total_collateral < vault_state.total_promised_base {
            InsufficientBuffer {
                vault_id,
                total_promised: vault_state.total_promised_base,
                total_collateral,
                shortfall: vault_state.total_promised_base - total_collateral,
                detected_at: now,
            }.publish(&env);
            panic!("Collateral value less than promised base");
        }

        vault_state.last_rebalance = now;
        env.storage().instance().set(&BufferDataKey::VaultState(vault_id), &vault_state);

        let buffer_pct = if total_collateral > 0 {
            ((vault_state.buffer_amount * BASIS_POINTS_DENOM as i128) / total_collateral) as u32
        } else {
            0
        };

        BufferRebalanced {
            vault_id,
            total_promised: vault_state.total_promised_base,
            total_collateral,
            buffer_amount: vault_state.buffer_amount,
            buffer_pct,
            rebalanced_at: now,
        }.publish(&env);
    }

    pub fn check_buffer_health(env: &Env, vault_id: u64) -> bool {
        let config: CollateralBufferConfig = env.storage().instance()
            .get(&BufferDataKey::BufferConfig)
            .expect("Buffer config not set");
        if let Some(vault_state) = env.storage().instance().get::<_, VaultBufferState>(&BufferDataKey::VaultState(vault_id)) {
            let required_buffer = (vault_state.total_promised_base * config.buffer_bps as i128) / BASIS_POINTS_DENOM as i128;
            let total_required = vault_state.total_promised_base + required_buffer;

            if vault_state.total_collateral_value < total_required {
                InsufficientBuffer {
                    vault_id,
                    total_promised: vault_state.total_promised_base,
                    total_collateral: vault_state.total_collateral_value,
                    shortfall: total_required - vault_state.total_collateral_value,
                    detected_at: env.ledger().timestamp(),
                }.publish(env);
                return false;
            }
            true
        } else {
            true
        }
    }

    pub fn get_vault_buffer_state(env: Env, vault_id: u64) -> VaultBufferState {
        env.storage().instance()
            .get(&BufferDataKey::VaultState(vault_id))
            .unwrap_or(VaultBufferState {
                vault_id,
                total_promised_base: 0,
                total_collateral_value: 0,
                buffer_amount: 0,
                last_rebalance: 0,
                positions: Vec::new(&env),
            })
    }

    pub fn guaranteed_withdrawable(env: Env, vault_id: u64) -> i128 {
        let vault_state = Self::get_vault_buffer_state(env, vault_id);
        let required_buffer = (vault_state.total_promised_base * DEFAULT_BUFFER_BPS as i128) / BASIS_POINTS_DENOM as i128;
        if vault_state.total_collateral_value >= vault_state.total_promised_base + required_buffer {
            vault_state.total_promised_base
        } else if vault_state.total_collateral_value >= vault_state.total_promised_base {
            vault_state.total_promised_base - (vault_state.total_collateral_value - vault_state.total_promised_base)
        } else {
            vault_state.total_collateral_value
        }
    }

    pub fn set_buffer_bps(env: Env, admin: Address, buffer_bps: u32) {
        Self::require_admin(&env, &admin);
        if buffer_bps > MAX_BUFFER_BPS {
            panic!("Buffer exceeds maximum");
        }
        let mut config: CollateralBufferConfig = env.storage().instance()
            .get(&BufferDataKey::BufferConfig)
            .expect("Buffer config not set");
        config.buffer_bps = buffer_bps;
        env.storage().instance().set(&BufferDataKey::BufferConfig, &config);
    }

    pub fn set_dao_admin(env: Env, admin: Address, new_dao_admin: Address) {
        Self::require_admin(&env, &admin);
        let mut config: CollateralBufferConfig = env.storage().instance()
            .get(&BufferDataKey::BufferConfig)
            .expect("Buffer config not set");
        config.dao_admin = new_dao_admin;
        env.storage().instance().set(&BufferDataKey::BufferConfig, &config);
    }

    pub fn get_config(env: Env) -> CollateralBufferConfig {
        env.storage().instance().get(&BufferDataKey::BufferConfig).expect("Buffer config not set")
    }

    pub fn set_pause(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&BufferDataKey::IsPaused, &paused);
    }

    fn calculate_pool_shares(base_amount: i128, other_amount: i128) -> i128 {
        base_amount + other_amount
    }

    fn withdraw_from_amm(position: &AMMPoolPosition) -> (i128, i128) {
        let total = position.pool_shares;
        let il_loss = (total * 5i128) / 1000i128;
        let base_withdrawn = position.deposited_base - il_loss;
        let other_withdrawn = position.deposited_other;
        (base_withdrawn, other_withdrawn)
    }

    fn require_admin(env: &Env, admin: &Address) {
        let stored: Address = env.storage().instance().get(&BufferDataKey::Admin).expect("Admin not set");
        if stored != *admin {
            admin.require_auth();
        }
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&BufferDataKey::IsPaused).unwrap_or(false) {
            panic!("Contract is paused");
        }
    }
}
