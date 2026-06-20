#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, Symbol, Vec, IntoVal};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FutureUnlockInfo {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub total_unvested: i128,
    pub vested_unclaimed: i128,
    pub guaranteed_future_unlocks: i128,
    pub current_time: u64,
    pub end_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CollateralizedPosition {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub lending_protocol: Address,
    pub loan_amount: i128,
    pub collateral_value: i128,
    pub liquidation_threshold_bps: u32,
    pub created_at: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AdapterDataKey {
    Admin,
    VaultContract,
    CollateralBridge,
    LiquidationThresholdBps,
    MaxLtvBps,
    IsPaused,
    Position(u64),
    PositionCounter,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UnlockScheduleVerified {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub total_unvested: i128,
    pub guaranteed_future_unlocks: i128,
    pub verified_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CollateralPositionOpened {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub lending_protocol: Address,
    pub loan_amount: i128,
    pub collateral_value: i128,
    pub created_at: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CollateralPositionClosed {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub closed_at: u64,
}

pub const MAX_LTV_BPS: u32 = 6000;
pub const LIQUIDATION_THRESHOLD_BPS: u32 = 8000;
pub const BASIS_POINTS_DENOM: u32 = 10000;

#[contract]
pub struct CollateralizedBorrowingAdapter;

#[contractimpl]
impl CollateralizedBorrowingAdapter {
    pub fn initialize(env: Env, admin: Address, vault_contract: Address, collateral_bridge: Address) {
        if env.storage().instance().has(&AdapterDataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&AdapterDataKey::Admin, &admin);
        env.storage().instance().set(&AdapterDataKey::VaultContract, &vault_contract);
        env.storage().instance().set(&AdapterDataKey::CollateralBridge, &collateral_bridge);
        env.storage().instance().set(&AdapterDataKey::LiquidationThresholdBps, &LIQUIDATION_THRESHOLD_BPS);
        env.storage().instance().set(&AdapterDataKey::MaxLtvBps, &MAX_LTV_BPS);
        env.storage().instance().set(&AdapterDataKey::PositionCounter, &0u64);
        env.storage().instance().set(&AdapterDataKey::IsPaused, &false);
    }

    pub fn verify_future_unlocks(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
        total_amount: i128,
        released_amount: i128,
        start_time: u64,
        end_time: u64,
    ) -> FutureUnlockInfo {
        let now = env.ledger().timestamp();

        let total_unvested = if total_amount > released_amount {
            total_amount - released_amount
        } else {
            0
        };

        let vesting_duration = if end_time > start_time { end_time - start_time } else { 1 };

        let vested_unclaimed = if now > start_time {
            let elapsed = if now > end_time { vesting_duration } else { now - start_time };
            let vested = (total_amount * elapsed as i128) / vesting_duration as i128;
            let claimed = released_amount;
            if vested > claimed { vested - claimed } else { 0 }
        } else {
            0
        };

        let guaranteed_future_unlocks = if total_unvested > 0 { total_unvested } else { 0 };

        let info = FutureUnlockInfo {
            vault_id,
            beneficiary: beneficiary.clone(),
            total_unvested,
            vested_unclaimed,
            guaranteed_future_unlocks,
            current_time: now,
            end_time,
        };

        UnlockScheduleVerified {
            vault_id,
            beneficiary: beneficiary.clone(),
            total_unvested,
            guaranteed_future_unlocks,
            verified_at: now,
        }.publish(&env);

        info
    }

    pub fn verify_collateral_for_loan(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
        total_amount: i128,
        released_amount: i128,
        start_time: u64,
        end_time: u64,
        desired_loan_amount: i128,
        stablecoin_price: i128,
    ) -> bool {
        let unlock_info = Self::verify_future_unlocks(
            env.clone(),
            vault_id,
            beneficiary.clone(),
            total_amount,
            released_amount,
            start_time,
            end_time,
        );

        if unlock_info.guaranteed_future_unlocks <= 0 {
            panic!("No future unlocks available");
        }

        let max_ltv_bps: u32 = env.storage().instance().get(&AdapterDataKey::MaxLtvBps).unwrap_or(MAX_LTV_BPS);
        let max_loan = (unlock_info.guaranteed_future_unlocks * max_ltv_bps as i128) / BASIS_POINTS_DENOM as i128;

        let max_loan_in_stablecoin = if stablecoin_price > 0 {
            (max_loan * stablecoin_price) / 10_000_000i128
        } else {
            max_loan
        };

        max_loan_in_stablecoin >= desired_loan_amount
    }

    pub fn calculate_max_borrowable(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
        total_amount: i128,
        released_amount: i128,
        start_time: u64,
        end_time: u64,
        stablecoin_price: i128,
    ) -> i128 {
        let unlock_info = Self::verify_future_unlocks(
            env.clone(),
            vault_id,
            beneficiary,
            total_amount,
            released_amount,
            start_time,
            end_time,
        );
        let max_ltv_bps: u32 = env.storage().instance().get(&AdapterDataKey::MaxLtvBps).unwrap_or(MAX_LTV_BPS);
        let max_loan = (unlock_info.guaranteed_future_unlocks * max_ltv_bps as i128) / BASIS_POINTS_DENOM as i128;

        if stablecoin_price > 0 {
            (max_loan * stablecoin_price) / 10_000_000i128
        } else {
            max_loan
        }
    }

    pub fn open_collateralized_position(
        env: Env,
        admin: Address,
        vault_id: u64,
        beneficiary: Address,
        lending_protocol: Address,
        loan_amount: i128,
        collateral_value: i128,
    ) -> u64 {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        if loan_amount <= 0 || collateral_value <= 0 {
            panic!("Amounts must be positive");
        }

        let max_ltv_bps: u32 = env.storage().instance().get(&AdapterDataKey::MaxLtvBps).unwrap_or(MAX_LTV_BPS);
        let max_loan = (collateral_value * max_ltv_bps as i128) / BASIS_POINTS_DENOM as i128;
        if loan_amount > max_loan {
            panic!("Loan exceeds max LTV");
        }

        let position_id = Self::increment_position_counter(&env);
        let position = CollateralizedPosition {
            vault_id,
            beneficiary: beneficiary.clone(),
            lending_protocol: lending_protocol.clone(),
            loan_amount,
            collateral_value,
            liquidation_threshold_bps: Self::get_liquidation_threshold(&env),
            created_at: env.ledger().timestamp(),
            is_active: true,
        };

        env.storage().instance().set(&AdapterDataKey::Position(position_id), &position);

        let collateral_bridge: Address = env.storage().instance()
            .get(&AdapterDataKey::CollateralBridge)
            .expect("Collateral bridge not set");

        let bridge_args = Vec::from_array(&env, [
            vault_id.into_val(&env),
            lending_protocol.clone().into_val(&env),
            collateral_value.into_val(&env),
            loan_amount.into_val(&env),
            500u32.into_val(&env),
            (env.ledger().timestamp() + 365 * 86400).into_val(&env),
        ]);
        let _lien_id: u64 = env.invoke_contract(
            &collateral_bridge,
            &Symbol::new(&env, "create_lien"),
            bridge_args,
        );

        CollateralPositionOpened {
            vault_id,
            beneficiary: beneficiary.clone(),
            lending_protocol: lending_protocol.clone(),
            loan_amount,
            collateral_value,
            created_at: env.ledger().timestamp(),
        }.publish(&env);

        position_id
    }

    pub fn close_collateralized_position(env: Env, admin: Address, position_id: u64) {
        Self::require_admin(&env, &admin);
        Self::require_not_paused(&env);

        let mut position: CollateralizedPosition = env.storage().instance()
            .get(&AdapterDataKey::Position(position_id))
            .expect("Position not found");

        if !position.is_active {
            panic!("Position already closed");
        }

        position.is_active = false;
        env.storage().instance().set(&AdapterDataKey::Position(position_id), &position);

        CollateralPositionClosed {
            vault_id: position.vault_id,
            beneficiary: position.beneficiary,
            closed_at: env.ledger().timestamp(),
        }.publish(&env);
    }

    pub fn get_position(env: Env, position_id: u64) -> CollateralizedPosition {
        env.storage().instance()
            .get(&AdapterDataKey::Position(position_id))
            .expect("Position not found")
    }

    pub fn get_vault_schedule(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
        total_amount: i128,
        released_amount: i128,
        start_time: u64,
        end_time: u64,
    ) -> FutureUnlockInfo {
        Self::verify_future_unlocks(
            env,
            vault_id,
            beneficiary,
            total_amount,
            released_amount,
            start_time,
            end_time,
        )
    }

    pub fn set_liquidation_threshold(env: Env, admin: Address, threshold_bps: u32) {
        Self::require_admin(&env, &admin);
        if threshold_bps > BASIS_POINTS_DENOM {
            panic!("Threshold exceeds 100%");
        }
        env.storage().instance().set(&AdapterDataKey::LiquidationThresholdBps, &threshold_bps);
    }

    pub fn get_liquidation_threshold(env: &Env) -> u32 {
        env.storage().instance().get(&AdapterDataKey::LiquidationThresholdBps).unwrap_or(LIQUIDATION_THRESHOLD_BPS)
    }

    pub fn set_max_ltv(env: Env, admin: Address, max_ltv_bps: u32) {
        Self::require_admin(&env, &admin);
        if max_ltv_bps > BASIS_POINTS_DENOM {
            panic!("Max LTV exceeds 100%");
        }
        env.storage().instance().set(&AdapterDataKey::MaxLtvBps, &max_ltv_bps);
    }

    pub fn get_max_ltv(env: Env) -> u32 {
        env.storage().instance().get(&AdapterDataKey::MaxLtvBps).unwrap_or(MAX_LTV_BPS)
    }

    pub fn set_pause(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&AdapterDataKey::IsPaused, &paused);
    }

    fn increment_position_counter(env: &Env) -> u64 {
        let count: u64 = env.storage().instance().get(&AdapterDataKey::PositionCounter).unwrap_or(0);
        let new_count = count + 1;
        env.storage().instance().set(&AdapterDataKey::PositionCounter, &new_count);
        new_count
    }

    fn require_admin(env: &Env, admin: &Address) {
        let stored: Address = env.storage().instance().get(&AdapterDataKey::Admin).expect("Admin not set");
        if stored != *admin {
            admin.require_auth();
        }
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&AdapterDataKey::IsPaused).unwrap_or(false) {
            panic!("Contract is paused");
        }
    }
}
