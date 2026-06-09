#![no_std]

use soroban_sdk::{contract, contractimpl, Env, Address, Vec, Map, String, BytesN, IntoVal};

mod storage;
pub mod types;
mod audit_exporter;
mod emergency;
pub mod errors;

pub use types::*;
use errors::Error;
use storage::{get_claim_history, set_claim_history, get_authorized_payout_address as storage_get_authorized_payout_address, set_authorized_payout_address as storage_set_authorized_payout_address, get_pending_address_request as storage_get_pending_address_request, set_pending_address_request as storage_set_pending_address_request, remove_pending_address_request as storage_remove_pending_address_request, get_timelock_duration, get_auditors, set_auditors, get_auditor_pause_requests, set_auditor_pause_requests, get_emergency_pause, set_emergency_pause, remove_emergency_pause, get_reputation_bridge_contract, set_reputation_bridge_contract, has_reputation_bonus_applied, set_reputation_bonus_applied, get_milestone_configs, set_milestone_configs, get_milestone_status, set_milestone_status, get_emergency_pause_duration, is_nullifier_used, set_nullifier_used, get_commitment, set_commitment, mark_commitment_used, add_privacy_claim_event, add_merkle_root, get_merkle_roots, is_valid_merkle_root, get_path_payment_config, set_path_payment_config, get_path_payment_claim_history, add_path_payment_claim_event, get_lst_config, set_lst_config, get_unvested_balance, set_unvested_balance, get_admin_dead_man_switch, set_admin_dead_man_switch, get_oracle_price_record, set_oracle_price_record, get_contract_total_unvested, set_contract_total_unvested, get_protocol_sunset, set_protocol_sunset, get_migration_payload, set_migration_payload, get_relayer_migration, set_relayer_migration};
use emergency::{AuditorPauseRequest, EmergencyPause, EmergencyPauseTriggered};

#[contract]
pub struct VestingVault;

#[contractimpl]
impl VestingVault {

    /// Claim vested tokens for a beneficiary.
    ///
    /// Runs the full compliance gate (KYC, sanctions, AML, etc.) before recording
    /// the claim event.  The actual token transfer is left to the caller's vesting
    /// logic; this function only validates and records.
    ///
    /// # Parameters
    /// - `user`       – The beneficiary address; must sign the transaction.
    /// - `vesting_id` – Identifier of the vesting schedule being claimed against.
    /// - `amount`     – Number of tokens (in base units) to claim.
    ///
    /// # Errors
    /// Returns a typed [`Error`] variant for every compliance failure (e.g.
    /// `KycNotCompleted`, `AddressSanctioned`, `AmlThresholdExceeded`, …).
    pub fn claim(e: Env, user: Address, vesting_id: u32, amount: i128) -> Result<(), Error> {
        user.require_auth();

        // ========== COMPLIANCE CHECKS ==========

        // KYC Verification Check
        if !Self::is_kyc_verified(&e, &user) {
            return Err(Error::KycNotCompleted);
        }

        // KYC Expiration Check
        if let Some(kyc_expiry) = Self::get_kyc_expiry(&e, &user) {
            let current_time = e.ledger().timestamp();
            if current_time > kyc_expiry {
                return Err(Error::KycExpired);
            }
        }

        // Sanctions Check
        if Self::is_address_sanctioned(&e, &user) {
            return Err(Error::AddressSanctioned);
        }

        // Jurisdiction Restriction Check
        if Self::is_jurisdiction_restricted(&e, &user) {
            return Err(Error::JurisdictionRestricted);
        }

        // Legal Signature Verification
        if !Self::has_valid_legal_signature(&e, &user, vesting_id) {
            return Err(Error::LegalSignatureMissing);
        }

        // AML Threshold Check
        if amount > Self::get_aml_threshold(&e) {
            return Err(Error::AmlThresholdExceeded);
        }

        // Risk Rating Check
        if Self::get_user_risk_rating(&e, &user) > Self::get_max_allowed_risk(&e) {
            return Err(Error::RiskRatingTooHigh);
        }

        // Document Verification Check
        if !Self::are_documents_verified(&e, &user) {
            return Err(Error::DocumentVerificationFailed);
        }

        // Accreditation Status Check (for regulated claims)
        if Self::is_accreditation_required(&e, vesting_id) && !Self::is_user_accredited(&e, &user) {
            return Err(Error::AccreditationStatusInvalid);
        }

        // Tax Compliance Check
        if !Self::is_tax_compliant(&e, &user) {
            return Err(Error::TaxComplianceFailed);
        }

        // Regulatory Block Check
        if Self::is_regulatory_block_active(&e) {
            return Err(Error::RegulatoryBlockActive);
        }

        // Whitelist Approval Check
        if !Self::is_whitelist_approved(&e, &user) {
            return Err(Error::WhitelistNotApproved);
        }

        // Blacklist Violation Check
        if Self::is_on_blacklist(&e, &user) {
            return Err(Error::BlacklistViolation);
        }

        // Geofencing Restriction Check
        if Self::is_geofencing_restricted(&e, &user) {
            return Err(Error::GeofencingRestriction);
        }

        // Identity Verification Expiration Check
        if let Some(identity_expiry) = Self::get_identity_expiry(&e, &user) {
            let current_time = e.ledger().timestamp();
            if current_time > identity_expiry {
                return Err(Error::IdentityVerificationExpired);
            }
        }

        // Source of Funds Verification Check
        if !Self::is_source_of_funds_verified(&e, &user) {
            return Err(Error::SourceOfFundsNotVerified);
        }

        // Beneficial Owner Verification Check
        if !Self::is_beneficial_owner_verified(&e, &user) {
            return Err(Error::BeneficialOwnerNotVerified);
        }

        // Politically Exposed Person Check
        if Self::is_politically_exposed_person(&e, &user) {
            return Err(Error::PoliticallyExposedPerson);
        }

        // Sanctions List Hit Check
        if Self::is_on_sanctions_list(&e, &user) {
            return Err(Error::SanctionsListHit);
        }

        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active {
                let current_time = e.ledger().timestamp();
                if current_time < pause.expires_at {
                    return Err(Error::RegulatoryBlockActive);
                } else {
                    // Pause expired, remove it
                    remove_emergency_pause(&e);
                }
            }
        }

        // Check if user has an authorized payout address
        if let Some(auth_address) = storage_get_authorized_payout_address(&e, &user) {
            if auth_address.is_active {
                let current_time = e.ledger().timestamp();

                // Check if timelock has passed
                if current_time < auth_address.effective_at {
                    return Err(Error::WhitelistNotApproved);
                }

                // Verify the claim is being made to the authorized address
                // In a real implementation, this would check the destination of the transfer
                // For now, we'll assume the claim function includes a destination parameter
                // or that the user context provides this information
            }
        }

        // Check milestone vesting if applicable
        if let Some(_milestone_configs) = get_milestone_configs(&e, vesting_id) {
            let _milestone_status = get_milestone_status(&e, vesting_id);
            // Additional milestone logic would go here
        }

        // Check LST configuration
        if let Some(lst_config) = get_lst_config(&e, vesting_id) {
            if lst_config.enabled {
                let exchange_rate = Self::get_lst_exchange_rate(&e, &lst_config.lst_token_address);
                // Rebasing math: exchange rate has 7 decimal precision (e.g. 1 LST = 1 Base -> 10,000,000)
                // LST amount = (Base amount * exchange rate) / 10_000_000
                let lst_amount = (amount * exchange_rate) / 10_000_000i128;
                
                LSTClaimExecuted {
                    user: user.clone(),
                    vesting_id,
                    base_amount: amount,
                    lst_amount,
                    lst_token_address: lst_config.lst_token_address.clone(),
                    timestamp: e.ledger().timestamp(),
                }.publish(&e);
                
                // TODO: Execute actual LST token transfer here using lst_amount
            }
        }

        // TODO: your base token vesting logic here

        let mut history = get_claim_history(&e);

        let event = ClaimEvent {
            beneficiary: user.clone(),
            amount,
            timestamp: e.ledger().timestamp(),
            vesting_id,
        };

        history.push_back(event);

        set_claim_history(&e, &history);

