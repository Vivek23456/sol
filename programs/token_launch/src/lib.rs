//! Token launch program: fixed-supply SPL token split into four PDA-controlled
//! vaults (presale, liquidity, development fund, airdrop).
//!
//! One crate = one Anchor `#[program]`. Every instruction is routed through the
//! single program module below; the business logic lives in the per-feature
//! modules (`token`, `presale`, `liquidity`, `dev_fund`, `airdrop`).
use anchor_lang::prelude::*;

declare_id!("2NrBKkt2FUntwsPXvjDdi8ry5FcgYLBSjXYmVrxCBMbf");

pub mod airdrop;
pub mod constants;
pub mod dev_fund;
pub mod errors;
pub mod liquidity;
pub mod math;
pub mod presale;
pub mod token;

pub use airdrop::*;
pub use constants::*;
pub use dev_fund::*;
pub use errors::*;
pub use liquidity::*;
pub use presale::*;
pub use token::*;

#[program]
pub mod token_launch {
    use super::*;

    // ---------------------------------------------------------------- token
    /// Creates the config PDA and the mint (authority = vault-authority PDA).
    pub fn initialize(ctx: Context<Initialize>, total_supply: u64) -> Result<()> {
        token::initialize(ctx, total_supply)
    }

    /// Creates the four allocation vaults, mints the whole supply into them and
    /// permanently revokes the mint authority (fixed supply).
    pub fn initialize_vaults(ctx: Context<InitializeVaults>) -> Result<()> {
        token::initialize_vaults(ctx)
    }

    // -------------------------------------------------------------- presale
    pub fn initialize_presale(ctx: Context<InitializePresale>) -> Result<()> {
        presale::initialize_presale(ctx)
    }

    pub fn buy_tokens(ctx: Context<BuyTokens>, amount: u64, max_cost_lamports: u64) -> Result<()> {
        presale::buy_tokens(ctx, amount, max_cost_lamports)
    }

    // ------------------------------------------------------------ liquidity
    pub fn initialize_liquidity(
        ctx: Context<InitializeLiquidity>,
        price_lamports_per_token: u64,
    ) -> Result<()> {
        liquidity::initialize_liquidity(ctx, price_lamports_per_token)
    }

    pub fn purchase_tokens(ctx: Context<PurchaseTokens>, amount: u64, max_price: u64) -> Result<()> {
        liquidity::purchase_tokens(ctx, amount, max_price)
    }

    // ------------------------------------------------------------- dev fund
    pub fn allocate_tokens(ctx: Context<AllocateTokens>, beneficiary: Pubkey) -> Result<()> {
        dev_fund::allocate_tokens(ctx, beneficiary)
    }

    pub fn release_tokens(ctx: Context<ReleaseTokens>) -> Result<()> {
        dev_fund::release_tokens(ctx)
    }

    // -------------------------------------------------------------- airdrop
    pub fn initialize_airdrop(
        ctx: Context<InitializeAirdrop>,
        amount_per_user: u64,
        start_time: i64,
        duration: i64,
    ) -> Result<()> {
        airdrop::initialize_airdrop(ctx, amount_per_user, start_time, duration)
    }

    pub fn whitelist_user(ctx: Context<WhitelistUser>, user: Pubkey) -> Result<()> {
        airdrop::whitelist_user(ctx, user)
    }

    pub fn claim_airdrop(ctx: Context<ClaimAirdrop>) -> Result<()> {
        airdrop::claim_airdrop(ctx)
    }
}
