#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, Vec, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TwapObservation {
    pub price: i128,
    pub timestamp: u64,
    pub cumulative_price: i128,
    pub observation_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PriceSnapshot {
    pub price: i128,
    pub recorded_at: u64,
    pub ledger_seq: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldState {
    pub total_shares: i128,
    pub total_underlying: i128,
    pub last_commit_ledger: u32,
    pub last_commit_time: u64,
    pub pending_shares: i128,
    pub pending_underlying: i128,
    pub min_hold_ledgers: u32,
    pub min_hold_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DefDataKey {
    Admin,
    VaultContract,
    YieldAdapter,
    TwapObservations(Symbol),
    PriceSnapshot(Symbol),
    YieldCommit(Symbol),
    MinHoldLedgers,
    MinHoldSeconds,
    TwapWindowSeconds,
    IsPaused,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ObservationSubmitted {
    pub asset: Symbol,
    pub price: i128,
    pub timestamp: u64,
    pub cumulative_price: i128,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct YieldStateCommitted {
    pub asset: Symbol,
    pub total_shares: i128,
    pub total_underlying: i128,
    pub commit_ledger: u32,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ManipulationDetected {
    pub asset: Symbol,
    pub spot_price: i128,
    pub twap_price: i128,
    pub deviation_bps: u32,
}

#[contract]
pub struct FlashLoanDefense;

#[contractimpl]
impl FlashLoanDefense {
    pub fn initialize(env: Env, admin: Address, vault_contract: Address, yield_adapter: Address) {
        if env.storage().instance().has(&DefDataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DefDataKey::Admin, &admin);
        env.storage().instance().set(&DefDataKey::VaultContract, &vault_contract);
        env.storage().instance().set(&DefDataKey::YieldAdapter, &yield_adapter);
        env.storage().instance().set(&DefDataKey::TwapWindowSeconds, &3600u64);
        env.storage().instance().set(&DefDataKey::MinHoldLedgers, &5u32);
        env.storage().instance().set(&DefDataKey::MinHoldSeconds, &30u64);
        env.storage().instance().set(&DefDataKey::IsPaused, &false);
    }

    pub fn submit_observation(env: Env, admin: Address, asset: Symbol, price: i128) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        if price <= 0 {
            panic!("Price must be positive");
        }

        let now = env.ledger().timestamp();
        let mut observations: Vec<TwapObservation> = env.storage().instance()
            .get(&DefDataKey::TwapObservations(asset.clone()))
            .unwrap_or(Vec::new(&env));

        let last_obs = observations.last();
        let cumulative_price = if let Some(ref prev) = last_obs {
            let time_elapsed = now - prev.timestamp;
            prev.cumulative_price + prev.price * time_elapsed as i128
        } else {
            0i128
        };

        let obs_count = if let Some(ref prev) = last_obs { prev.observation_count + 1 } else { 1 };

        let obs = TwapObservation {
            price,
            timestamp: now,
            cumulative_price,
            observation_count: obs_count,
        };

        observations.push_back(obs);

        let window = Self::get_twap_window(&env);
        let cutoff = now.saturating_sub(window);
        let mut pruned = Vec::new(&env);
        for o in observations.iter() {
            if o.timestamp >= cutoff || pruned.len() == 0 {
                pruned.push_back(o);
            }
        }
        env.storage().instance().set(&DefDataKey::TwapObservations(asset.clone()), &pruned);

        ObservationSubmitted {
            asset: asset.clone(),
            price,
            timestamp: now,
            cumulative_price,
        }.publish(&env);
    }

    pub fn get_twap_price(env: Env, asset: Symbol) -> i128 {
        let observations: Vec<TwapObservation> = env.storage().instance()
            .get(&DefDataKey::TwapObservations(asset))
            .unwrap_or(Vec::new(&env));

        if observations.len() < 2 {
            return 0;
        }

        let first = observations.first().unwrap();
        let last = observations.last().unwrap();
        let time_elapsed = last.timestamp - first.timestamp;

        if time_elapsed == 0 {
            return 0;
        }

        let price_delta = last.cumulative_price - first.cumulative_price;
        if price_delta <= 0 {
            return 0;
        }

        price_delta / time_elapsed as i128
    }

    pub fn get_spot_price(env: Env, asset: Symbol) -> i128 {
        env.storage().instance()
            .get(&DefDataKey::PriceSnapshot(asset))
            .map(|s: PriceSnapshot| s.price)
            .unwrap_or(0)
    }

    pub fn check_price_manipulation(env: Env, asset: Symbol, spot_price: i128, max_deviation_bps: u32) -> bool {
        let twap = Self::get_twap_price(env.clone(), asset.clone());
        if twap == 0 {
            return false;
        }
        if spot_price == 0 {
            return false;
        }

        let diff = if spot_price > twap { spot_price - twap } else { twap - spot_price };
        let deviation_bps = (diff * 10000) / twap;

        if deviation_bps > max_deviation_bps as i128 {
            ManipulationDetected {
                asset: asset.clone(),
                spot_price,
                twap_price: twap,
                deviation_bps: deviation_bps as u32,
            }.publish(&env);
            return true;
        }
        false
    }

    pub fn commit_yield_state(env: Env, admin: Address, asset: Symbol, total_shares: i128, total_underlying: i128) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        let current_ledger = env.ledger().sequence();
        let current_time = env.ledger().timestamp();

        let mut state: YieldState = env.storage().instance()
            .get(&DefDataKey::YieldCommit(asset.clone()))
            .unwrap_or(YieldState {
                total_shares: 0,
                total_underlying: 0,
                last_commit_ledger: 0,
                last_commit_time: 0,
                pending_shares: 0,
                pending_underlying: 0,
                min_hold_ledgers: Self::get_min_hold_ledgers(&env),
                min_hold_seconds: Self::get_min_hold_seconds(&env),
            });

        if total_shares <= 0 || total_underlying <= 0 {
            panic!("State values must be positive");
        }

        if state.last_commit_ledger > 0 {
            let ledger_diff = current_ledger - state.last_commit_ledger;
            let time_diff = current_time - state.last_commit_time;
            if ledger_diff < state.min_hold_ledgers || time_diff < state.min_hold_seconds {
                panic!("Commit cooldown not elapsed");
            }
        }

        let snapshot = PriceSnapshot {
            price: if total_shares > 0 { (total_underlying * 10_000_000) / total_shares } else { 10_000_000 },
            recorded_at: current_time,
            ledger_seq: current_ledger,
        };
        env.storage().instance().set(&DefDataKey::PriceSnapshot(asset.clone()), &snapshot);

        state.total_shares = total_shares;
        state.total_underlying = total_underlying;
        state.last_commit_ledger = current_ledger;
        state.last_commit_time = current_time;
        state.pending_shares = 0;
        state.pending_underlying = 0;
        env.storage().instance().set(&DefDataKey::YieldCommit(asset.clone()), &state);

        YieldStateCommitted {
            asset: asset.clone(),
            total_shares,
            total_underlying,
            commit_ledger: current_ledger,
        }.publish(&env);
    }

    pub fn get_yield_state(env: Env, asset: Symbol) -> YieldState {
        env.storage().instance()
            .get(&DefDataKey::YieldCommit(asset))
            .unwrap_or(YieldState {
                total_shares: 0,
                total_underlying: 0,
                last_commit_ledger: 0,
                last_commit_time: 0,
                pending_shares: 0,
                pending_underlying: 0,
                min_hold_ledgers: 0,
                min_hold_seconds: 0,
            })
    }

    pub fn set_min_hold_ledgers(env: Env, admin: Address, ledgers: u32) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DefDataKey::MinHoldLedgers, &ledgers);
    }

    pub fn get_min_hold_ledgers(env: &Env) -> u32 {
        env.storage().instance().get(&DefDataKey::MinHoldLedgers).unwrap_or(5u32)
    }

    pub fn set_min_hold_seconds(env: Env, admin: Address, seconds: u64) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DefDataKey::MinHoldSeconds, &seconds);
    }

    pub fn get_min_hold_seconds(env: &Env) -> u64 {
        env.storage().instance().get(&DefDataKey::MinHoldSeconds).unwrap_or(30u64)
    }

    pub fn set_twap_window(env: Env, admin: Address, window_seconds: u64) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DefDataKey::TwapWindowSeconds, &window_seconds);
    }

    pub fn get_twap_window(env: &Env) -> u64 {
        env.storage().instance().get(&DefDataKey::TwapWindowSeconds).unwrap_or(3600u64)
    }

    pub fn set_pause(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DefDataKey::IsPaused, &paused);
    }

    fn require_admin(env: &Env, admin: &Address) {
        let stored: Address = env.storage().instance().get(&DefDataKey::Admin).expect("Admin not set");
        if stored != *admin {
            admin.require_auth();
        }
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&DefDataKey::IsPaused).unwrap_or(false) {
            panic!("Contract is paused");
        }
    }
}