        Ok(())
    }

    /// Sets an authorized payout address with a 48-hour timelock
    /// This provides multi-layer defense against phishing hacks
    pub fn set_authorized_payout_address(e: Env, beneficiary: Address, authorized_address: Address) {
        beneficiary.require_auth();
        
        let current_time = e.ledger().timestamp();
        let effective_at = current_time + get_timelock_duration();

        // Create the pending request
        let request = AddressWhitelistRequest {
            beneficiary: beneficiary.clone(),
            requested_address: authorized_address.clone(),
            requested_at: current_time,
            effective_at,
        };

        // Store the pending request
        storage_set_pending_address_request(&e, &beneficiary, &request);

        // Emit event
        AddressWhitelistRequested { beneficiary: beneficiary.clone(), requested_address: authorized_address, requested_at: current_time, effective_at }.publish(&e);
    }

    /// Confirms and activates a pending authorized payout address request
    /// Can only be called after the 48-hour timelock period has passed
    pub fn confirm_auth_payout_addr(e: Env, beneficiary: Address) -> Result<(), Error> {
        beneficiary.require_auth();
        
        let current_time = e.ledger().timestamp();
        
        // Get the pending request
        let pending_request = storage_get_pending_address_request(&e, &beneficiary)
            .ok_or(Error::InvalidInput)?;

        // Check if timelock has passed
        if current_time < pending_request.effective_at {
            return Err(Error::TimelockNotElapsed);
        }

        // Create the authorized address record
        let auth_address = AuthorizedPayoutAddress {
            beneficiary: beneficiary.clone(),
            authorized_address: pending_request.requested_address.clone(),
            requested_at: pending_request.requested_at,
            effective_at: pending_request.effective_at,
            is_active: true,
        };

        // Store the authorized address
        storage_set_authorized_payout_address(&e, &beneficiary, &auth_address);

        // Remove the pending request
        storage_remove_pending_address_request(&e, &beneficiary);

        // Emit confirmation event
        AuthorizedAddressSet { beneficiary: beneficiary.clone(), authorized_address: pending_request.requested_address, effective_at: pending_request.effective_at }.publish(&e);
    }

    /// Gets the current authorized payout address for a beneficiary
    pub fn get_authorized_payout_address(e: Env, beneficiary: Address) -> Option<AuthorizedPayoutAddress> {
        storage_get_authorized_payout_address(&e, &beneficiary)
    }

    /// Gets any pending address request for a beneficiary
    pub fn get_pending_address_request(e: Env, beneficiary: Address) -> Option<AddressWhitelistRequest> {
        storage_get_pending_address_request(&e, &beneficiary)
    }

    /// Removes the authorized payout address (immediate effect)
    /// This allows beneficiaries to disable the whitelisting feature
    pub fn remove_authorized_payout_address(e: Env, beneficiary: Address) {
        beneficiary.require_auth();
        
        // Remove the authorized address
        e.storage().instance().remove(&(storage::AUTHORIZED_PAYOUT_ADDRESS, beneficiary.clone()));
        
        // Also remove any pending request
        storage_remove_pending_address_request(&e, &beneficiary);
    }

    /// Return the full on-chain claim history (needed by the audit exporter).
    pub fn get_all_claims(e: Env) -> Vec<ClaimEvent> {
        get_claim_history(&e)
    }

    /// Return all claim events for a specific beneficiary.
    ///
    /// # Parameters
    /// - `user` – The beneficiary whose claim history is requested.
    pub fn get_claims_by_user(e: Env, user: Address) -> Vec<ClaimEvent> {
        audit_exporter::export_claims_by_user(&e, user)
    }

    // ========== ISSUE #140: Emergency Protocol Pause for Auditors ==========
    
    /// Initialize the auditor security team
    /// Initialise the 2-of-3 auditor multisig security team.
    ///
    /// # Parameters
    /// - `admin`    – Must sign; only the admin may set auditors.
    /// - `auditors` – Exactly 3 auditor addresses required.
    ///
    /// # Errors
    /// - `InvalidInput` if `auditors.len() != 3`.
    pub fn initialize_auditors(e: Env, admin: Address, auditors: Vec<Address>) -> Result<(), Error> {
        admin.require_auth();
        
        // Require exactly 3 auditors for 2-out-of-3 multisig
        if auditors.len() != 3 {
            return Err(Error::InvalidInput);
        }
        
        set_auditors(&e, &auditors);

        Ok(())
    }

    /// Request emergency pause by an auditor
    /// Submit an emergency-pause request from an authorised auditor.
    ///
    /// When 2 of the 3 registered auditors have requested a pause the contract
    /// is automatically frozen for the configured pause duration.
    ///
    /// # Errors
    /// - `Unauthorized`  – caller is not a registered auditor.
    /// - `AlreadyVoted`  – this auditor already submitted a request.
    pub fn request_emergency_pause(e: Env, auditor: Address, reason: String) -> Result<(), Error> {
        auditor.require_auth();
        
        // Verify caller is an authorized auditor
        let authorized_auditors = get_auditors(&e);
        if !authorized_auditors.contains(&auditor) {
            return Err(Error::Unauthorized);
        }
        
        let current_time = e.ledger().timestamp();
        let mut requests = get_auditor_pause_requests(&e);
        
        // Check if auditor already requested
        if requests.contains_key(auditor.clone()) {
            return Err(Error::AlreadyVoted);
        }
        
        let request = AuditorPauseRequest {
            auditor: auditor.clone(),
            timestamp: current_time,
            reason: reason.clone(),
        };
        
        requests.set(auditor.clone(), request);
        set_auditor_pause_requests(&e, &requests);
        
        // Check if we have 2-out-of-3 requests
        if requests.len() >= 2 {
            Self::trigger_emergency_pause(&e);
        }

        Ok(())
    }

    /// Internal function to trigger emergency pause
    fn trigger_emergency_pause(e: &Env) {
        let requests = get_auditor_pause_requests(e);
        let current_time = e.ledger().timestamp();
        let pause_duration = get_emergency_pause_duration();
        
        let mut auditors = Vec::new(e);
        let mut reason = String::from_str(e, "Emergency pause requested by auditors: ");
        
        for (auditor_addr, _request) in requests.iter() {
            auditors.push_back(auditor_addr);
            // Simple string concatenation - just use the reason directly
            reason = String::from_str(e, "Emergency pause requested by auditors: ");
        }
        
        let pause = EmergencyPause {
            paused_by: auditors.clone(),
            paused_at: current_time,
            expires_at: current_time + pause_duration,
            reason: reason.clone(),
            is_active: true,
        };
        
        set_emergency_pause(e, &pause);
        
        // Clear the requests
        set_auditor_pause_requests(e, &Map::new(e));
        
        // Emit event
        EmergencyPauseTriggered { auditors: auditors.clone(), paused_at: current_time, expires_at: current_time + pause_duration, reason: reason.clone() }.publish(e);
    }

    /// Check if contract is currently paused
    /// Returns `true` if the contract is currently under an active emergency pause.
    pub fn is_emergency_paused(e: Env) -> bool {
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active {
                let current_time = e.ledger().timestamp();
                return current_time < pause.expires_at;
            }
        }
        false
    }

    /// Get current emergency pause status
    /// Returns the current emergency-pause record, or `None` if no pause is active.
    pub fn get_emergency_pause_status(e: Env) -> Option<EmergencyPause> {
        get_emergency_pause(&e)
    }

    // ========== ISSUE #137: Vesting Simulate Claim Dry-Run Helper ==========
    
    /// Simulate a claim to show exact amounts without consuming gas
    pub fn simulate_claim(e: Env, user: Address, _vesting_id: u32) -> ClaimSimulation {
        let current_time = e.ledger().timestamp();
        
        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active && current_time < pause.expires_at {
                return ClaimSimulation {
                    tokens_to_release: 0,
                    estimated_gas_fee: 0,
                    tax_withholding_amount: 0,
                    net_amount: 0,
                    can_claim: false,
                    reason: String::from_str(&e, "Contract is under emergency pause"),
                };
            }
        }
        
        // Check authorized payout address timelock
        if let Some(auth_address) = storage_get_authorized_payout_address(&e, &user) {
            if auth_address.is_active && current_time < auth_address.effective_at {
                return ClaimSimulation {
                    tokens_to_release: 0,
                    estimated_gas_fee: 0,
                    tax_withholding_amount: 0,
                    net_amount: 0,
                    can_claim: false,
                    reason: String::from_str(&e, "Authorized payout address is still in timelock period"),
                };
            }
        }
        
        // TODO: Calculate actual vesting amounts
        // This is a placeholder - in real implementation, you'd calculate:
        // - tokens_to_release based on vesting schedule
        // - estimated_gas_fee based on current gas prices
        // - tax_withholding_amount based on tax rules
        
        let tokens_to_release = 1000i128; // Placeholder
        let estimated_gas_fee = 50000u64; // Placeholder in stroops
        let tax_withholding_amount = 50i128; // Placeholder
        let net_amount = tokens_to_release - tax_withholding_amount;
        
        ClaimSimulation {
            tokens_to_release,
            estimated_gas_fee,
            tax_withholding_amount,
            net_amount,
            can_claim: true,
            reason: String::from_str(&e, "Claim available"),
        }
    }

    // ========== ISSUE #139: Cross-Project Reputation Bonus Hook ==========
    
    /// Set the reputation bridge contract address
    /// Set the cross-project reputation bridge contract address (admin only).
    pub fn set_reputation_bridge(e: Env, admin: Address, bridge_contract: Address) {
        admin.require_auth();
        set_reputation_bridge_contract(&e, &bridge_contract);
    }

    /// Apply reputation bonus based on cross-project success
    /// Apply a cliff-reduction reputation bonus for a beneficiary who has
    /// achieved 100 % completion on a linked project.
    ///
    /// # Errors
    /// - `AlreadyInitialized` – bonus has already been applied for this address.
    pub fn apply_reputation_bonus(e: Env, beneficiary: Address) -> Result<(), Error> {
        beneficiary.require_auth();
        
        // Check if bonus already applied
        if has_reputation_bonus_applied(&e, &beneficiary) {
            return Err(Error::AlreadyInitialized);
        }
        
        // Get reputation bridge contract
        let _bridge_contract = get_reputation_bridge_contract(&e)
            .expect("Reputation bridge contract not set");
        
        // TODO: Call bridge contract to check completion rate
        // For now, assume 100% completion rate
        let completion_rate = 100u32;
        
        if completion_rate >= 100 {
            let cliff_reduction = 1u32; // 1 month reduction
            let current_time = e.ledger().timestamp();
            
            // Mark bonus as applied
            set_reputation_bonus_applied(&e, &beneficiary);
            
            // Emit event
            ReputationBonusApplied { beneficiary: beneficiary.clone(), cliff_reduction_months: cliff_reduction, applied_at: current_time }.publish(&e);
        }

        Ok(())
    }

    /// Check if user has reputation bonus applied
    /// Returns `true` if the reputation bonus has already been applied for `beneficiary`.
    pub fn has_reputation_bonus(e: Env, beneficiary: Address) -> bool {
        has_reputation_bonus_applied(&e, &beneficiary)
    }

    // ========== ISSUE #138: Milestone-Gated Step Vesting ==========
    
    /// Configure milestone vesting for a vesting schedule
    /// Configure milestone-gated step vesting for a schedule.
    ///
    /// Each entry in `milestone_percentages` is the percentage of total tokens
    /// unlocked when that milestone is completed.  The values must sum to 100.
    ///
    /// # Errors
    /// - `InvalidInput` – percentages do not sum to 100.
    pub fn configure_milestone_vesting(e: Env, admin: Address, vesting_id: u32, milestone_percentages: Vec<u32>) -> Result<(), Error> {
        admin.require_auth();
        
        // Validate percentages sum to 100
        let mut total = 0u32;
        for percentage in milestone_percentages.iter() {
            total += percentage;
        }
        
        if total != 100 {
            return Err(Error::InvalidInput);
        }
        
        let _config = MilestoneConfig {
            vesting_id,
            milestone_percentages: milestone_percentages.clone(),
            total_milestones: milestone_percentages.len() as u32,
        };
        
        set_milestone_configs(&e, vesting_id, &milestone_percentages);

        Ok(())
    }

    /// Complete a milestone (admin only)
    /// Mark a milestone as completed, unlocking the associated token tranche.
    ///
    /// Milestones must be completed sequentially (N-1 before N).
    ///
    /// # Errors
    /// - `AlreadyInitialized`   – milestone already completed.
    /// - `MilestoneNotCompleted` – previous milestone not yet completed.
    pub fn complete_milestone(e: Env, admin: Address, vesting_id: u32, milestone_number: u32) -> Result<(), Error> {
        admin.require_auth();
        
        let mut status = get_milestone_status(&e, vesting_id);
        
        // Check if milestone already completed
        if status.contains_key(milestone_number) {
            return Err(Error::AlreadyInitialized);
        }
        
        // Check sequential completion (milestone N-1 must be completed)
        if milestone_number > 1 {
            if !status.contains_key(milestone_number - 1) {
                return Err(Error::MilestoneNotCompleted);
            }
        }
        
        // Mark milestone as completed
        status.set(milestone_number, true);
        set_milestone_status(&e, vesting_id, &status);
        
        // Emit event
        MilestoneCompleted { vesting_id, milestone_number, completed_at: e.ledger().timestamp() }.publish(&e);

        Ok(())
    }

    /// Get milestone status for a vesting schedule
    /// Return the completion status map (`milestone_number -> completed`) for a schedule.
    pub fn get_milestone_status(e: Env, vesting_id: u32) -> Map<u32, bool> {
        get_milestone_status(&e, vesting_id)
    }

    /// Get milestone configuration for a vesting schedule
    /// Return the milestone percentage configuration for a schedule, if set.
    pub fn get_milestone_config(e: Env, vesting_id: u32) -> Option<Vec<u32>> {
        get_milestone_configs(&e, vesting_id)
    }

    // ========== ISSUE #148 & #95: Zero-Knowledge Privacy Claims Foundation ==========
    
    /// Create a commitment for future private claims
    /// This function allows users to create a commitment that can be used for private claims later
    pub fn create_commitment(e: Env, user: Address, vesting_id: u32, amount: i128, commitment_hash: BytesN<32>) -> Result<(), Error> {
        user.require_auth();
        
        // Check if commitment already exists
        if get_commitment(&e, &commitment_hash).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        
        let current_time = e.ledger().timestamp();
        
        // Create the commitment
        let commitment = Commitment {
            hash: commitment_hash.clone(),
            created_at: current_time,
            vesting_id,
            amount,
            is_used: false,
        };
        
        // Store the commitment
        set_commitment(&e, &commitment_hash.clone(), &commitment);
        
        // Emit event
        CommitmentCreated { commitment_hash, vesting_id, amount, created_at: current_time }.publish(&e);

        Ok(())
    }
    
    /// Execute a private claim using ZK proof
    /// This function allows users to claim tokens without revealing their identity
    pub fn private_claim(e: Env, zk_proof: ZKClaimProof, nullifier: Nullifier, amount: i128) -> Result<(), Error> {
        // No require_auth() - this is a privacy feature
        
        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active {
                let current_time = e.ledger().timestamp();
                if current_time < pause.expires_at {
                    return Err(Error::ContractPaused);
                } else {
                    // Pause expired, remove it
                    remove_emergency_pause(&e);
                }
            }
        }
        
        // Check if nullifier has already been used (prevent double-spending)
        if is_nullifier_used(&e, &nullifier) {
            return Err(Error::AlreadyInitialized);
        }
        
        // Verify the commitment exists and is not used
        let commitment = get_commitment(&e, &zk_proof.commitment_hash)
            .ok_or(Error::VaultNotFound)?;
        
        if commitment.is_used {
            return Err(Error::AlreadyFullyClaimed);
        }
        
        // Verify the commitment amount matches the claim amount
        if commitment.amount != amount {
            return Err(Error::InvalidAmount);
        }
        
        // Verify the Merkle root is valid (for ZK proof verification)
        if !is_valid_merkle_root(&e, &zk_proof.merkle_root) {
            return Err(Error::InvalidInput);
        }
        
        // TODO: Verify actual ZK-SNARK proof
        // This is a placeholder for the actual ZK proof verification
        // In a full implementation, this would use a ZK verification library
        Self::verify_zk_proof(&e, &zk_proof);
        
        // Mark nullifier as used
        set_nullifier_used(&e, &nullifier);
        
        // Mark commitment as used
        mark_commitment_used(&e, &zk_proof.commitment_hash);
        
        // Create privacy claim event
        let current_time = e.ledger().timestamp();
        let privacy_event = PrivacyClaimEvent {
            nullifier: nullifier.clone(),
            amount,
            timestamp: current_time,
            vesting_id: commitment.vesting_id,
            is_private: true,
        };
        
        // Add to privacy claim history
        add_privacy_claim_event(&e, &privacy_event);
        
        // Emit event
        PrivateClaimExecuted { nullifier_hash: nullifier.hash, amount, timestamp: current_time }.publish(&e);
        
        // TODO: Execute actual token transfer
        // This would integrate with the existing vesting logic

        Ok(())
    }
    
    /// Add a Merkle root for ZK proof verification
    /// This function is called by the admin to add new Merkle roots
    pub fn add_merkle_root_admin(e: Env, admin: Address, merkle_root: BytesN<32>) -> Result<(), Error> {
        admin.require_auth();
        
        // Check if Merkle root already exists
        if is_valid_merkle_root(&e, &merkle_root) {
            return Err(Error::AlreadyInitialized);
        }
        
        // Add the Merkle root
        add_merkle_root(&e, &merkle_root);

        Ok(())
    }
    
    /// Get all valid Merkle roots
    /// Return all valid Merkle roots registered for ZK proof verification.
    pub fn get_merkle_roots(e: Env) -> Vec<BytesN<32>> {
        get_merkle_roots(&e)
    }
    
    /// Check if a nullifier has been used
    /// Returns `true` if the given nullifier has already been consumed (double-spend guard).
    pub fn is_nullifier_used_public(e: Env, nullifier: Nullifier) -> bool {
        is_nullifier_used(&e, &nullifier)
    }
    
    /// Get commitment information
    /// Return the commitment record for a given hash, or `None` if not found.
    pub fn get_commitment_info(e: Env, commitment_hash: BytesN<32>) -> Option<Commitment> {
        get_commitment(&e, &commitment_hash)
    }
    
    /// Get privacy claim history
    /// Return the full history of private (ZK) claim events.
    pub fn get_privacy_claim_history(e: Env) -> Vec<PrivacyClaimEvent> {
        storage::get_privacy_claim_history(&e)
    }
    
    /// Placeholder for ZK proof verification
    /// In a full implementation, this would verify the actual ZK-SNARK proof
    fn verify_zk_proof(_e: &Env, _zk_proof: &ZKClaimProof) -> bool {
        // TODO: Implement actual ZK proof verification
        // For now, we'll assume the proof is valid
        // In production, this would integrate with a ZK verification library
        true
    }
    
    /// Enable privacy mode for a vesting schedule
    /// This allows beneficiaries to choose between public and private claims
    pub fn enable_privacy_mode(_e: Env, user: Address, _vesting_id: u32) {
        user.require_auth();
        
        // TODO: Implement privacy mode toggle
        // This would allow users to enable/disable privacy for their vesting
        // For now, this is a placeholder for the architectural foundation
    }
    
    /// Disable privacy mode for a vesting schedule
    pub fn disable_privacy_mode(_e: Env, user: Address, _vesting_id: u32) {
        user.require_auth();
        
        // TODO: Implement privacy mode toggle
        // This would allow users to enable/disable privacy for their vesting
        // For now, this is a placeholder for the architectural foundation
    }

    // ========== ISSUE #146 & #93: Stellar Horizon Path Payment Claim ==========
    
    /// Configure path payment settings for auto-exit feature
    /// This allows users to claim tokens and instantly swap them for USDC in one transaction
    pub fn configure_path_payment(e: Env, admin: Address, destination_asset: Address, min_destination_amount: i128, path: Vec<Address>) {
        admin.require_auth();
        
        let config = PathPaymentConfig {
            destination_asset: destination_asset.clone(),
            min_destination_amount,
            path: path.clone(),
            enabled: true,
        };
        
        set_path_payment_config(&e, &config);
        
        // Emit configuration event
        PathPaymentConfigured { destination_asset, min_destination_amount, path, timestamp: e.ledger().timestamp() }.publish(&e);
    }
    
    /// Disable path payment feature
    pub fn disable_path_payment(e: Env, admin: Address) {
        admin.require_auth();
        
        if let Some(mut config) = get_path_payment_config(&e) {
            config.enabled = false;
            set_path_payment_config(&e, &config);
            
            // Emit disable event
            PathPaymentDisabled { timestamp: e.ledger().timestamp() }.publish(&e);
        }
    }
    
    /// Claim tokens with automatic path payment to USDC (Auto-Exit feature)
    /// This allows users to instantly swap their claimed tokens for USDC in one transaction
    pub fn claim_with_path_payment(e: Env, user: Address, vesting_id: u32, amount: i128, min_destination_amount: Option<i128>) -> Result<(), Error> {
        user.require_auth();

        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active {
                let current_time = e.ledger().timestamp();
                if current_time < pause.expires_at {
                    return Err(Error::ContractPaused);
                } else {
                    // Pause expired, remove it
                    remove_emergency_pause(&e);
                }
            }
        }

        // Get path payment configuration
        let config = get_path_payment_config(&e)
            .ok_or(Error::PathPaymentNotConfigured)?;

        if !config.enabled {
            return Err(Error::PathPaymentDisabled);
        }

        // Use provided min_destination_amount or fallback to config
        let final_min_amount = min_destination_amount.unwrap_or(config.min_destination_amount);
        
        // Validate the amount
        if final_min_amount <= 0 {
            return Err(Error::InvalidInput);
        }

        // TODO: Calculate actual vesting amounts and validate claim
        // This would integrate with the existing vesting logic
        let actual_claimable_amount = amount; // Placeholder - should calculate based on vesting schedule
        
        if actual_claimable_amount <= 0 {
            return Err(Error::NothingToClaim);
        }

        // Execute the path payment using Stellar's built-in path_payment_strict_receive
        // This is the core of the Auto-Exit feature
        let destination_amount = Self::execute_path_payment(&e, &user, actual_claimable_amount, &config.destination_asset, final_min_amount, &config.path)?;
        
        // Record the path payment claim event
        let current_time = e.ledger().timestamp();
        let path_payment_event = PathPaymentClaimEvent {
            beneficiary: user.clone(),
            source_amount: actual_claimable_amount,
            destination_amount,
            destination_asset: config.destination_asset.clone(),
            timestamp: current_time,
            vesting_id,
        };
        
        add_path_payment_claim_event(&e, &path_payment_event);
        
        // Also record in regular claim history for compatibility
        let mut history = get_claim_history(&e);
        let claim_event = ClaimEvent {
            beneficiary: user.clone(),
            amount: actual_claimable_amount,
            timestamp: current_time,
            vesting_id,
        };
        history.push_back(claim_event);
        set_claim_history(&e, &history);
        
        // Emit the path payment claim event
        PathPaymentClaimExecuted { user: user.clone(), source_amount: actual_claimable_amount, destination_amount, destination_asset: config.destination_asset.clone(), timestamp: current_time, vesting_id }.publish(&e);

        Ok(())
    }
    
    /// Simulate a path payment claim to show expected amounts without consuming gas
    pub fn simulate_path_payment_claim(e: Env, _user: Address, _vesting_id: u32, amount: i128, min_destination_amount: Option<i128>) -> PathPaymentSimulation {
        let current_time = e.ledger().timestamp();
        
        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active && current_time < pause.expires_at {
                return PathPaymentSimulation {
                    source_amount: amount,
                    estimated_destination_amount: 0,
                    min_destination_amount: min_destination_amount.unwrap_or(0),
                    path: Vec::new(&e),
                    can_execute: false,
                    reason: String::from_str(&e, "Contract is under emergency pause"),
                    estimated_gas_fee: 0,
                };
            }
        }
        
        // Check if path payment is configured and enabled
        let config = match get_path_payment_config(&e) {
            Some(c) => c,
            None => {
                return PathPaymentSimulation {
                    source_amount: amount,
                    estimated_destination_amount: 0,
                    min_destination_amount: min_destination_amount.unwrap_or(0),
                    path: Vec::new(&e),
                    can_execute: false,
                    reason: String::from_str(&e, "Path payment not configured"),
                    estimated_gas_fee: 0,
                };
            }
        };
        
        if !config.enabled {
            return PathPaymentSimulation {
                source_amount: amount,
                estimated_destination_amount: 0,
                min_destination_amount: min_destination_amount.unwrap_or(0),
                path: config.path.clone(),
                can_execute: false,
                reason: String::from_str(&e, "Path payment feature is disabled"),
                estimated_gas_fee: 0,
            };
        }
        
        // Use provided min_destination_amount or fallback to config
        let final_min_amount = min_destination_amount.unwrap_or(config.min_destination_amount);
        
        // TODO: Calculate actual vesting amounts
        // This would integrate with the existing vesting logic
        let actual_claimable_amount = amount; // Placeholder
        
        if actual_claimable_amount <= 0 {
            return PathPaymentSimulation {
                source_amount: amount,
                estimated_destination_amount: 0,
                min_destination_amount: final_min_amount,
                path: config.path.clone(),
                can_execute: false,
                reason: String::from_str(&e, "No tokens available to claim"),
                estimated_gas_fee: 0,
            };
        }
        
        // Simulate the path payment (in real implementation, this would query Stellar DEX)
        let estimated_destination_amount = Self::simulate_path_payment_result(&e, actual_claimable_amount, &config.destination_asset, &config.path);
        
        let can_execute = estimated_destination_amount >= final_min_amount;
        
        PathPaymentSimulation {
            source_amount: actual_claimable_amount,
            estimated_destination_amount,
            min_destination_amount: final_min_amount,
            path: config.path.clone(),
            can_execute,
            reason: if can_execute {
                String::from_str(&e, "Path payment claim available")
            } else {
                String::from_str(&e, "Insufficient liquidity for minimum destination amount")
            },
            estimated_gas_fee: 150000u64, // Higher gas fee due to path payment complexity
        }
    }
    
    /// Get current path payment configuration
    /// Return the current path-payment (auto-exit) configuration, or `None` if not set.
    pub fn get_path_payment_config(e: Env) -> Option<PathPaymentConfig> {
        get_path_payment_config(&e)
    }
    
    /// Get path payment claim history
    /// Return the history of all path-payment claim events.
    pub fn get_path_payment_claim_history(e: Env) -> Vec<PathPaymentClaimEvent> {
        get_path_payment_claim_history(&e)
    }
    
    /// Internal function to execute the path payment using Stellar's path_payment_strict_receive
    /// This is the core logic that enables the Auto-Exit feature
    fn execute_path_payment(e: &Env, _beneficiary: &Address, source_amount: i128, destination_asset: &Address, min_destination_amount: i128, path: &Vec<Address>) -> Result<i128, Error> {
        // In a real Stellar Soroban implementation, this would use the built-in
        // path_payment_strict_receive function from the Stellar SDK
        
        // For this implementation, we simulate the path payment execution
        // In production, this would be:
        // e.invoke_contract::<i128>(
        //     &stellar_sdk::STELLAR_ASSET_CONTRACT,
        //     &symbol_short!("path_payment_strict_receive"),
        //     (beneficiary, source_amount, destination_asset, min_destination_amount, path)
        // );
        
        // Placeholder implementation - simulate successful path payment
        let simulated_destination_amount = Self::simulate_path_payment_result(e, source_amount, destination_asset, path);
        
        if simulated_destination_amount < min_destination_amount {
            return Err(Error::InsufficientLiquidity);
        }
        
        Ok(simulated_destination_amount)
    }
    
    /// Check if a user's tokens are unlocked for a specific vesting schedule
    /// Returns `true` if the lock-up period for `user` on `vesting_id` has elapsed
    /// (or no lock-up is configured).
    pub fn is_user_unlocked(e: Env, user: Address, vesting_id: u32) -> bool {
        if let Some(lockup_config) = get_lockup_config(&e, vesting_id) {
            if lockup_config.enabled {
                // In a real implementation, this would query the lockup token contract
                // For now, we'll return false (locked) as a placeholder
                false
            } else {
                true // No lock-up configured
            }
        } else {
            true // No lock-up configured
        }
    }
    
    /// Get the unlock time for a user's tokens
    /// Return the Unix timestamp at which `user`'s tokens unlock for `vesting_id`,
    /// or `None` if no lock-up is configured.
    pub fn get_user_unlock_time(e: Env, user: Address, vesting_id: u32) -> Option<u64> {
        if let Some(lockup_config) = get_lockup_config(&e, vesting_id) {
            if lockup_config.enabled {
                // In a real implementation, this would query the lockup token contract
                // For now, we'll return a placeholder
                Some(e.ledger().timestamp() + lockup_config.lockup_duration_seconds)
            } else {
                None // No lock-up configured
            }
        } else {
            None // No lock-up configured
        }
    }

    // ========== ISSUE #211: Lock-Up Periods for Claimed Assets ==========

    /// Configure lock-up period for a vesting schedule
    /// This enables legal compliance requirements where tokens cannot be sold immediately after vesting
    pub fn configure_lockup(e: Env, admin: Address, vesting_id: u32, lockup_duration_seconds: u64, lockup_token_address: Address) {
        admin.require_auth();
        
        let config = LockupConfig {
            vesting_id,
            lockup_duration_seconds,
            enabled: true,
            lockup_token_address: lockup_token_address.clone(),
        };
        
        set_lockup_config(&e, vesting_id, &config);
        
        // Emit configuration event
        LockupConfigured {
            vesting_id,
            lockup_duration_seconds,
            lockup_token_address,
            timestamp: e.ledger().timestamp(),
        }.publish(&e);
    }
    
    /// Disable lock-up period for a vesting schedule
    /// This allows immediate claims without lock-up restrictions
    pub fn disable_lockup(e: Env, admin: Address, vesting_id: u32) {
        admin.require_auth();
        
        remove_lockup_config(&e, vesting_id);
        
        // Emit disable event
        LockupDisabled {
            vesting_id,
            timestamp: e.ledger().timestamp(),
        }.publish(&e);
    }
    
    /// Claim tokens with lock-up period handling
    /// This is the enhanced claim function that handles lock-up periods
    pub fn claim_with_lockup(e: Env, user: Address, vesting_id: u32, amount: i128) -> Result<(), Error> {
        user.require_auth();

        // Check if contract is under emergency pause
        if let Some(pause) = get_emergency_pause(&e) {
            if pause.is_active {
                let current_time = e.ledger().timestamp();
                if current_time < pause.expires_at {
                    return Err(Error::ContractPaused);
                } else {
                    // Pause expired, remove it
                    remove_emergency_pause(&e);
                }
            }
        }

        // Check if user has an authorized payout address
        if let Some(auth_address) = storage_get_authorized_payout_address(&e, &user) {
            if auth_address.is_active {
                let current_time = e.ledger().timestamp();
                
                // Check if timelock has passed
                if current_time < auth_address.effective_at {
                    return Err(Error::TimelockNotElapsed);
                }
            }
        }

        // Check milestone vesting if applicable
        if let Some(_milestone_configs) = get_milestone_configs(&e, vesting_id) {
            let _milestone_status = get_milestone_status(&e, vesting_id);
            // Additional milestone logic would go here
        }

        // Check if lock-up period is configured for this vesting schedule
        if let Some(lockup_config) = get_lockup_config(&e, vesting_id) {
            if lockup_config.enabled {
                // Issue wrapped tokens instead of raw tokens
                Self::issue_wrapped_tokens(&e, &user, vesting_id, amount, &lockup_config);
                return;
            }
        }

        // Original claim logic for non-lockup cases
        let mut history = get_claim_history(&e);

        let event = ClaimEvent {
            beneficiary: user.clone(),
            amount,
            timestamp: e.ledger().timestamp(),
            vesting_id,
        };

        history.push_back(event);

        set_claim_history(&e, &history);

        Ok(())
    }
    
    /// Internal function to issue wrapped tokens during lock-up period
    fn issue_wrapped_tokens(e: &Env, user: &Address, vesting_id: u32, amount: i128, lockup_config: &LockupConfig) {
        let current_time = e.ledger().timestamp();
        let unlock_time = current_time + lockup_config.lockup_duration_seconds;
        
        // Call the lockup token contract to issue wrapped tokens
        // In a real implementation, this would be a cross-contract call
        // For now, we'll simulate this by emitting an event
        
        // Record the lockup claim event
        let mut history = get_claim_history(e);
        
        let claim_event = ClaimEvent {
            beneficiary: user.clone(),
            amount,
            timestamp: current_time,
            vesting_id,
        };
        
        history.push_back(claim_event);
        set_claim_history(e, &history);
        
        // Emit lockup claim event
        LockupClaimExecuted {
            user: user.clone(),
            vesting_id,
            amount,
            lockup_token_address: lockup_config.lockup_token_address.clone(),
            unlock_time,
            timestamp: current_time,
        }.publish(e);
        
        // In a real implementation, this would invoke the lockup token contract:
        // e.invoke_contract::<()>(
        //     &lockup_config.lockup_token_address,
        //     &symbol_short!("issue_wrapped_tokens"),
        //     (e.current_contract_address(), user.clone(), vesting_id, amount)
        // );
    }

    // ========== ISSUE #114 & #212: Beneficiary Reassignment with Governance Veto ==========

    /// Initialize total token supply for governance calculations
    /// Initialise the total token supply used for governance veto-power calculations.
    pub fn initialize_token_supply(e: Env, admin: Address, total_supply: i128) {
        admin.require_auth();
        
        let supply_info = TokenSupplyInfo {
            total_supply,
            last_updated: e.ledger().timestamp(),
        };
        
        set_token_supply_info(&e, &supply_info);
    }
    
    /// Update total token supply (for dynamic supply tracking)
    /// Update the total token supply (for dynamic supply tracking).
    pub fn update_token_supply(e: Env, admin: Address, new_total_supply: i128) {
        admin.require_auth();
        
        let supply_info = TokenSupplyInfo {
            total_supply: new_total_supply,
            last_updated: e.ledger().timestamp(),
        };
        
        set_token_supply_info(&e, &supply_info);
    }
    
    /// Set governance veto threshold percentage
    pub fn set_governance_veto_threshold(e: Env, admin: Address, threshold_percentage: u32) -> Result<(), Error> {
        admin.require_auth();
        
        if threshold_percentage > 100 {
            return Err(Error::InvalidInput);
        }
        
        set_governance_veto_threshold(&e, threshold_percentage);

        Ok(())
    }
    
    /// Request beneficiary reassignment with governance veto protection
    pub fn request_beneficiary_reassignment(e: Env, current_beneficiary: Address, new_beneficiary: Address, vesting_id: u32, total_amount: i128) {
        current_beneficiary.require_auth();
        
        let current_time = e.ledger().timestamp();
        let veto_period = get_governance_veto_period();
        
        // Check if reassignment exceeds 5% threshold
        let supply_info = get_token_supply_info(&e);
        let threshold = get_governance_veto_threshold(&e);
        let threshold_amount = (supply_info.total_supply * threshold as i128) / 100;
        
        let requires_governance_veto = total_amount > threshold_amount;
        let effective_at = if requires_governance_veto {
            current_time + veto_period // 7-day timelock for large reassignments
        } else {
            current_time + get_timelock_duration() // 48-hour timelock for small reassignments
        };
        
        // Generate reassignment ID
        let reassignment_id = get_reassignment_counter(&e) + 1;
        set_reassignment_counter(&e, reassignment_id);
        
        let reassignment = BeneficiaryReassignment {
            vesting_id,
            current_beneficiary: current_beneficiary.clone(),
            new_beneficiary: new_beneficiary.clone(),
            requested_at: current_time,
            effective_at,
            total_amount,
            requires_governance_veto,
            is_executed: false,
        };
        
        set_beneficiary_reassignment(&e, reassignment_id, &reassignment);
        
        // Emit reassignment request event
        BeneficiaryReassignmentRequested {
            reassignment_id,
            vesting_id,
            current_beneficiary: current_beneficiary.clone(),
            new_beneficiary: new_beneficiary.clone(),
            total_amount,
            effective_at,
            requires_governance_veto,
        }.publish(&e);
        
        // If governance veto is required, start veto period
        if requires_governance_veto {
            VetoPeriodStarted {
                reassignment_id,
                vesting_id,
                veto_deadline: effective_at,
                threshold_percentage: threshold,
            }.publish(&e);
        }
    }
    
    /// Execute beneficiary reassignment after timelock period
    pub fn execute_beneficiary_reassignment(e: Env, reassignment_id: u32) -> Result<(), Error> {
        let mut reassignment = get_beneficiary_reassignment(&e, reassignment_id)
            .ok_or(Error::VaultNotFound)?;
        
        if reassignment.is_executed {
            return Err(Error::AlreadyInitialized);
        }
        
        let current_time = e.ledger().timestamp();
        
        if current_time < reassignment.effective_at {
            return Err(Error::TimelockNotElapsed);
        }
        
        // Check if governance veto was triggered
        if reassignment.requires_governance_veto {
            let veto_votes = get_veto_votes(&e, reassignment_id);
            let total_veto_power = veto_votes.iter()
                .filter(|vote| vote.vote_for_veto)
                .map(|vote| vote.voting_power)
                .sum();
            
            let threshold = get_governance_veto_threshold(&e);
            let supply_info = get_token_supply_info(&e);
            let veto_threshold = (supply_info.total_supply * threshold as i128) / 100;
            
            if total_veto_power >= veto_threshold {
                return Err(Error::QuorumNotMet);
            }
        }
        
        // Execute the reassignment
        reassignment.is_executed = true;
        set_beneficiary_reassignment(&e, reassignment_id, &reassignment);
        
        // In a real implementation, this would update the actual vesting schedule beneficiary
        // For now, we'll just emit the event
        
        // Emit execution event
        BeneficiaryReassignmentExecuted {
            reassignment_id,
            vesting_id: reassignment.vesting_id,
            old_beneficiary: reassignment.current_beneficiary,
            new_beneficiary: reassignment.new_beneficiary,
            executed_at: current_time,
        }.publish(&e);

        Ok(())
    }
    
    /// Cast a vote for or against a beneficiary reassignment
    pub fn cast_veto_vote(e: Env, voter: Address, reassignment_id: u32, vote_for_veto: bool, voting_power: i128) -> Result<(), Error> {
        voter.require_auth();
        
        let reassignment = get_beneficiary_reassignment(&e, reassignment_id)
            .ok_or(Error::VaultNotFound)?;
        
        if !reassignment.requires_governance_veto {
            return Err(Error::InvalidInput);
        }
        
        if reassignment.is_executed {
            return Err(Error::AlreadyInitialized);
        }
        
        let current_time = e.ledger().timestamp();
        
        if current_time >= reassignment.effective_at {
            return Err(Error::VotingPeriodEnded);
        }
        
        // Check if voter has already voted
        let existing_votes = get_veto_votes(&e, reassignment_id);
        if existing_votes.iter().any(|vote| vote.voter == voter) {
            return Err(Error::AlreadyVoted);
        }
        
        let vote = VetoVote {
            voter: voter.clone(),
            reassignment_id,
            vote_for_veto,
            voting_power,
            voted_at: current_time,
        };
        
        add_veto_vote(&e, reassignment_id, &vote);
        
        // Emit vote event
        VetoVoteCast {
            voter: voter.clone(),
            reassignment_id,
            vote_for_veto,
            voting_power,
            voted_at: current_time,
        }.publish(&e);
        
        // Check if veto threshold is reached
        let all_votes = get_veto_votes(&e, reassignment_id);
        let total_veto_power = all_votes.iter()
            .filter(|vote| vote.vote_for_veto)
            .map(|vote| vote.voting_power)
            .sum();
        
        let threshold = get_governance_veto_threshold(&e);
        let supply_info = get_token_supply_info(&e);
        let veto_threshold = (supply_info.total_supply * threshold as i128) / 100;
        
        if total_veto_power >= veto_threshold {
            // Veto threshold reached - cancel the reassignment
            remove_beneficiary_reassignment(&e, reassignment_id);
            
            // Emit veto event
            ReassignmentVetoed {
                reassignment_id,
                veto_triggered_by: voter,
                veto_power: total_veto_power,
                vetoed_at: current_time,
            }.publish(&e);
        }

        Ok(())
    }
    
    /// Get beneficiary reassignment details
    /// Return the reassignment record for `reassignment_id`, or `None` if not found.
    pub fn get_beneficiary_reassignment(e: Env, reassignment_id: u32) -> Option<BeneficiaryReassignment> {
        get_beneficiary_reassignment(&e, reassignment_id)
    }
    
    /// Get veto votes for a reassignment
    /// Return all veto votes cast for a given reassignment.
    pub fn get_veto_votes(e: Env, reassignment_id: u32) -> Vec<VetoVote> {
        get_veto_votes(&e, reassignment_id)
    }
    
    /// Get current token supply info
    /// Return the current token supply info used for governance calculations.
    pub fn get_token_supply_info(e: Env) -> TokenSupplyInfo {
        get_token_supply_info(&e)
    }
    
    /// Get current governance veto threshold
    /// Return the current governance veto threshold percentage (0–100).
    pub fn get_governance_veto_threshold(e: Env) -> u32 {
        get_governance_veto_threshold(&e)
    }
    
    /// Check if a reassignment requires governance veto
    /// Returns `true` if a reassignment of `amount` tokens requires a governance veto period.
    pub fn requires_governance_veto(e: Env, amount: i128) -> bool {
        let supply_info = get_token_supply_info(&e);
        let threshold = get_governance_veto_threshold(&e);
        let threshold_amount = (supply_info.total_supply * threshold as i128) / 100;
        
        amount > threshold_amount
    }
    
    /// Get veto status for a reassignment
    /// Return `(is_vetoed, total_veto_power, veto_threshold)` for a reassignment.
    pub fn get_veto_status(e: Env, reassignment_id: u32) -> (bool, i128, i128) {
        let votes = get_veto_votes(&e, reassignment_id);
        let total_veto_power = votes.iter()
            .filter(|vote| vote.vote_for_veto)
            .map(|vote| vote.voting_power)
            .sum();
        
        let threshold = get_governance_veto_threshold(&e);
        let supply_info = get_token_supply_info(&e);
        let veto_threshold = (supply_info.total_supply * threshold as i128) / 100;
        
        let is_vetoed = total_veto_power >= veto_threshold;
        
        (is_vetoed, total_veto_power, veto_threshold)
    }

    // ========== ISSUE #205: Automated Tax Withholding Logic ==========
    
    /// Configure tax withholding settings
    pub fn configure_tax_withholding(e: Env, admin: Address, tax_treasury_address: Address, tax_withholding_bps: u32) -> Result<(), Error> {
        admin.require_auth();
        
        // Validate tax rate (basis points, 10000 = 100%)
        if tax_withholding_bps > 10000 {
            return Err(Error::InvalidInput);
        }
        
        let config = TaxWithholdingConfig {
            tax_treasury_address: tax_treasury_address.clone(),
            tax_withholding_bps,
            enabled: true,
        };
        
        set_tax_withholding_config(&e, &config);
        
        // Emit configuration event
        TaxWithholdingConfigured { tax_treasury_address, tax_withholding_bps, timestamp: e.ledger().timestamp() }.publish(&e);

        Ok(())
    }
    
    /// Disable tax withholding feature
    /// Disable the tax withholding feature (admin only).
    pub fn disable_tax_withholding(e: Env, admin: Address) {
        admin.require_auth();
        
        if let Some(mut config) = get_tax_withholding_config(&e) {
            config.enabled = false;
            set_tax_withholding_config(&e, &config);
            
            // Emit disable event
            TaxWithholdingDisabled { timestamp: e.ledger().timestamp() }.publish(&e);
        }
    }
    
    /// Get current tax withholding configuration
    /// Return the current tax withholding configuration, or `None` if not set.
    pub fn get_tax_withholding_config(e: Env) -> Option<TaxWithholdingConfig> {
        get_tax_withholding_config(&e)
    }
    
    /// Internal function to calculate and execute tax withholding
    fn execute_tax_withholding(e: &Env, gross_amount: i128) -> (i128, i128, Address) {
        if let Some(config) = get_tax_withholding_config(e) {
            if config.enabled {
                // Calculate tax amount (basis points)
                let tax_amount = (gross_amount * config.tax_withholding_bps as i128) / 10000i128;
                let net_amount = gross_amount - tax_amount;
                
                return (net_amount, tax_amount, config.tax_treasury_address);
            }
        }
        
        // No tax withholding configured or disabled
        (gross_amount, 0i128, Address::from_string(&String::from_str(e, "placeholder")))
    }

    // ========== ISSUE #204: On-Chain SEP-12 KYC Gating for Claims ==========
    
    /// Configure SEP-12 Identity Oracle
    pub fn configure_sep12_oracle(e: Env, admin: Address, oracle_address: Address) {
        admin.require_auth();
        
        let oracle = SEP12IdentityOracle {
            contract_address: oracle_address.clone(),
            enabled: true,
        };
        
        set_sep12_identity_oracle(&e, &oracle);
        
        // Emit configuration event
        SEP12OracleConfigured { oracle_address, timestamp: e.ledger().timestamp() }.publish(&e);
    }
    
    /// Disable SEP-12 KYC checking
    /// Disable SEP-12 KYC checking (admin only).
    pub fn disable_sep12_kyc(e: Env, admin: Address) {
        admin.require_auth();
        
        if let Some(mut oracle) = get_sep12_identity_oracle(&e) {
            oracle.enabled = false;
            set_sep12_identity_oracle(&e, &oracle);
            
            // Emit disable event
            SEP12KYCDisabled { timestamp: e.ledger().timestamp() }.publish(&e);
        }
    }
    
    /// Get current SEP-12 Identity Oracle configuration
    /// Return the current SEP-12 Identity Oracle configuration, or `None` if not set.
    pub fn get_sep12_oracle_config(e: Env) -> Option<SEP12IdentityOracle> {
        get_sep12_identity_oracle(&e)
    }
    
    /// Internal function to check KYC status via SEP-12
    fn check_kyc_status(e: &Env, beneficiary: &Address) -> Result<bool, String> {
        if let Some(oracle) = get_sep12_identity_oracle(e) {
            if oracle.enabled {
                // Placeholder: assume KYC check passes for demonstration
                let is_verified = Self::simulate_sep12_check(e, beneficiary);
                
                if !is_verified {
                    // Emit KYC check failed event
                    KYCCheckFailed {
                        beneficiary: beneficiary.clone(),
                        reason: String::from_str(e, "SEP-12 KYC verification failed"),
                        timestamp: e.ledger().timestamp(),
                    }.publish(e);
                    
                    return Err(String::from_str(e, "SEP-12 KYC verification failed"));
                }
                
                return Ok(true);
            }
        }
        
        // No SEP-12 oracle configured or disabled - allow claim
        Ok(true)
    }
    
    /// Placeholder for SEP-12 identity verification simulation
    fn simulate_sep12_check(_e: &Env, _beneficiary: &Address) -> bool {
        // In production, this would be an actual cross-contract call to SEP-12
        true
    }

    // ========== ISSUE #203: Handle Zero-Decimal Token Precision Safely ==========
    
    /// Register token metadata for precision handling
    pub fn register_token_metadata(e: Env, admin: Address, asset_address: Address, decimals: u32) -> Result<(), Error> {
        admin.require_auth();
        
        // Validate decimals (0-18 typical range for Stellar assets)
        if decimals > 18 {
            return Err(Error::InvalidInput);
        }
        
        let metadata = TokenMetadata {
            decimals,
            asset_address: asset_address.clone(),
        };
        
        set_token_metadata(&e, &asset_address, &metadata);
        
        // Emit registration event
        TokenMetadataRegistered { asset_address, decimals, timestamp: e.ledger().timestamp() }.publish(&e);

        Ok(())
    }
    
    /// Get token metadata
    /// Return the registered token metadata for `asset_address`, or `None` if not registered.
    pub fn get_token_metadata_info(e: Env, asset_address: Address) -> Option<TokenMetadata> {
        get_token_metadata(&e, &asset_address)
    }
    
    /// Precision-agnostic division function
    /// Prevents rounding-to-zero errors when dealing with low-decimal tokens
    fn precision_safe_divide(e: &Env, amount: i128, divisor: i128, asset_address: &Address) -> i128 {
        // Get token metadata to determine precision
        let decimals = if let Some(metadata) = get_token_metadata(e, asset_address) {
            metadata.decimals
        } else {
            // Default to 7 decimals (XLM standard) if not registered
            7
        };
        
        // For low-decimal tokens, we need to handle division carefully
        if decimals == 0 {
            // Zero-decimal tokens - use integer division with careful rounding
            if amount < divisor {
                // Prevent rounding to zero by returning minimum unit
                1i128
            } else {
                amount / divisor
            }
        } else if decimals <= 2 {
            // Low-decimal tokens (1-2 decimals) - use enhanced precision
            // Multiply by a scaling factor to prevent rounding to zero
            let scaling_factor = 10i128.pow(decimals as u32);
            let scaled_amount = amount * scaling_factor;
            let result = (scaled_amount / divisor) / scaling_factor;
            
            // Ensure we don't return zero when there should be a minimal amount
            if result == 0 && amount > 0 {
                1i128
            } else {
                result
            }
        } else {
            // Normal precision tokens (3+ decimals) - standard division
            amount / divisor
        }
    }

    // ========== ISSUE #202: Implement Revocability Expiration (Cliff-Drop) ==========
    
    /// Create a new vesting grant with revocability expiration
    pub fn create_vesting_grant(e: Env, admin: Address, vesting_id: u32, beneficiary: Address, is_revocable: bool) -> Result<(), Error> {
        admin.require_auth();
        
        // Check if grant already exists
        if get_vesting_grant(&e, vesting_id).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        
        let current_time = e.ledger().timestamp();
        let twelve_months = 12 * 30 * 24 * 60 * 60; // Approximate 12 months in seconds
        
        let grant = VestingGrant {
            vesting_id,
            beneficiary: beneficiary.clone(),
            created_at: current_time,
            is_revocable,
            revocability_expires_at: current_time + twelve_months,
        };
        
        set_vesting_grant(&e, vesting_id, &grant);
        
        // Emit grant creation event
        VestingGrantCreated {
            vesting_id,
            beneficiary,
            is_revocable,
            revocability_expires_at: grant.revocability_expires_at,
            created_at: current_time,
        }.publish(&e);

        Ok(())
    }
    
    /// Check if a grant is still revocable
    pub fn is_grant_revocable(e: Env, vesting_id: u32) -> bool {
        if let Some(grant) = get_vesting_grant(&e, vesting_id) {
            if !grant.is_revocable {
                return false;
            }
            
            let current_time = e.ledger().timestamp();
            if current_time >= grant.revocability_expires_at {
                // Revocability has expired - update the grant
                let mut updated_grant = grant.clone();
                updated_grant.is_revocable = false;
                set_vesting_grant(&e, vesting_id, &updated_grant);
                
                // Emit expiration event
                RevocabilityExpired {
                    vesting_id,
                    beneficiary: grant.beneficiary,
                    expired_at: current_time,
                }.publish(&e);
                
                return false;
            }
            
            return true;
        }
        
        false // Grant doesn't exist
    }
    
    /// Get vesting grant information
    /// Return the vesting grant record for `vesting_id`, or `None` if not found.
    pub fn get_vesting_grant_info(e: Env, vesting_id: u32) -> Option<VestingGrant> {
        get_vesting_grant(&e, vesting_id)
    }

    // ========== COMPLIANCE HELPER FUNCTIONS ==========

    /// Check if user has completed KYC verification
    fn is_kyc_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual KYC verification check
        // This would typically integrate with a KYC provider oracle
        // For now, return true as placeholder
        true
    }
    
    /// Get KYC expiration timestamp for user
    fn get_kyc_expiry(_e: &Env, _user: &Address) -> Option<u64> {
        // TODO: Implement actual KYC expiry check
        // This would typically be stored from KYC provider data
        // For now, return None (no expiry)
        None
    }
    
    /// Check if address is on sanctions list
    fn is_address_sanctioned(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual sanctions check
        // This would integrate with sanctions screening oracle
        // For now, return false as placeholder
        false
    }
    
    /// Check if user's jurisdiction is restricted
    fn is_jurisdiction_restricted(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual jurisdiction check
        // This would check user's location against restricted jurisdictions
        // For now, return false as placeholder
        false
    }
    
    /// Check if user has valid legal signature for this vesting
    fn has_valid_legal_signature(_e: &Env, _user: &Address, _vesting_id: u32) -> bool {
        // TODO: Implement actual legal signature verification
        // This would verify digital signatures against legal documents
        // For now, return true as placeholder
        true
    }
    
    /// Get AML threshold for the contract
    fn get_aml_threshold(_e: &Env) -> i128 {
        // TODO: Implement actual AML threshold
        // This would be configurable based on regulatory requirements
        // For now, return a high threshold
        1000000i128
    }
    
    /// Get user's risk rating (lower is better)
    fn get_user_risk_rating(_e: &Env, _user: &Address) -> u32 {
        // TODO: Implement actual risk rating calculation
        // This would integrate with risk assessment oracle
        // For now, return low risk
        1u32
    }
    
    /// Get maximum allowed risk rating
    fn get_max_allowed_risk(_e: &Env) -> u32 {
        // TODO: Implement actual max risk configuration
        // This would be configurable based on risk appetite
        // For now, allow moderate risk
        5u32
    }
    
    /// Check if user's documents are verified
    fn are_documents_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual document verification check
        // This would check verification status of required documents
        // For now, return true as placeholder
        true
    }
    
    /// Check if accreditation is required for this vesting
    fn is_accreditation_required(_e: &Env, _vesting_id: u32) -> bool {
        // TODO: Implement actual accreditation requirement check
        // This would check if this vesting requires accredited investor status
        // For now, return false as placeholder
        false
    }
    
    /// Check if user is accredited investor
    fn is_user_accredited(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual accreditation check
        // This would verify accredited investor status
        // For now, return true as placeholder
        true
    }
    
    /// Check if user is tax compliant
    fn is_tax_compliant(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual tax compliance check
        // This would check tax withholding and reporting status
        // For now, return true as placeholder
        true
    }
    
    /// Check if regulatory block is active
    fn is_regulatory_block_active(_e: &Env) -> bool {
        // TODO: Implement actual regulatory block check
        // This would check for regulatory holds or blocks
        // For now, return false as placeholder
        false
    }
    
    /// Check if user is approved on whitelist
    fn is_whitelist_approved(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual whitelist approval check
        // This would check against approved investor whitelist
        // For now, return true as placeholder
        true
    }
    
    /// Check if user is on blacklist
    fn is_on_blacklist(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual blacklist check
        // This would check against prohibited persons list
        // For now, return false as placeholder
        false
    }
    
    /// Check if user is subject to geofencing restrictions
    fn is_geofencing_restricted(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual geofencing check
        // This would check IP/location-based restrictions
        // For now, return false as placeholder
        false
    }
    
    /// Get identity verification expiration for user
    fn get_identity_expiry(_e: &Env, _user: &Address) -> Option<u64> {
        // TODO: Implement actual identity expiry check
        // This would check when identity verification expires
        // For now, return None (no expiry)
        None
    }
    
    /// Check if user's source of funds is verified
    fn is_source_of_funds_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual source of funds verification
        // This would verify origin of funds for AML compliance
        // For now, return true as placeholder
        true
    }
    
    /// Check if user's beneficial owners are verified
    fn is_beneficial_owner_verified(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual beneficial owner verification
        // This would verify ultimate beneficial ownership
        // For now, return true as placeholder
        true
    }
    
    /// Check if user is a politically exposed person
    fn is_politically_exposed_person(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual PEP check
        // This would screen against PEP lists
        // For now, return false as placeholder
        false
    }
    
    /// Check if user appears on sanctions lists
    fn is_on_sanctions_list(_e: &Env, _user: &Address) -> bool {
        // TODO: Implement actual sanctions list screening
        // This would check multiple sanctions databases
        // For now, return false as placeholder
        false
    }

    // ========== LST SUPPORT ==========

    /// Configure LST support for a vesting schedule
    pub fn configure_lst(e: Env, admin: Address, vesting_id: u32, lst_token_address: Address, base_token_address: Address) {
        admin.require_auth();

        let config = LSTConfig {
            vesting_id,
            enabled: true,
            lst_token_address: lst_token_address.clone(),
            base_token_address: base_token_address.clone(),
        };

        set_lst_config(&e, vesting_id, &config);

        LSTConfigured {
            vesting_id,
            lst_token_address,
            base_token_address,
            timestamp: e.ledger().timestamp(),
        }.publish(&e);
    }

    /// Fetch the current exchange rate between base token and LST
    /// For simulation purposes, we assume 1 base token = 0.9 LST (or similar)
    /// In production, this would call the LST contract's exchange rate oracle
    fn get_lst_exchange_rate(_e: &Env, _lst_token: &Address) -> i128 {
        // Exchange rate with 7 decimals precision (e.g., 1 LST = 1.1 Base Token -> rate is 0.9090909)
        // Returning a mocked constant for rebasing math: 0.9 LST per base token (9_000_000)
        9_000_000i128
    }

    // ========== ISSUE #223: Cross-Contract balanceOf Adapter for DAO Voting ==========

    /// Returns the voting power of an address, defined as its total unvested token balance.
    /// DAO governance contracts can call this to allow employees to vote with locked tokens,
    /// ensuring protocol alignment even before tokens are fully vested.
    pub fn get_voting_power(e: Env, voter: Address) -> i128 {
        let power = get_unvested_balance(&e, &voter);
        VotingPowerQueried {
            voter,
            voting_power: power,
            timestamp: e.ledger().timestamp(),
        }.publish(&e);
        power
    }

    /// Admin function to record/update an address's unvested balance.
    /// Called when vaults are created or tokens are claimed to keep the balance current.
    pub fn record_unvested_balance(e: Env, admin: Address, beneficiary: Address, unvested_amount: i128) -> Result<(), Error> {
        admin.require_auth();
        if unvested_amount < 0 {
            return Err(Error::InvalidInput);
        }
        set_unvested_balance(&e, &beneficiary, unvested_amount);

        Ok(())
    }

    // ========== ISSUE #226: Admin Dead-Man's Switch ==========

    /// Configure the recovery address for the admin dead-man's switch.
    /// If the admin is inactive for 365 days, the recovery address can claim admin rights.
    pub fn set_admin_recovery_address(e: Env, admin: Address, recovery_address: Address) -> Result<(), Error> {
        admin.require_auth();

        if recovery_address == admin {
            return Err(Error::RecoveryAddressInvalid);
        }

        let current_time = e.ledger().timestamp();

        let switch = AdminDeadManSwitch {
            recovery_address: recovery_address.clone(),
            last_admin_activity: current_time,
            is_triggered: false,
        };

        set_admin_dead_man_switch(&e, &switch);

        AdminRecoveryAddressSet {
            recovery_address,
            set_at: current_time,
        }.publish(&e);

        Ok(())
    }

    /// Record admin activity to reset the dead-man's switch inactivity timer.
    /// Admin should call this periodically (at least once per year) to prevent recovery.
    pub fn ping_admin_activity(e: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();

        let mut switch = get_admin_dead_man_switch(&e)
            .ok_or(Error::AdminSwitchNotConfigured)?;

        if switch.is_triggered {
            return Err(Error::AdminSwitchAlreadyTriggered);
        }

        let current_time = e.ledger().timestamp();
        switch.last_admin_activity = current_time;
        set_admin_dead_man_switch(&e, &switch);

        AdminActivityRecorded {
            admin,
            timestamp: current_time,
        }.publish(&e);

        Ok(())
    }

    /// Claim admin rights after 365 days of admin inactivity.
    /// Only the pre-configured recovery address can call this.
    pub fn claim_admin_recovery(e: Env, recovery_address: Address) -> Result<(), Error> {
        recovery_address.require_auth();

        let mut switch = get_admin_dead_man_switch(&e)
            .ok_or(Error::AdminSwitchNotConfigured)?;

        if switch.is_triggered {
            return Err(Error::AdminSwitchAlreadyTriggered);
        }

        if switch.recovery_address != recovery_address {
            return Err(Error::Unauthorized);
        }

        let current_time = e.ledger().timestamp();
        let elapsed = current_time.saturating_sub(switch.last_admin_activity);

        if elapsed < ADMIN_INACTIVITY_TIMEOUT {
            return Err(Error::AdminInactivityNotElapsed);
        }

        switch.is_triggered = true;
        set_admin_dead_man_switch(&e, &switch);

        AdminRecoveryClaimed {
            recovery_address,
            claimed_at: current_time,
        }.publish(&e);

        Ok(())
    }

    /// Get the current admin dead-man's switch state.
    pub fn get_admin_switch_state(e: Env) -> Option<AdminDeadManSwitch> {
        get_admin_dead_man_switch(&e)
    }

    // ========== ISSUE #228: Oracle Price Deviation Circuit Breaker ==========

    /// Submit a new oracle price. If the price deviates >30% from the previous ledger's
    /// price, the vault is frozen to prevent oracle manipulation attacks.
    /// Returns Err(OracleCircuitBreakerActive) if already frozen.
    /// Returns Err(OraclePriceDeviationTooHigh) if this update trips the breaker.
    pub fn update_oracle_price(e: Env, admin: Address, new_price: i128) -> Result<(), Error> {
        admin.require_auth();

        if new_price <= 0 {
            return Err(Error::InvalidInput);
        }

        let current_time = e.ledger().timestamp();
        let current_ledger = e.ledger().sequence();

        match get_oracle_price_record(&e) {
            None => {
                // First price submission — just store it
                set_oracle_price_record(&e, &OraclePriceRecord {
                    last_price: new_price,
                    last_ledger: current_ledger,
                    is_frozen: false,
                    frozen_at: 0,
                });
                OraclePriceUpdated {
                    old_price: 0,
                    new_price,
                    ledger: current_ledger,
                    timestamp: current_time,
                }.publish(&e);
            }
            Some(record) => {
                if record.is_frozen {
                    return Err(Error::OracleCircuitBreakerActive);
                }

                // Only check deviation when the update is within the same ledger
                if record.last_ledger == current_ledger {
                    let deviation_bps = Self::calc_deviation_bps(record.last_price, new_price);
                    if deviation_bps > ORACLE_DEVIATION_THRESHOLD_BPS {
                        // Trip the circuit breaker
                        set_oracle_price_record(&e, &OraclePriceRecord {
                            last_price: record.last_price,
                            last_ledger: current_ledger,
                            is_frozen: true,
                            frozen_at: current_time,
                        });
                        OracleCircuitBreakerTripped {
                            old_price: record.last_price,
                            new_price,
                            deviation_bps,
                            tripped_at: current_time,
                        }.publish(&e);
                        return Err(Error::OraclePriceDeviationTooHigh);
                    }
                }

                set_oracle_price_record(&e, &OraclePriceRecord {
                    last_price: new_price,
                    last_ledger: current_ledger,
                    is_frozen: record.is_frozen,
                    frozen_at: record.frozen_at,
                });
                OraclePriceUpdated {
                    old_price: record.last_price,
                    new_price,
                    ledger: current_ledger,
                    timestamp: current_time,
                }.publish(&e);
            }
        }

        Ok(())
    }

    /// Admin resets the oracle circuit breaker after manual review.
    pub fn reset_oracle_circuit_breaker(e: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();

        let mut record = get_oracle_price_record(&e)
            .ok_or(Error::InvalidInput)?;

        record.is_frozen = false;
        record.frozen_at = 0;
        set_oracle_price_record(&e, &record);

        OracleCircuitBreakerReset {
            reset_by: admin,
            reset_at: e.ledger().timestamp(),
        }.publish(&e);

        Ok(())
    }

    /// Returns the current oracle price record.
    pub fn get_oracle_price(e: Env) -> Option<OraclePriceRecord> {
        get_oracle_price_record(&e)
    }

    /// Internal helper: compute deviation in basis points between two prices.
    fn calc_deviation_bps(old_price: i128, new_price: i128) -> u32 {
        if old_price == 0 {
            return 0;
        }
        let diff = if new_price > old_price {
            new_price - old_price
        } else {
            old_price - new_price
        };
        // deviation_bps = (diff * 10000) / old_price
        ((diff * 10_000) / old_price) as u32
    }

    // ========== ISSUE #231: Self-Destruct Prevention & Storage Locking ==========

    /// Guard function that MUST be called before any contract upgrade or deletion.
    /// Returns Err(UpgradeBlockedByUnvestedFunds) if Total_Unvested_Balance > 0,
    /// ensuring user funds can never be trapped by a malicious admin action.
    pub fn assert_safe_to_upgrade(e: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();

        let total_unvested = get_contract_total_unvested(&e);
        if total_unvested > 0 {
            UpgradeBlocked {
                total_unvested_balance: total_unvested,
                blocked_at: e.ledger().timestamp(),
            }.publish(&e);
            return Err(Error::UpgradeBlockedByUnvestedFunds);
        }

        Ok(())
    }

    /// Returns the contract-wide total unvested balance.
    /// Used by assert_safe_to_upgrade and external auditors.
    pub fn get_total_unvested_balance(e: Env) -> i128 {
        get_contract_total_unvested(&e)
    }

    /// Admin function to update the contract-wide total unvested balance.
    /// Should be called whenever vaults are created, claimed from, or revoked.
    pub fn update_total_unvested_balance(e: Env, admin: Address, new_total: i128) -> Result<(), Error> {
        admin.require_auth();
        if new_total < 0 {
            return Err(Error::InvalidInput);
        }
        set_contract_total_unvested(&e, new_total);
        Ok(())
    }

    // ========== ISSUE #276: Vesting Schedule Consolidation and Mergers ==========
    
    /// Merge multiple vesting schedules into a single master schedule
    /// 
    /// This function allows employees to consolidate multiple sequential grants
    /// into a single unified schedule, reducing transaction overhead and storage footprint.
    /// 
    /// # Parameters
    /// - `user` - The beneficiary address initiating the merge
    /// - `schedule_ids` - Array of schedule IDs to merge (must belong to caller)
    /// 
    /// # Security Features
    /// - All schedules must belong to the calling user
    /// - All schedules must have the same underlying asset
    /// - Weighted-average calculation prevents artificial acceleration of unlock dates
    /// - Mathematical integrity ensures total area under vesting curve remains identical
    /// 
    /// # Errors
    /// - `InsufficientSchedules` - Less than 2 schedules provided
    /// - `UnauthorizedScheduleAccess` - Schedule doesn't belong to caller
    /// - `AssetMismatch` - Schedules have different underlying assets
    /// - `UnlockDateAcceleration` - Merge would artificially accelerate unlock dates
    /// - `ScheduleNotActive` - Schedule already merged or inactive
    pub fn merge_schedules(e: Env, user: Address, schedule_ids: Vec<u32>) -> Result<u32, Error> {
        user.require_auth();

        // Validate input
        if schedule_ids.len() < 2 {
            return Err(Error::InsufficientSchedules);
        }

        let current_time = e.ledger().timestamp();
        let mut total_amount = 0i128;
        let mut total_claimed = 0i128;
        let mut weighted_start_time = 0u64;
        let mut weighted_end_time = 0u64;
        let mut weighted_cliff_duration = 0u64;
        let mut common_asset_address: Option<Address> = None;
        let mut validated_schedules = Vec::new(&e);

        // Validate each schedule and collect data for weighted calculations
        for schedule_id in schedule_ids.iter() {
            // Check if schedule already merged
            if storage::is_schedule_merged(&e, *schedule_id) {
                return Err(Error::ScheduleNotActive);
            }

            // Get schedule data (this would need to be implemented based on actual schedule storage)
            let schedule_data = Self::get_schedule_data(&e, *schedule_id)
                .ok_or(Error::VaultNotFound)?;

            // Verify ownership
            if schedule_data.beneficiary != user {
                return Err(Error::UnauthorizedScheduleAccess);
            }

            // Verify asset consistency
            match &common_asset_address {
                None => common_asset_address = Some(schedule_data.asset_address.clone()),
                Some(asset) => {
                    if *asset != schedule_data.asset_address {
                        return Err(Error::AssetMismatch);
                    }
                }
            }

            // Accumulate data for weighted calculations
            let schedule_remaining = schedule_data.total_amount - schedule_data.claimed_amount;
            total_amount += schedule_remaining;
            total_claimed += schedule_data.claimed_amount;

            // Weighted average calculations based on remaining amounts
            if schedule_remaining > 0 {
                weighted_start_time += schedule_data.start_time * schedule_remaining as u64;
                weighted_end_time += schedule_data.end_time * schedule_remaining as u64;
                weighted_cliff_duration += schedule_data.cliff_duration * schedule_remaining as u64;
            }

            validated_schedules.push_back((*schedule_id, schedule_data));
        }

        if total_amount <= 0 {
            return Err(Error::InvalidInput);
        }

        // Calculate weighted averages
        let avg_start_time = weighted_start_time / total_amount as u64;
        let avg_end_time = weighted_end_time / total_amount as u64;
        let avg_cliff_duration = weighted_cliff_duration / total_amount as u64;

        // Security check: ensure merge doesn't artificially accelerate unlock dates
        for (_schedule_id, schedule_data) in validated_schedules.iter() {
            if avg_end_time < schedule_data.end_time {
                // The new end time would be earlier than the latest original end time
                // This would artificially accelerate unlock dates
                return Err(Error::UnlockDateAcceleration);
            }
        }

        // Create master schedule
        let master_id = storage::get_next_master_schedule_id(&e);
        let master_schedule = MasterSchedule {
            master_id,
            beneficiary: user.clone(),
            asset_address: common_asset_address.unwrap(),
            total_amount,
            claimed_amount: total_claimed,
            start_time: avg_start_time,
            end_time: avg_end_time,
            cliff_duration: avg_cliff_duration,
            merged_schedule_ids: schedule_ids.clone(),
            created_at: current_time,
            is_active: true,
        };

        // Store master schedule
        storage::set_master_schedule(&e, master_id, &master_schedule);

        // Mark original schedules as merged
        for schedule_id in schedule_ids.iter() {
            storage::mark_schedule_merged(&e, *schedule_id);
        }

        // Emit consolidation event
        SchedulesConsolidated {
            beneficiary: user.clone(),
            burned_schedule_ids: schedule_ids.clone(),
            master_schedule_id: master_id,
            total_amount,
            new_end_time: avg_end_time,
            timestamp: current_time,
        }.publish(&e);

        Ok(master_id)
    }

    /// Get master schedule information
    pub fn get_master_schedule(e: Env, master_id: u32) -> Option<MasterSchedule> {
        storage::get_master_schedule(&e, master_id)
    }

    /// Check if a schedule has been merged
    pub fn is_schedule_merged(e: Env, schedule_id: u32) -> bool {
        storage::is_schedule_merged(&e, schedule_id)
    }

    /// Helper function to get schedule data from vault storage
    fn get_schedule_data(e: &Env, schedule_id: u32) -> Option<ScheduleData> {
        // Access the vesting contracts vault storage
        // Note: This assumes the vesting vault has access to vesting contracts storage
        // In a real implementation, this might need to be a cross-contract call
        
        // For now, we'll create a mock implementation that would need to be
        // replaced with actual cross-contract storage access
        let vault_data_key = ("VaultData", schedule_id);
        
        if let Some(vault_bytes) = e.storage().instance().get::<_, Vec<u8>>(&vault_data_key) {
            // This is a simplified implementation - in reality, we'd need to 
            // deserialize the Vault struct from the vesting contracts
            // For now, we'll return a mock schedule data structure
            
            // Mock data - replace with actual vault data extraction
            Some(ScheduleData {
                beneficiary: Address::from_string(&e, &"mock_address".into_val(&e)),
                asset_address: Address::from_string(&e, &"mock_asset".into_val(&e)),
                total_amount: 1000i128,
                claimed_amount: 0i128,
                start_time: e.ledger().timestamp(),
                end_time: e.ledger().timestamp() + 31536000, // 1 year
                cliff_duration: 2592000, // 30 days
            })
        } else {
            None
        }
    }
}

/// Struct for schedule data extracted from vault storage
#[derive(Clone, Debug)]
struct ScheduleData {
    beneficiary: Address,
    asset_address: Address,
    total_amount: i128,
    claimed_amount: i128,
    start_time: u64,
    end_time: u64,
    cliff_duration: u64,
}
