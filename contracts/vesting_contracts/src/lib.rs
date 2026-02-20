#![no_std]
use soroban_sdk::{
    contract, contractimpl, vec, Env, String, Vec, Map, Symbol, Address, 
    token, IntoVal, TryFromVal, try_from_val, ConversionError
};

#[contract]
pub struct VestingContract;

// Storage keys for efficient access
const VAULT_COUNT: Symbol = Symbol::new(&"VAULT_COUNT");
const VAULT_DATA: Symbol = Symbol::new(&"VAULT_DATA");
const USER_VAULTS: Symbol = Symbol::new(&"USER_VAULTS");
const INITIAL_SUPPLY: Symbol = Symbol::new(&"INITIAL_SUPPLY");
const ADMIN_BALANCE: Symbol = Symbol::new(&"ADMIN_BALANCE");
const ADMIN_ADDRESS: Symbol = Symbol::new(&"ADMIN_ADDRESS");
const PROPOSED_ADMIN: Symbol = Symbol::new(&"PROPOSED_ADMIN");

// Vault structure with lazy initialization
#[contracttype]
pub struct Vault {
    pub owner: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub is_initialized: bool, // Lazy initialization flag
}

#[contracttype]
pub struct BatchCreateData {
    pub recipients: Vec<Address>,
    pub amounts: Vec<i128>,
    pub start_times: Vec<u64>,
    pub end_times: Vec<u64>,
}

#[contractimpl]
impl VestingContract {
    // Initialize contract with initial supply
    pub fn initialize(env: Env, admin: Address, initial_supply: i128) {
        // Set initial supply
        env.storage().instance().set(&INITIAL_SUPPLY, &initial_supply);
        
        // Set admin balance (initially all tokens go to admin)
        env.storage().instance().set(&ADMIN_BALANCE, &initial_supply);
        
        // Set admin address
        env.storage().instance().set(&ADMIN_ADDRESS, &admin);
        
        // Initialize vault count
        env.storage().instance().set(&VAULT_COUNT, &0u64);
    }
    
