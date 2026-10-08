//! Tokenomics. All token amounts are in base units (10^DECIMALS per token).
//!
//! The original code used 9 decimals, but u64::MAX ≈ 1.84e19 base units, i.e.
//! only ~18.4 billion whole tokens at 9 decimals. The allocations below need
//! 350+ billion tokens, so 6 decimals are used (u64 then fits ~18.4 trillion).

pub const DECIMALS: u8 = 6;
pub const ONE_TOKEN: u64 = 10u64.pow(DECIMALS as u32);

pub const PRESALE_TIER_SIZE: u64 = 50_000_000_000 * ONE_TOKEN;
pub const PRESALE_TIERS: usize = 3;
pub const PRESALE_ALLOCATION: u64 = PRESALE_TIER_SIZE * PRESALE_TIERS as u64; // 150B
/// Price per whole token, in lamports: 0.0001 / 0.0002 / 0.0003 SOL.
pub const PRESALE_TIER_PRICES: [u64; PRESALE_TIERS] = [100_000, 200_000, 300_000];

pub const LIQUIDITY_ALLOCATION: u64 = 100_000_000_000 * ONE_TOKEN; // 100B
pub const DEV_FUND_ALLOCATION: u64 = 100_000_000_000 * ONE_TOKEN; // 100B

/// Everything left over after presale + liquidity + dev fund goes to the airdrop.
pub const FIXED_ALLOCATIONS: u64 = PRESALE_ALLOCATION + LIQUIDITY_ALLOCATION + DEV_FUND_ALLOCATION;

pub const VESTING_PERIOD_MONTHS: u64 = 24;
/// Average Gregorian month (365.2425 days / 12).
pub const SECONDS_PER_MONTH: i64 = 2_629_746;

// PDA seeds
pub const CONFIG_SEED: &[u8] = b"config";
pub const MINT_SEED: &[u8] = b"mint";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";
pub const PRESALE_VAULT_SEED: &[u8] = b"presale_vault";
pub const LIQUIDITY_VAULT_SEED: &[u8] = b"liquidity_vault";
pub const DEV_VAULT_SEED: &[u8] = b"dev_vault";
pub const AIRDROP_VAULT_SEED: &[u8] = b"airdrop_vault";
pub const PRESALE_SEED: &[u8] = b"presale";
pub const LIQUIDITY_SEED: &[u8] = b"liquidity";
pub const DEV_FUND_SEED: &[u8] = b"dev_fund";
pub const AIRDROP_SEED: &[u8] = b"airdrop";
pub const WHITELIST_SEED: &[u8] = b"whitelist";
