//! Pure, dependency-free arithmetic (unit-tested on the host, see /math-tests).
use crate::constants::*;

/// Lamport cost of buying `amount` base units when `total_sold` are already
/// sold. Straddling purchases pay each tier's price for the portion inside it.
/// Rounds up (in the protocol's favour). `None` = overflow / over allocation.
pub fn presale_cost(total_sold: u64, amount: u64) -> Option<u64> {
    let end = total_sold.checked_add(amount)?;
    if end > PRESALE_ALLOCATION {
        return None;
    }
    let mut numerator: u128 = 0;
    for (i, price) in PRESALE_TIER_PRICES.iter().enumerate() {
        let tier_start = PRESALE_TIER_SIZE * i as u64;
        let tier_end = tier_start + PRESALE_TIER_SIZE;
        let lo = total_sold.max(tier_start);
        let hi = end.min(tier_end);
        if hi > lo {
            numerator += (hi - lo) as u128 * *price as u128;
        }
    }
    u64::try_from(numerator.div_ceil(ONE_TOKEN as u128)).ok()
}

/// Lamport cost of `amount` base units at a flat price per whole token.
pub fn flat_cost(amount: u64, price_lamports_per_token: u64) -> Option<u64> {
    u64::try_from((amount as u128 * price_lamports_per_token as u128).div_ceil(ONE_TOKEN as u128)).ok()
}

/// Amount vested at `now` under 24-month linear (monthly-step) vesting.
/// Based on `total_allocated`, and releases 100% at the end (no dust).
pub fn vested_amount(total_allocated: u64, start: i64, now: i64) -> u64 {
    if now <= start {
        return 0;
    }
    let months = ((now - start) / SECONDS_PER_MONTH) as u64;
    if months >= VESTING_PERIOD_MONTHS {
        return total_allocated;
    }
    (total_allocated as u128 * months as u128 / VESTING_PERIOD_MONTHS as u128) as u64
}