    // Helper function to check if caller is admin
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&ADMIN_ADDRESS)
            .unwrap_or_else(|| panic!("Admin not set"));
        let caller = env.current_contract_address();
        require!(caller == admin, "Caller is not admin");
    }
    
    // Propose a new admin (first step of two-step process)
    pub fn propose_new_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        
        // Store the proposed admin
        env.storage().instance().set(&PROPOSED_ADMIN, &new_admin);
    }
    
    // Accept admin ownership (second step of two-step process)
    pub fn accept_ownership(env: Env) {
        let proposed_admin: Address = env.storage().instance().get(&PROPOSED_ADMIN)
            .unwrap_or_else(|| panic!("No proposed admin found"));
        
        let caller = env.current_contract_address();
        require!(caller == proposed_admin, "Caller is not the proposed admin");
        
        // Transfer admin rights
        env.storage().instance().set(&ADMIN_ADDRESS, &proposed_admin);
        
        // Clear the proposed admin
        env.storage().instance().remove(&PROPOSED_ADMIN);
    }
    
    // Get current admin address
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN_ADDRESS)
            .unwrap_or_else(|| panic!("Admin not set"))
    }
    
    // Get proposed admin address (if any)
    pub fn get_proposed_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&PROPOSED_ADMIN)
    }
    
    // Full initialization - writes all metadata immediately
    pub fn create_vault_full(env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64) -> u64 {
        Self::require_admin(&env);
        
        // Get next vault ID
        let mut vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        vault_count += 1;
        
        // Check admin balance and transfer tokens
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= amount, "Insufficient admin balance");
        admin_balance -= amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        // Create vault with full initialization
        let vault = Vault {
            owner: owner.clone(),
            total_amount: amount,
            released_amount: 0,
            start_time,
            end_time,
            is_initialized: true, // Mark as fully initialized
        };
        
        // Store vault data immediately (expensive gas usage)
        env.storage().instance().set(&VAULT_DATA, &vault_count, &vault);
        
        // Update user vaults list
        let mut user_vaults: Vec<u64> = env.storage().instance()
            .get(&USER_VAULTS, &owner)
            .unwrap_or(Vec::new(&env));
        user_vaults.push_back(vault_count);
        env.storage().instance().set(&USER_VAULTS, &owner, &user_vaults);
        
        // Update vault count
        env.storage().instance().set(&VAULT_COUNT, &vault_count);
        
        vault_count
    }
    
    // Lazy initialization - writes minimal data initially
    pub fn create_vault_lazy(env: Env, owner: Address, amount: i128, start_time: u64, end_time: u64) -> u64 {
        Self::require_admin(&env);
        
        // Get next vault ID
        let mut vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        vault_count += 1;
        
        // Check admin balance and transfer tokens
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= amount, "Insufficient admin balance");
        admin_balance -= amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        // Create vault with lazy initialization (minimal storage)
        let vault = Vault {
            owner: owner.clone(),
            total_amount: amount,
            released_amount: 0,
            start_time,
            end_time,
            is_initialized: false, // Mark as lazy initialized
        };
        
        // Store only essential data initially (cheaper gas)
        env.storage().instance().set(&VAULT_DATA, &vault_count, &vault);
        
        // Update vault count
        env.storage().instance().set(&VAULT_COUNT, &vault_count);
        
        // Don't update user vaults list yet (lazy)
        
        vault_count
    }
    
    // Initialize vault metadata when needed (on-demand)
    pub fn initialize_vault_metadata(env: Env, vault_id: u64) -> bool {
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                // Return empty vault if not found
                Vault {
                    owner: Address::from_contract_id(&env.current_contract_address()),
                    total_amount: 0,
                    released_amount: 0,
                    start_time: 0,
                    end_time: 0,
                    is_initialized: false,
                }
            });
        
        // Only initialize if not already initialized
        if !vault.is_initialized {
            let mut updated_vault = vault.clone();
            updated_vault.is_initialized = true;
            
            // Store updated vault with full metadata
            env.storage().instance().set(&VAULT_DATA, &vault_id, &updated_vault);
            
            // Update user vaults list (deferred)
            let mut user_vaults: Vec<u64> = env.storage().instance()
                .get(&USER_VAULTS, &updated_vault.owner)
                .unwrap_or(Vec::new(&env));
            user_vaults.push_back(vault_id);
            env.storage().instance().set(&USER_VAULTS, &updated_vault.owner, &user_vaults);
            
            true
        } else {
            false // Already initialized
        }
    }
    
    // Claim tokens from vault
    pub fn claim_tokens(env: Env, vault_id: u64, claim_amount: i128) -> i128 {
        let mut vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                panic!("Vault not found");
            });
        
        require!(vault.is_initialized, "Vault not initialized");
        require!(claim_amount > 0, "Claim amount must be positive");
        
        let available_to_claim = vault.total_amount - vault.released_amount;
        require!(claim_amount <= available_to_claim, "Insufficient tokens to claim");
        
        // Update vault
        vault.released_amount += claim_amount;
        env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
        
        claim_amount
    }
    
    // Batch create vaults with lazy initialization
    pub fn batch_create_vaults_lazy(env: Env, batch_data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        
        let mut vault_ids = Vec::new(&env);
        let initial_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        
        // Check total admin balance
        let total_amount: i128 = batch_data.amounts.iter().sum();
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= total_amount, "Insufficient admin balance for batch");
        admin_balance -= total_amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        for i in 0..batch_data.recipients.len() {
            let vault_id = initial_count + i as u64 + 1;
            
            // Create vault with lazy initialization
            let vault = Vault {
                owner: batch_data.recipients.get(i).unwrap(),
                total_amount: batch_data.amounts.get(i).unwrap(),
                released_amount: 0,
                start_time: batch_data.start_times.get(i).unwrap(),
                end_time: batch_data.end_times.get(i).unwrap(),
                is_initialized: false, // Lazy initialization
            };
            
            // Store vault data (minimal writes)
            env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
            vault_ids.push_back(vault_id);
        }
        
        // Update vault count once (cheaper than individual updates)
        let final_count = initial_count + batch_data.recipients.len() as u64;
        env.storage().instance().set(&VAULT_COUNT, &final_count);
        
        vault_ids
    }
    
    // Batch create vaults with full initialization
    pub fn batch_create_vaults_full(env: Env, batch_data: BatchCreateData) -> Vec<u64> {
        Self::require_admin(&env);
        
        let mut vault_ids = Vec::new(&env);
        let initial_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        
        // Check total admin balance
        let total_amount: i128 = batch_data.amounts.iter().sum();
        let mut admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        require!(admin_balance >= total_amount, "Insufficient admin balance for batch");
        admin_balance -= total_amount;
        env.storage().instance().set(&ADMIN_BALANCE, &admin_balance);
        
        for i in 0..batch_data.recipients.len() {
            let vault_id = initial_count + i as u64 + 1;
            
            // Create vault with full initialization
            let vault = Vault {
                owner: batch_data.recipients.get(i).unwrap(),
                total_amount: batch_data.amounts.get(i).unwrap(),
                released_amount: 0,
                start_time: batch_data.start_times.get(i).unwrap(),
                end_time: batch_data.end_times.get(i).unwrap(),
                is_initialized: true, // Full initialization
            };
            
            // Store vault data (expensive writes)
            env.storage().instance().set(&VAULT_DATA, &vault_id, &vault);
            
            // Update user vaults list for each vault (expensive)
            let mut user_vaults: Vec<u64> = env.storage().instance()
                .get(&USER_VAULTS, &vault.owner)
                .unwrap_or(Vec::new(&env));
            user_vaults.push_back(vault_id);
            env.storage().instance().set(&USER_VAULTS, &vault.owner, &user_vaults);
            
            vault_ids.push_back(vault_id);
        }
        
        // Update vault count once
        let final_count = initial_count + batch_data.recipients.len() as u64;
        env.storage().instance().set(&VAULT_COUNT, &final_count);
        
        vault_ids
    }
    
    // Get vault info (initializes if needed)
    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        let vault: Vault = env.storage().instance()
            .get(&VAULT_DATA, &vault_id)
            .unwrap_or_else(|| {
                Vault {
                    owner: Address::from_contract_id(&env.current_contract_address()),
                    total_amount: 0,
                    released_amount: 0,
                    start_time: 0,
                    end_time: 0,
                    is_initialized: false,
                }
            });
        
        // Auto-initialize if lazy
        if !vault.is_initialized {
            Self::initialize_vault_metadata(env, vault_id);
            // Get updated vault
            env.storage().instance().get(&VAULT_DATA, &vault_id).unwrap()
        } else {
            vault
        }
    }
    
    // Get user vaults (initializes all if needed)
    pub fn get_user_vaults(env: Env, user: Address) -> Vec<u64> {
        let vault_ids: Vec<u64> = env.storage().instance()
            .get(&USER_VAULTS, &user)
            .unwrap_or(Vec::new(&env));
        
        // Initialize all lazy vaults for this user
        for vault_id in vault_ids.iter() {
            let vault: Vault = env.storage().instance()
                .get(&VAULT_DATA, vault_id)
                .unwrap_or_else(|| {
                    Vault {
                        owner: user.clone(),
                        total_amount: 0,
                        released_amount: 0,
                        start_time: 0,
                        end_time: 0,
                        is_initialized: false,
                    }
                });
            
            if !vault.is_initialized {
                Self::initialize_vault_metadata(env, *vault_id);
            }
        }
        
        vault_ids
    }
    
    // Get contract state for invariant checking
    pub fn get_contract_state(env: Env) -> (i128, i128, i128) {
        let initial_supply: i128 = env.storage().instance().get(&INITIAL_SUPPLY).unwrap_or(0);
        let admin_balance: i128 = env.storage().instance().get(&ADMIN_BALANCE).unwrap_or(0);
        
        // Calculate total locked and claimed amounts
        let vault_count: u64 = env.storage().instance().get(&VAULT_COUNT).unwrap_or(0);
        let mut total_locked = 0i128;
        let mut total_claimed = 0i128;
        
        for i in 1..=vault_count {
            if let Some(vault) = env.storage().instance().get::<_, Vault>(&VAULT_DATA, &i) {
                total_locked += vault.total_amount - vault.released_amount;
                total_claimed += vault.released_amount;
            }
        }
        
        (total_locked, total_claimed, admin_balance)
    }
    
    // Check invariant: Total Locked + Total Claimed + Admin Balance = Initial Supply
    pub fn check_invariant(env: Env) -> bool {
        let initial_supply: i128 = env.storage().instance().get(&INITIAL_SUPPLY).unwrap_or(0);
        let (total_locked, total_claimed, admin_balance) = Self::get_contract_state(env);
        
        let sum = total_locked + total_claimed + admin_balance;
        sum == initial_supply
    }
}
