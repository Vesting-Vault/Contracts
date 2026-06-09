# Vesting Vault Contracts

Soroban smart contracts for the Vesting-Vault protocol on Stellar. Provides token vesting, governance, staking, inheritance (dead-man's switch), and disaster recovery features.

## Architecture

```
┌─────────────────────────────────────────────┐
│              Admin / Issuer                 │
└────────────┬──────────────────┬─────────────┘
             │                  │
             ▼                  ▼
   ┌──────────────────┐  ┌──────────────────────┐
   │  GrantContract   │  │   VestingContract    │
   │  (single grant)  │  │  (multi-vault pool)  │
   └────────┬─────────┘  └──────────┬───────────┘
            │                       │
            ▼                       ▼
     Beneficiary            Vault[1..N]
     claims linearly        per beneficiary
```

### Contracts

| Contract | Purpose |
|---|---|
| `vesting_contracts` | Multi-vault, admin-controlled vesting manager |
| `grant_contracts` | Single-beneficiary, time-linear vesting |
| `vesting_vault` | Core vault logic with staking, succession, governance |
| `vesting_status_nft` | NFT-based vesting status tracking |
| `staking_contract` | Whitelisted staking contract for auto-stake |
| `deposit_to_yield_adapter` | Yield adapter for deposits |
| `insurance_treasury` | Insurance treasury management |

## Key Features

### Governance (Defensive Governance with Consent Logic)

All major admin actions require a 72-hour challenge period. Beneficiaries holding >51% of total locked tokens can veto proposals.

| Governable Action | Function |
|---|---|
| Admin Rotation | `propose_admin_rotation(new_admin)` |
| Contract Upgrade | `propose_contract_upgrade(new_contract)` |
| Emergency Pause | `propose_emergency_pause(pause_state)` |

### Auto-Stake

Tokens stay locked inside the vault. `auto_stake()` makes a synchronous cross-contract call to a whitelisted staking contract. No token transfer occurs — the staking contract holds only the record.

### Inheritance (Dead-Man's Switch)

If a primary beneficiary is inactive for `switch_duration`, a nominated backup can claim ownership via a two-step process with a challenge window.

### Vault Freeze

Admin can freeze vaults to block claims (primary defense against revocation front-running). Revocation proceeds even on frozen vaults.

### Disaster Recovery

Encrypted backups, RTO validation, and quarterly fire drills for database infrastructure.

## Setup

### Prerequisites

- Rust 1.91.0 (see `rust-toolchain.toml`)
- `wasm32v1-none` target
- Soroban SDK 25.3.1

### Build

```bash
cargo build --target wasm32v1-none --release
```

### Test

```bash
cargo test --workspace
cargo test -p vesting_contracts formal_reentrancy -- --nocapture
```

### Known Build Issues

The codebase currently has compilation errors that need to be resolved before tests can run:

- **`vesting_contracts`**: 669 errors — primarily from ZK verifier test files (`zk_verifier_test.rs`) using incorrect API signatures (`BytesN::from_array` missing `&Env` arg, `ZKVerifierError` missing `Debug` derive, mismatched types)
- **`vesting_vault`**: 75 errors — including `Vec<u8>` type inference issues and struct-in-impl-block errors (partially fixed)
- **`insurance_treasury`**: Missing `vec!` macro import (fixed)

These are API-mismatch issues from Soroban SDK version changes and incomplete test code. Resolution requires updating all test files to match the current `soroban-sdk 25.3.1` API.

## Project Structure

```
├── contracts/
│   ├── vesting_contracts/    # Multi-vault vesting manager
│   ├── grant_contracts/      # Single grant vesting
│   ├── vesting_vault/        # Core vault logic
│   ├── vesting_status_nft/   # NFT status tracking
│   ├── staking_contract/     # Staking contract
│   ├── deposit_to_yield_adapter/ # Yield adapter
│   └── insurance_treasury/   # Insurance treasury
├── analytics/                # Revenue prediction engine
├── scripts/                  # Backup & recovery scripts
├── social/                   # WebSocket implementation
├── doc_tests/                # Verification summaries
├── SECURITY.md               # Security documentation & front-running analysis
├── SPEC.md                   # Technical specification
├── INVARIANTS.md             # Mathematical invariants for auditors
└── PULL_REQUEST_TEMPLATE.md  # PR template
```

## Deployed Contract

- **Network:** Stellar Testnet
- **Contract ID:** `CD6OGC46OFCV52IJQKEDVKLX5ASA3ZMSTHAAZQIPDSJV6VZ3KUJDEP4D`

## Gas Costs

| Operation | Estimated Cost (XLM) |
|---|---|
| Create Vault | ~0.05 |
| Claim | ~0.01 |
| Propose Governance Action | ~0.02 |
| Vote on Proposal | ~0.01 |
| Execute Proposal | ~0.02 |

*Actual costs vary by network conditions and operation parameters.*

## Security

See [SECURITY.md](SECURITY.md) for detailed analysis of revocation front-running, operational security procedures, and the vault freeze mechanism.

See [INVARIANTS.md](INVARIANTS.md) for the full mathematical invariant specification used by security auditors.

## Technical Specification

See [SPEC.md](SPEC.md) for the complete technical specification covering storage layouts, vesting formulas, state machines, and error codes.

## License

Proprietary. All rights reserved.
