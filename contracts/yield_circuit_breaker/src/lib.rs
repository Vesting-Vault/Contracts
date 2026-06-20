#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, Symbol, String, Vec, IntoVal};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldProtocolConfig {
    pub protocol: Address,
    pub asset: Address,
    pub apy_bps: u32,
    pub last_checked_at: u64,
    pub gas_cost_harvest: i128,
    pub min_apy_threshold_bps: u32,
    pub circuit_breaker_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitBreakerConfig {
    pub min_apy_threshold_bps: u32,
    pub max_gas_cost: i128,
    pub cooldown_seconds: u64,
    pub auto_withdraw: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BreakerDataKey {
    Admin,
    YieldAdapter,
    VaultContract,
    InsuranceTreasury,
    ProtocolConfig(Address, Address),
    GlobalConfig,
    BreakerHistory(Address, Address),
    IsPaused,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ProtocolRiskUpdated {
    pub protocol: Address,
    pub asset: Address,
    pub apy_bps: u32,
    pub gas_cost: i128,
    pub checked_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CircuitBreakerTripped {
    pub protocol: Address,
    pub asset: Address,
    pub apy_bps: u32,
    pub threshold_bps: u32,
    pub gas_cost: i128,
    pub reason: String,
    pub tripped_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct EmergencyWithdrawalTriggered {
    pub protocol: Address,
    pub asset: Address,
    pub vault_id: u64,
    pub amount: i128,
    pub triggered_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CircuitBreakerReset {
    pub protocol: Address,
    pub asset: Address,
    pub reset_by: Address,
    pub reset_at: u64,
}

#[contract]
pub struct YieldCircuitBreaker;

#[contractimpl]
impl YieldCircuitBreaker {
    pub fn initialize(env: Env, admin: Address, yield_adapter: Address, vault_contract: Address, insurance_treasury: Address) {
        if env.storage().instance().has(&BreakerDataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&BreakerDataKey::Admin, &admin);
        env.storage().instance().set(&BreakerDataKey::YieldAdapter, &yield_adapter);
        env.storage().instance().set(&BreakerDataKey::VaultContract, &vault_contract);
        env.storage().instance().set(&BreakerDataKey::InsuranceTreasury, &insurance_treasury);
        env.storage().instance().set(&BreakerDataKey::IsPaused, &false);

        let default_config = CircuitBreakerConfig {
            min_apy_threshold_bps: 50,
            max_gas_cost: 100_000_000i128,
            cooldown_seconds: 86400,
            auto_withdraw: true,
        };
        env.storage().instance().set(&BreakerDataKey::GlobalConfig, &default_config);
    }

    pub fn register_protocol(env: Env, admin: Address, protocol: Address, asset: Address, min_apy_bps: u32) {
        Self::require_admin(&env, &admin);

        let config = YieldProtocolConfig {
            protocol: protocol.clone(),
            asset: asset.clone(),
            apy_bps: 0,
            last_checked_at: 0,
            gas_cost_harvest: 0,
            min_apy_threshold_bps: min_apy_bps,
            circuit_breaker_active: false,
        };
        env.storage().instance().set(&BreakerDataKey::ProtocolConfig(protocol, asset), &config);
    }

    pub fn update_protocol_apy(env: Env, admin: Address, protocol: Address, asset: Address, apy_bps: u32, gas_cost: i128) {
        Self::require_admin(&env, &admin);

        let mut config: YieldProtocolConfig = Self::get_protocol_config(env.clone(), protocol.clone(), asset.clone());
        let now = env.ledger().timestamp();

        config.apy_bps = apy_bps;
        config.last_checked_at = now;
        config.gas_cost_harvest = gas_cost;

        let global: CircuitBreakerConfig = env.storage().instance()
            .get(&BreakerDataKey::GlobalConfig)
            .expect("Global config not set");

        let threshold = if config.min_apy_threshold_bps > 0 { config.min_apy_threshold_bps } else { global.min_apy_threshold_bps };

        if apy_bps < threshold || gas_cost > global.max_gas_cost {
            config.circuit_breaker_active = true;
            env.storage().instance().set(&BreakerDataKey::ProtocolConfig(protocol.clone(), asset.clone()), &config);

            let mut reason = String::from_str(&env, "APY below threshold");
            if gas_cost > global.max_gas_cost {
                reason = String::from_str(&env, "Gas cost exceeds maximum");
            }
            if apy_bps < threshold && gas_cost > global.max_gas_cost {
                reason = String::from_str(&env, "APY below threshold and gas cost exceeds maximum");
            }

            CircuitBreakerTripped {
                protocol: protocol.clone(),
                asset: asset.clone(),
                apy_bps,
                threshold_bps: threshold,
                gas_cost,
                reason: reason.clone(),
                tripped_at: now,
            }.publish(&env);

            if global.auto_withdraw {
                Self::trigger_emergency_withdraw(&env, &protocol, &asset, now);
            }
        } else {
            env.storage().instance().set(&BreakerDataKey::ProtocolConfig(protocol.clone(), asset.clone()), &config);
        }

        ProtocolRiskUpdated {
            protocol: protocol.clone(),
            asset: asset.clone(),
            apy_bps,
            gas_cost,
            checked_at: now,
        }.publish(&env);
    }

    pub fn trigger_emergency_withdraw_all(env: Env, admin: Address, protocol: Address, asset: Address) {
        Self::require_admin(&env, &admin);

        let now = env.ledger().timestamp();
        Self::trigger_emergency_withdraw(&env, &protocol, &asset, now);
    }

    pub fn check_circuit_breaker(env: Env, protocol: Address, asset: Address) -> bool {
        let config = Self::get_protocol_config(env, protocol, asset);
        config.circuit_breaker_active
    }

    pub fn reset_circuit_breaker(env: Env, admin: Address, protocol: Address, asset: Address) {
        Self::require_admin(&env, &admin);

        let mut config = Self::get_protocol_config(env.clone(), protocol.clone(), asset.clone());
        config.circuit_breaker_active = false;
        env.storage().instance().set(&BreakerDataKey::ProtocolConfig(protocol.clone(), asset.clone()), &config);

        CircuitBreakerReset {
            protocol: protocol.clone(),
            asset: asset.clone(),
            reset_by: admin.clone(),
            reset_at: env.ledger().timestamp(),
        }.publish(&env);
    }

    pub fn update_global_config(env: Env, admin: Address, min_apy_threshold_bps: u32, max_gas_cost: i128, cooldown_seconds: u64, auto_withdraw: bool) {
        Self::require_admin(&env, &admin);

        let config = CircuitBreakerConfig {
            min_apy_threshold_bps,
            max_gas_cost,
            cooldown_seconds,
            auto_withdraw,
        };
        env.storage().instance().set(&BreakerDataKey::GlobalConfig, &config);
    }

    pub fn get_global_config(env: Env) -> CircuitBreakerConfig {
        env.storage().instance()
            .get(&BreakerDataKey::GlobalConfig)
            .expect("Global config not set")
    }

    pub fn get_protocol_config(env: Env, protocol: Address, asset: Address) -> YieldProtocolConfig {
        env.storage().instance()
            .get(&BreakerDataKey::ProtocolConfig(protocol, asset))
            .expect("Protocol not registered")
    }

    pub fn set_pause(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&BreakerDataKey::IsPaused, &paused);
    }

    fn trigger_emergency_withdraw(env: &Env, protocol: &Address, asset: &Address, now: u64) {
        let yield_adapter: Address = env.storage().instance()
            .get(&BreakerDataKey::YieldAdapter)
            .expect("Yield adapter not set");
        let vault_contract: Address = env.storage().instance()
            .get(&BreakerDataKey::VaultContract)
            .expect("Vault contract not set");

        let args = Vec::from_array(env, [vault_contract.into_val(env), asset.clone().into_val(env)]);
        env.invoke_contract::<()>(&yield_adapter, &Symbol::new(env, "emergency_withdraw_from_yield"), args);

        let mut config: YieldProtocolConfig = env.storage().instance()
            .get(&BreakerDataKey::ProtocolConfig(protocol.clone(), asset.clone()))
            .expect("Protocol not registered");
        config.circuit_breaker_active = true;
        env.storage().instance().set(&BreakerDataKey::ProtocolConfig(protocol.clone(), asset.clone()), &config);

        EmergencyWithdrawalTriggered {
            protocol: protocol.clone(),
            asset: asset.clone(),
            vault_id: 0,
            amount: 0,
            triggered_at: now,
        }.publish(env);
    }

    fn require_admin(env: &Env, admin: &Address) {
        let stored: Address = env.storage().instance().get(&BreakerDataKey::Admin).expect("Admin not set");
        if stored != *admin {
            admin.require_auth();
        }
    }
}
