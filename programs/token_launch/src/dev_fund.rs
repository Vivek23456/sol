use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::LaunchError;
use crate::math;
use crate::token::Config;

#[account]
#[derive(InitSpace)]
pub struct FundAccount {
    pub beneficiary: Pubkey,
    pub total_allocated: u64,
    pub total_released: u64,
    pub vesting_start_time: i64,
    pub bump: u8,
}

pub fn allocate_tokens(ctx: Context<AllocateTokens>, beneficiary: Pubkey) -> Result<()> {
    let total = ctx.accounts.dev_vault.amount;
    require!(total > 0, LaunchError::InvalidAmount);

    let fund = &mut ctx.accounts.fund_account;
    fund.beneficiary = beneficiary;
    fund.total_allocated = total;
    fund.total_released = 0;
    fund.vesting_start_time = Clock::get()?.unix_timestamp;
    fund.bump = ctx.bumps.fund_account;
    Ok(())
}

/// Permissionless crank: tokens can only ever go to the stored beneficiary.
pub fn release_tokens(ctx: Context<ReleaseTokens>) -> Result<()> {
    let fund = &mut ctx.accounts.fund_account;
    let now = Clock::get()?.unix_timestamp;
    let vested = math::vested_amount(fund.total_allocated, fund.vesting_start_time, now);
    require!(vested > fund.total_released, LaunchError::NoTokensAvailable);
    let amount = vested - fund.total_released;

    let bump = [ctx.accounts.config.vault_authority_bump];
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &bump];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.dev_vault.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    fund.total_released = fund.total_released.checked_add(amount).ok_or(LaunchError::MathOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct AllocateTokens<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Box<Account<'info, Config>>,
    #[account(init, payer = admin, space = 8 + FundAccount::INIT_SPACE, seeds = [DEV_FUND_SEED], bump)]
    pub fund_account: Account<'info, FundAccount>,
    #[account(seeds = [DEV_VAULT_SEED], bump)]
    pub dev_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReleaseTokens<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [DEV_FUND_SEED], bump = fund_account.bump)]
    pub fund_account: Account<'info, FundAccount>,
    #[account(mut, seeds = [DEV_VAULT_SEED], bump)]
    pub dev_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: PDA signer.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, token::mint = config.mint, token::authority = fund_account.beneficiary)]
    pub recipient_token_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}
