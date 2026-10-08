#[path = "../../programs/token_launch/src/constants.rs"]
pub mod constants;
#[path = "../../programs/token_launch/src/math.rs"]
pub mod math;

#[cfg(test)]
mod tests {
    use crate::constants::*;
    use crate::math::*;

    const SOL: u64 = 1_000_000_000;
    fn tokens(n: u64) -> u64 { n * ONE_TOKEN }

    #[test]
    fn supply_fits_in_u64() {
        assert!((FIXED_ALLOCATIONS as u128) < u64::MAX as u128);
        // With the original 9 decimals, 50B tokens alone overflow u64:
        assert!(50_000_000_000u128 * 1_000_000_000u128 > u64::MAX as u128);
    }

    #[test]
    fn tier_prices() {
        assert_eq!(presale_cost(0, tokens(1)), Some(100_000)); // 0.0001 SOL
        assert_eq!(presale_cost(PRESALE_TIER_SIZE, tokens(1)), Some(200_000));
        assert_eq!(presale_cost(2 * PRESALE_TIER_SIZE, tokens(1)), Some(300_000));
        assert_eq!(presale_cost(0, tokens(10_000)), Some(1 * SOL));
    }

    #[test]
    fn straddling_purchase_pays_both_tiers() {
        // 10 tokens before the boundary at tier-1 price, 10 after at tier-2 price.
        let cost = presale_cost(PRESALE_TIER_SIZE - tokens(10), tokens(20)).unwrap();
        assert_eq!(cost, 10 * 100_000 + 10 * 200_000);
        // Buying the whole presale in one tx costs the full 3-tier price, not 150B * tier-1.
        let all = presale_cost(0, PRESALE_ALLOCATION).unwrap();
        assert_eq!(all, 50_000_000_000 * (100_000 + 200_000 + 300_000));
        assert_eq!(all, 30_000_000 * SOL); // 5M + 10M + 15M SOL
    }

    #[test]
    fn cannot_exceed_presale_cap() {
        assert_eq!(presale_cost(PRESALE_ALLOCATION, 1), None);
        assert_eq!(presale_cost(PRESALE_ALLOCATION - 5, 6), None);
        assert_eq!(presale_cost(u64::MAX, 1), None);
        assert!(presale_cost(PRESALE_ALLOCATION - 5, 5).is_some());
    }

    #[test]
    fn dust_rounds_up_not_free() {
        // 1 base unit = 0.000001 token -> 0.1 lamport, rounds up to 1 (never free).
        assert_eq!(presale_cost(0, 1), Some(1));
        assert_eq!(flat_cost(1, 100_000), Some(1));
        assert_eq!(flat_cost(tokens(3), 250_000), Some(750_000));
    }

    #[test]
    fn vesting_schedule() {
        let total = DEV_FUND_ALLOCATION;
        let s = 1_700_000_000i64;
        assert_eq!(vested_amount(total, s, s - 100), 0); // clock before start
        assert_eq!(vested_amount(total, s, s), 0);
        assert_eq!(vested_amount(total, s, s + SECONDS_PER_MONTH - 1), 0);
        assert_eq!(vested_amount(total, s, s + SECONDS_PER_MONTH), total / 24);
        assert_eq!(vested_amount(total, s, s + 12 * SECONDS_PER_MONTH), total / 2);
        assert_eq!(vested_amount(total, s, s + 24 * SECONDS_PER_MONTH), total); // no dust stranded
        assert_eq!(vested_amount(total, s, s + 100 * SECONDS_PER_MONTH), total); // capped
        // Original bug: dividing by seconds-per-YEAR meant month 1 only after 365 days.
        assert!(vested_amount(total, s, s + 31 * 86_400) > 0);
    }

    #[test]
    fn vesting_monotonic_and_bounded() {
        let total = 100_000_000_000 * ONE_TOKEN + 7; // awkward number
        let s = 0i64;
        let mut prev = 0;
        for m in 0..=30 {
            let v = vested_amount(total, s, m * SECONDS_PER_MONTH);
            assert!(v >= prev && v <= total);
            prev = v;
        }
        assert_eq!(prev, total);
    }
}
