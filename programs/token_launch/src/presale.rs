use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::LaunchError;
use crate::math;
use crate::token::Config;

#[account]
#[derive(InitSpace)]
pub struct PresaleAccount {
    pub total_sold: u64,
    pub bump: u8,
}

pub fn initialize_presale(ctx: Context<InitializePresale>) -> Result<()> {
    let presale = &mut ctx.accounts.presale_account;
    presale.total_sold = 0;
    presale.bump = ctx.bumps.presale_account;
    Ok(())
}

pub fn buy_tokens(ctx: Context<BuyTokens>, amount: u64, max_cost_lamports: u64) -> Result<()> {
    require!(amount > 0, LaunchError::InvalidAmount);
    let presale = &mut ctx.accounts.presale_account;
    require!(presale.total_sold < PRESALE_ALLOCATION, LaunchError::PresaleEnded);

    let end = presale.total_sold.checked_add(amount).ok_or(LaunchError::MathOverflow)?;
    require!(end <= PRESALE_ALLOCATION, LaunchError::ExceedsPresaleAllocation);
    let cost = math::presale_cost(presale.total_sold, amount).ok_or(LaunchError::MathOverflow)?;
    require!(cost > 0, LaunchError::InvalidAmount);
    require!(cost <= max_cost_lamports, LaunchError::SlippageExceeded);

    // 1. Buyer actually pays (system program fails on insufficient lamports).
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.treasury.to_account_info(),
            },
        ),
        cost,
    )?;

    // 2. Tokens come out of the pre-minted presale vault, signed by the PDA.
    let bump = [ctx.accounts.config.vault_authority_bump];
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &bump];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.presale_vault.to_account_info(),
                to: ctx.accounts.buyer_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    presale.total_sold = presale.total_sold.checked_add(amount).ok_or(LaunchError::MathOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializePresale<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,
    #[account(init, payer = admin, space = 8 + PresaleAccount::INIT_SPACE, seeds = [PRESALE_SEED], bump)]
    pub presale_account: Account<'info, PresaleAccount>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = treasury)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [PRESALE_SEED], bump = presale_account.bump)]
    pub presale_account: Account<'info, PresaleAccount>,
    #[account(mut, seeds = [PRESALE_VAULT_SEED], bump)]
    pub presale_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: PDA signer.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, token::mint = config.mint, token::authority = buyer)]
    pub buyer_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: validated by `has_one = treasury`.
    #[account(mut)]
    pub treasury: UncheckedAccount<'info>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
