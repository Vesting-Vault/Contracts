# Technical Specification: Vesting & Grant Contracts

**Version:** 1.0.0  
**Target Audience:** Security Auditors, Protocol Engineers  
**Network:** Stellar / Soroban Smart Contracts  
**Language:** Rust (`#![no_std]`, `soroban-sdk`)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Contract: GrantContract](#contract-grantcontract)
   - [Storage Layout](#grant-storage-layout)
   - [Vesting Formula](#vesting-formula)
   - [State Machine](#grant-state-machine)
   - [Functions](#grant-functions)
4. [Contract: VestingContract](#contract-vestingcontract)
   - [Storage Layout](#vesting-storage-layout)
   - [Vault Lifecycle](#vault-lifecycle)
   - [State Machine](#vesting-state-machine)
   - [Functions](#vesting-functions)
5. [Security Model](#security-model)
6. [Invariants](#invariants)
7. [Error Codes & Panic Conditions](#error-codes--panic-conditions)
8. [Known Limitations & Auditor Notes](#known-limitations--auditor-notes)

---

## Overview

This system consists of two Soroban smart contracts deployed on Stellar:

- **`GrantContract`** — A single-beneficiary, time-linear vesting contract. It accepts a total token amount and a duration, then exposes a claimable balance that grows linearly from `start_time` to `end_time`.
- **`VestingContract`** — A multi-vault, admin-controlled vesting manager. An admin allocates tokens into discrete vaults for multiple beneficiaries, with support for lazy or full initialization, batch creation, revocation, and beneficiary transfer.

The two contracts are architecturally independent but conceptually complementary: `GrantContract` models a single grant issuance, while `VestingContract` manages an entire fleet of grants from a shared supply.

---

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

---

## Contract: GrantContract

### Grant Storage Layout

All values are stored in `instance` storage (tied to contract lifetime).

| Key Symbol  | Type  | Description                              |
|-------------|-------|------------------------------------------|
| `TOTAL`     | U256  | Total tokens allocated to the grant      |
| `START`     | u64   | Unix timestamp when vesting begins       |
| `END`       | u64   | Unix timestamp when vesting completes    |
| `RECIPIENT` | Address | The sole beneficiary of this grant     |
| `CLAIMED`   | U256  | Cumulative amount already claimed        |

### Vesting Formula

The claimable balance at any point in time is computed as follows:

```
Let:
  T   = total_amount
  t0  = start_time
  t1  = end_time
  tn  = current ledger timestamp
  C   = claimed (cumulative)

if tn <= t0:
    claimable = 0

elif tn >= t1:
    elapsed = t1 - t0          # capped at full duration
else:
    elapsed = tn - t0

vested   = T * elapsed / (t1 - t0)
claimable = max(vested - C, 0)
```

**Key properties:**
- Vesting is strictly **linear** — no cliff, no step function.
- The formula uses `U256` arithmetic throughout to prevent overflow on large token amounts.
- Integer division truncates (floors), so claimable values may be up to `1` token less than the theoretical continuous value. Tests confirm this tolerance explicitly.
- Once `tn >= t1`, `elapsed` is frozen at `t1 - t0`, so the claimable balance never exceeds `total_amount`.

#### Numerical Example

```
total_amount = 1,000,000 tokens
duration     = 100 seconds
start_time   = 1000 (unix)

At t=1050 (halfway):
  elapsed  = 50
  vested   = 1,000,000 * 50 / 100 = 500,000
  claimed  = 0
  claimable = 500,000

At t=1100 (end):
  elapsed  = 100 (capped)
  vested   = 1,000,000
  claimable = 1,000,000 - claimed
```

### Grant State Machine

```
         initialize_grant()
               │
               ▼
    ┌─────────────────────┐
    │     INITIALIZED     │◄──────────────────────────┐
    │  claimable = 0      │                           │
    │  (tn <= start_time) │                           │
    └────────┬────────────┘                           │
             │ time advances past start_time          │
             ▼                                        │
    ┌─────────────────────┐                           │
    │      VESTING        │──── claim() ─────────────►│
    │  0 < claimable < T  │   (updates CLAIMED,       │
    │  (t0 < tn < t1)     │    resets claimable to 0) │
    └────────┬────────────┘                           │
             │ time advances past end_time            │
             ▼                                        │
    ┌─────────────────────┐                           │
    │   FULLY VESTED      │──── claim() ─────────────►┘
    │  claimable = T - C  │
    │  (tn >= t1)         │
    └─────────────────────┘
             │ all tokens claimed
             ▼
    ┌─────────────────────┐
    │   EXHAUSTED         │
    │  claimable = 0      │
    │  claimed = T        │
    └─────────────────────┘
```

### Grant Functions

#### `initialize_grant(recipient, total_amount, duration_seconds) → u64`
- Sets all storage keys.
- `start_time` = current ledger timestamp at time of call.
- `end_time` = `start_time + duration_seconds`.
- Returns `end_time`.
- **No re-initialization guard exists.** Calling this a second time will overwrite the existing grant. Auditors should verify this is acceptable in the deployment model.

#### `claimable_balance() → U256`
- Pure read — does not mutate state.
- Returns the currently claimable (unvested minus already-claimed) balance.

#### `claim(recipient) → U256`
- Requires `recipient.require_auth()`.
- Asserts `recipient == stored RECIPIENT` — panics otherwise.
- Asserts `claimable > 0` — panics if nothing to claim.
- Increments `CLAIMED` by the claimable amount.
- Returns the claimed amount.
- **Does not perform actual token transfer** — the contract records accounting only. Token disbursement is expected to be handled externally.

#### `get_grant_info() → (U256, u64, u64, U256)`
- Returns `(total_amount, start_time, end_time, claimed)`.
- Pure read.

---

## Contract: VestingContract

### Vesting Storage Layout

All values stored in `instance` storage.

| Key Symbol      | Type           | Description                                      |
|-----------------|----------------|--------------------------------------------------|
| `VAULT_COUNT`   | u64            | Total number of vaults created (monotonic)       |
| `VAULT_DATA`    | Vault (struct) | Keyed by vault_id (u64); stores per-vault state  |
| `USER_VAULTS`   | Vec\<u64\>     | Keyed by Address; lists vault IDs per user       |
| `INITIAL_SUPPLY`| i128           | The total token supply set at initialization     |
| `ADMIN_BALANCE` | i128           | Tokens not yet allocated to any vault            |
| `ADMIN_ADDRESS` | Address        | Current admin                                    |
| `PROPOSED_ADMIN`| Address        | Pending admin from two-step transfer (optional)  |

#### Vault Struct

```rust
pub struct Vault {
    pub owner: Address,          // Current beneficiary
    pub total_amount: i128,      // Total tokens in this vault
    pub released_amount: i128,   // Tokens already claimed or revoked
    pub start_time: u64,         // Vesting start (unix timestamp)
    pub end_time: u64,           // Vesting end (unix timestamp)
    pub is_initialized: bool,    // Lazy init flag
}
```

> **Note for auditors:** The `VestingContract` does not compute a vested amount internally. It tracks `total_amount` and `released_amount` only. The actual time-based vesting calculation — and any enforcement of `start_time`/`end_time` at claim time — is **not present** in `claim_tokens()`. Any caller can claim any unreleased amount regardless of the current time. This is a significant design note detailed further in [Known Limitations](#known-limitations--auditor-notes).

### Vault Lifecycle

#### Initialization Modes

| Mode            | `is_initialized` at creation | `USER_VAULTS` updated at creation |
|-----------------|-------------------------------|-------------------------------------|
| `create_vault_full`    | `true`               | Yes                                 |
| `create_vault_lazy`    | `false`              | No (deferred)                       |
| `batch_create_vaults_full` | `true`           | Yes (per vault)                     |
| `batch_create_vaults_lazy` | `false`          | No (deferred)                       |

Lazy vaults have their `USER_VAULTS` index populated on first access via `initialize_vault_metadata()`, `get_vault()`, or `get_user_vaults()`.

### Vesting State Machine

```
          create_vault_full() / create_vault_lazy()
                        │
                        ▼
             ┌────────────────────┐
             │      CREATED       │
             │  (is_initialized   │
             │   = true or false) │
             └──────┬─────────────┘
                    │
       ┌────────────┴─────────────────────┐
       │ lazy                             │ full
       ▼                                  ▼
┌────────────────┐               ┌─────────────────┐
│  LAZY (index   │               │  ACTIVE (index   │
│  not written)  │               │  written to      │
│                │               │  USER_VAULTS)    │
└───────┬────────┘               └────────┬─────────┘
        │ initialize_vault_metadata()      │
        │ / get_vault() / get_user_vaults()│
        └─────────────┬────────────────────┘
                      ▼
             ┌─────────────────┐
             │    ACTIVE       │◄──────── transfer_beneficiary()
             │  (fully indexed)│         (updates USER_VAULTS)
             └──────┬──────────┘
                    │
         ┌──────────┴──────────────────┐
         │ claim_tokens()              │ revoke_tokens()
         ▼                             ▼
 ┌───────────────┐            ┌──────────────────────┐
 │   PARTIALLY   │            │      REVOKED         │
 │   CLAIMED     │            │  released_amount      │
 │               │            │  = total_amount       │
 └───────┬───────┘            └──────────────────────┘
         │ all tokens claimed
         ▼
 ┌───────────────┐
 │   EXHAUSTED   │
 │  released =   │
 │  total_amount │
 └───────────────┘
```

### Vesting Functions

#### `initialize(admin, initial_supply)`
- Sets `INITIAL_SUPPLY`, `ADMIN_BALANCE` (= `initial_supply`), `ADMIN_ADDRESS`, and `VAULT_COUNT = 0`.
- No re-initialization guard. Calling again resets all balances.

#### `propose_new_admin(new_admin)`
- Admin-only (see [Security Model](#security-model)).
- Writes `new_admin` to `PROPOSED_ADMIN`.

#### `accept_ownership()`
- Caller must match `PROPOSED_ADMIN`.
- Moves `PROPOSED_ADMIN` → `ADMIN_ADDRESS`, clears `PROPOSED_ADMIN`.

#### `get_admin() → Address` / `get_proposed_admin() → Option<Address>`
- Pure reads.

#### `create_vault_full(owner, amount, start_time, end_time) → u64`
- Admin-only.
- Deducts `amount` from `ADMIN_BALANCE`. Panics if insufficient.
- Writes full vault struct with `is_initialized = true`.
- Updates `USER_VAULTS[owner]`.
- Emits `VaultCreated` event.
- Returns new `vault_id`.

#### `create_vault_lazy(owner, amount, start_time, end_time) → u64`
- Admin-only.
- Same as above but sets `is_initialized = false` and skips `USER_VAULTS` write.
- Lower storage cost at creation time.

#### `initialize_vault_metadata(vault_id) → bool`
- Public (no auth required).
- If vault is lazy (`is_initialized = false`), sets it to `true` and writes to `USER_VAULTS`.
- Returns `true` if initialization occurred, `false` if already initialized.

#### `claim_tokens(vault_id, claim_amount) → i128`
- No auth check — **any caller can invoke this function**.
- Requires `is_initialized == true`.
- Requires `claim_amount > 0`.
- Requires `claim_amount <= (total_amount - released_amount)`.
- Increments `released_amount`. Returns `claim_amount`.
- **Does not verify time-based vesting schedule** — see Known Limitations.

#### `transfer_beneficiary(vault_id, new_address)`
- Admin-only.
- Updates `vault.owner`.
- If `is_initialized`: removes `vault_id` from old owner's `USER_VAULTS`, adds to new owner's.
- If lazy: skips index update (index will be correct when initialized later).
- Emits `BeneficiaryChanged` event.

#### `batch_create_vaults_lazy(batch_data) → Vec<u64>`
- Admin-only.
- Validates total batch amount against `ADMIN_BALANCE` in a single check upfront.
- Creates all vaults lazily in a loop. Updates `VAULT_COUNT` once at the end.

#### `batch_create_vaults_full(batch_data) → Vec<u64>`
- Same as above but with full initialization per vault (writes `USER_VAULTS` per vault).

#### `revoke_tokens(vault_id) → i128`
- Admin-only.
- Computes `unreleased = total_amount - released_amount`.
- Sets `released_amount = total_amount` (marks vault as fully released).
- Returns `unreleased` to `ADMIN_BALANCE`.
- Emits `TokensRevoked` event.
- Panics if `unreleased == 0` (already exhausted or revoked).

#### `get_vault(vault_id) → Vault`
- Auto-initializes lazy vaults on read.

#### `get_user_vaults(user) → Vec<u64>`
- Returns vault ID list for user. Auto-initializes any lazy vaults found.

#### `get_contract_state() → (i128, i128, i128)`
- Returns `(total_locked, total_claimed, admin_balance)` across all vaults.

#### `check_invariant() → bool`
- Returns whether `total_locked + total_claimed + admin_balance == initial_supply`.

---

## Security Model

### Admin Authentication (`require_admin`)

```rust
fn require_admin(env: &Env) {
    let admin = env.storage().instance().get(&ADMIN_ADDRESS)...;
    let caller = env.current_contract_address();
    require!(caller == admin, "Caller is not admin");
}
```

> **Critical Auditor Note:** The admin check compares `env.current_contract_address()` against the stored admin. `current_contract_address()` returns the address of the *contract itself*, **not the transaction invoker**. This means `require_admin` as implemented will **always fail in practice** unless the contract is calling itself (e.g., via cross-contract invocation). This pattern does not protect against unauthorized external callers in the way a traditional `require_auth()` check would. This is a **high-severity finding** that should be reviewed before mainnet deployment.

### Two-Step Admin Transfer

The admin handover uses a propose-then-accept pattern to prevent accidental or malicious transfers to wrong addresses:

```
Admin calls propose_new_admin(X)  →  PROPOSED_ADMIN = X
X calls accept_ownership()        →  ADMIN_ADDRESS = X, PROPOSED_ADMIN cleared
```

This prevents the admin role from being transferred to an address that cannot sign transactions.

### `claim_tokens` — No Authorization

`claim_tokens` performs no `require_auth()` check and no time-based vesting check. Any address can call it for any vault. The only enforced constraint is that `claim_amount ≤ unreleased`. Combined with the broken `require_admin` check, this means the VestingContract's token accounting can be manipulated by any external actor.

### `GrantContract.claim` — Authorization

`claim()` correctly calls `recipient.require_auth()` and verifies the caller matches the stored recipient. This is the correctly implemented auth pattern that should be replicated in `VestingContract`.

---

## Invariants

The `VestingContract` defines and exposes a global balance invariant:

```
INVARIANT: total_locked + total_claimed + admin_balance == initial_supply

Where:
  total_locked  = Σ (vault.total_amount - vault.released_amount) for all vaults
  total_claimed = Σ vault.released_amount for all vaults
  admin_balance = ADMIN_BALANCE
```

This invariant holds under all valid state transitions:

| Operation                     | Effect on invariant components                            |
|-------------------------------|----------------------------------------------------------|
| `create_vault_full/lazy`      | `admin_balance -= amount`, `total_locked += amount`       |
| `claim_tokens(id, x)`         | `total_locked -= x`, `total_claimed += x`                 |
| `revoke_tokens(id)`           | `total_locked -= unreleased`, `admin_balance += unreleased`|
| `batch_create_vaults_*`       | Same as single create, repeated                           |
| `transfer_beneficiary`        | No token amounts change; invariant unaffected             |
| `initialize_vault_metadata`   | No token amounts change; invariant unaffected             |

The invariant can be verified on-chain by calling `check_invariant()`.

---

## Error Codes & Panic Conditions

Soroban contracts do not use typed error enums in this codebase. All errors are runtime panics with string messages. The following table documents all reachable panic conditions:

### VestingContract Panics

| Function                       | Condition                                      | Panic Message                              |
|-------------------------------|------------------------------------------------|--------------------------------------------|
| `require_admin`               | Stored admin not set                           | `"Admin not set"`                          |
| `require_admin`               | Caller is not admin                            | `"Caller is not admin"`                    |
| `propose_new_admin`           | Caller is not admin                            | (via `require_admin`)                      |
| `accept_ownership`            | No proposed admin in storage                   | `"No proposed admin found"`                |
| `accept_ownership`            | Caller is not the proposed admin               | `"Caller is not the proposed admin"`       |
| `create_vault_full`           | `admin_balance < amount`                       | `"Insufficient admin balance"`             |
| `create_vault_lazy`           | `admin_balance < amount`                       | `"Insufficient admin balance"`             |
| `batch_create_vaults_lazy`    | `admin_balance < sum(amounts)`                 | `"Insufficient admin balance for batch"`   |
| `batch_create_vaults_full`    | `admin_balance < sum(amounts)`                 | `"Insufficient admin balance for batch"`   |
| `claim_tokens`                | Vault not found in storage                     | `"Vault not found"`                        |
| `claim_tokens`                | `vault.is_initialized == false`                | `"Vault not initialized"`                  |
| `claim_tokens`                | `claim_amount <= 0`                            | `"Claim amount must be positive"`          |
| `claim_tokens`                | `claim_amount > unreleased`                    | `"Insufficient tokens to claim"`           |
| `transfer_beneficiary`        | Vault not found in storage                     | `"Vault not found"`                        |
| `revoke_tokens`               | Vault not found in storage                     | `"Vault not found"`                        |
| `revoke_tokens`               | `unreleased_amount == 0`                       | `"No tokens available to revoke"`          |

### GrantContract Panics

| Function            | Condition                                     | Panic Message / Assert              |
|--------------------|-----------------------------------------------|-------------------------------------|
| `claim`            | `recipient != stored RECIPIENT`               | `"Unauthorized recipient"`          |
| `claim`            | `claimable == 0`                              | `"No tokens to claim"`              |
| `get_grant_info`   | Storage key missing (first access)            | Unwrap panic (no message)           |

### Implicit Panics (SDK Unwrap)

Several functions call `.unwrap()` on storage reads without a fallback. These will panic if the contract is queried before `initialize` / `initialize_grant` is called:

- `get_admin()` — panics if `ADMIN_ADDRESS` not set
- `claim()` in `GrantContract` — panics if `RECIPIENT` not set

---

## Known Limitations & Auditor Notes

### 1. `require_admin` Uses Wrong Caller Identity
As noted above, `env.current_contract_address()` is the contract's own address, not the transaction signer. All admin-gated functions in `VestingContract` are therefore **unprotected** in practice. The correct pattern is `admin.require_auth()`.

### 2. `claim_tokens` Has No Time-Based Guard
The `VestingContract` stores `start_time` and `end_time` on vaults but never checks them during `claim_tokens()`. A beneficiary (or any caller) can claim all tokens the moment the vault is created. The time parameters are currently only cosmetic / event metadata.

### 3. `claim_tokens` Has No Caller Authorization
There is no `owner.require_auth()` or equivalent. Any address can call `claim_tokens(vault_id, x)` and drain a vault's accounting balance.

### 4. No Re-Initialization Guard on Either Contract
Both `initialize()` and `initialize_grant()` will overwrite existing state if called again. This can be used to reset `ADMIN_BALANCE` or `CLAIMED` to arbitrary values.

### 5. Token Transfers Are Accounting Only
Neither contract integrates with a Soroban token contract (`token::Client`). All `claim`, `revoke`, and `initialize` operations update internal accounting only. The actual movement of tokens to/from beneficiaries is not implemented.

### 6. Lazy Vault `initialize_vault_metadata` Is Unpermissioned
Any external caller can call `initialize_vault_metadata(vault_id)` on any lazy vault, triggering the `USER_VAULTS` index write. While not directly harmful to token balances, it may have unintended gas/storage side effects at scale.

### 7. `get_vault` Mutates State
`get_vault()` is named like a view function but calls `initialize_vault_metadata()` which writes to storage. Auditors and integrators should treat it as a state-mutating call.

### 8. Integer Precision
`GrantContract` uses `U256` for token arithmetic (safe for all realistic token amounts). `VestingContract` uses `i128` (max ~1.7 × 10³⁸), which is sufficient but auditors should verify no negative values are introduced via unexpected call ordering.