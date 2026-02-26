# Periodic Vesting Feature

This document describes the periodic vesting steps feature implemented in Issue #14.

## Overview

The vesting contract now supports both linear and periodic vesting schedules:

- **Linear Vesting** (`step_duration = 0`): Tokens vest continuously over time
- **Periodic Vesting** (`step_duration > 0`): Tokens vest in discrete steps (e.g., monthly)

## Key Features

### Step Function Vesting
When `step_duration > 0`, the contract uses a step function that rounds down to the nearest completed step. This ensures users only receive tokens that have fully vested according to the step schedule.

### Formula
For periodic vesting, the calculation is:
```
vested = (elapsed / step_duration) * (total_amount / total_steps) * step_duration
```

This simplifies to:
```
vested = completed_steps * amount_per_step
```

### Common Time Durations
The contract provides helper functions for common vesting periods:

```rust
// Monthly vesting (30 days)
let monthly_duration = VestingContract::monthly(); // 2,592,000 seconds

// Quarterly vesting (90 days)  
let quarterly_duration = VestingContract::quarterly(); // 7,776,000 seconds

// Yearly vesting (365 days)
let yearly_duration = VestingContract::yearly(); // 31,536,000 seconds
```

## Usage Examples

### Creating a Monthly Vesting Vault
```rust
let vault_id = client.create_vault_full(
    &beneficiary,
    &12000i128,                    // 12,000 tokens total
    &start_time,
    &end_time,                     // 12 months later
    &0i128,                       // no keeper fee
    &false,                        // revocable
    &true,                         // transferable
    &VestingContract::monthly()     // monthly step duration
);
```

### Creating a Quarterly Vesting Vault
```rust
let vault_id = client.create_vault_full(
    &beneficiary,
    &4000i128,                     // 4,000 tokens total (1,000 per quarter)
    &start_time,
    &end_time,                     // 1 year later
    &0i128,
    &false,
    &true,
    &VestingContract::quarterly()  // quarterly step duration
);
```

## Behavior

### Rounding Down
Periodic vesting rounds down to the nearest completed step:
- After 1.5 months with monthly steps: Only 1 month worth vests
- After 2.9 months with monthly steps: Only 2 months worth vests
- After exactly 3 months with monthly steps: 3 months worth vests

### Linear vs Periodic Comparison
| Time Elapsed | Linear Vesting | Monthly Periodic |
|-------------|----------------|------------------|
| 1 month     | 1,000 tokens   | 1,000 tokens    |
| 1.5 months  | 1,500 tokens   | 1,000 tokens    |
| 2 months     | 2,000 tokens   | 2,000 tokens    |
| 2.5 months  | 2,500 tokens   | 2,000 tokens    |
| 3 months     | 3,000 tokens   | 3,000 tokens    |

## Acceptance Criteria

✅ **[x] If step_duration > 0, calculate vested = (elapsed / step_duration) * rate * step_duration**

✅ **[x] Round down to the nearest month**

## Testing

Comprehensive tests are included in `test.rs`:

- `test_monthly_vesting_step_function()`: Tests monthly vesting with step function behavior
- `test_quarterly_vesting()`: Tests quarterly vesting
- `test_linear_vs_periodic_vesting()`: Compares linear vs periodic vesting
- `test_periodic_vesting_edge_cases()`: Tests edge cases

## Implementation Details

### Vault Structure
The `Vault` struct includes a `step_duration` field:
```rust
pub struct Vault {
    // ... other fields
    pub step_duration: u64, // Duration of each vesting step in seconds (0 = linear)
    // ... other fields
}
```

### Calculation Function
The `calculate_time_vested_amount` function handles both linear and periodic vesting:
- If `step_duration == 0`: Uses linear vesting formula
- If `step_duration > 0`: Uses periodic vesting with step function

### Backward Compatibility
- Existing vaults with `step_duration = 0` continue to use linear vesting
- New vaults can specify any positive `step_duration` for periodic vesting
- All existing functionality remains unchanged

## Benefits

1. **Predictable Vesting**: Users know exactly when tokens will vest
2. **Accounting Simplicity**: Easy to track vesting in discrete periods
3. **Compliance**: Meets regulatory requirements for periodic vesting
4. **Flexibility**: Supports any step duration (monthly, quarterly, yearly, custom)
