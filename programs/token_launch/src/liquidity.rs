use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::LaunchError;
use crate::math;
use crate::token::Config;

#[account]
#[derive(InitSpace)]
pub struct LiquidityState {
    pub total_liquidity: u64,
    pub sold_tokens: u64,
    /// Lamports per whole token, set by the admin (never by the buyer).
    pub price_lamports_per_token: u64,
    pub bump: u8,
}

pub fn initialize_liquidity(ctx: Context<InitializeLiquidity>, price_lamports_per_token: u64) -> Result<()> {
    require!(price_lamports_per_token > 0, LaunchError::InvalidPrice);
    let state = &mut ctx.accounts.state;
    // Backed 1:1 by the pre-minted liquidity vault, not an arbitrary argument.
    state.total_liquidity = ctx.accounts.liquidity_vault.amount;
    state.sold_tokens = 0;
    state.price_lamports_per_token = price_lamports_per_token;
    state.bump = ctx.bumps.state;
    Ok(())
}

/// `max_price` is the buyer's slippage guard (lamports per whole token).
pub fn purchase_tokens(ctx: Context<PurchaseTokens>, amount: u64, max_price: u64) -> Result<()> {
    require!(amount > 0, LaunchError::InvalidAmount);
    let state = &mut ctx.accounts.state;
    require!(state.total_liquidity >= amount, LaunchError::InsufficientLiquidity);
    require!(state.price_lamports_per_token <= max_price, LaunchError::SlippageExceeded);

    let cost = math::flat_cost(amount, state.price_lamports_per_token).ok_or(LaunchError::MathOverflow)?;
    require!(cost > 0, LaunchError::InvalidAmount);

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

    let bump = [ctx.accounts.config.vault_authority_bump];
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &bump];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.liquidity_vault.to_account_info(),
                to: ctx.accounts.buyer_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    state.total_liquidity = state.total_liquidity.checked_sub(amount).ok_or(LaunchError::MathOverflow)?;
    state.sold_tokens = state.sold_tokens.checked_add(amount).ok_or(LaunchError::MathOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeLiquidity<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Box<Account<'info, Config>>,
    #[account(init, payer = admin, space = 8 + LiquidityState::INIT_SPACE, seeds = [LIQUIDITY_SEED], bump)]
    pub state: Account<'info, LiquidityState>,
    #[account(seeds = [LIQUIDITY_VAULT_SEED], bump)]
    pub liquidity_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseTokens<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = treasury)]
    pub config: Box<Account<'info, Config>>,
    // `mut` was missing in the original, so state changes were never persisted.
    #[account(mut, seeds = [LIQUIDITY_SEED], bump = state.bump)]
    pub state: Account<'info, LiquidityState>,
    #[account(mut, seeds = [LIQUIDITY_VAULT_SEED], bump)]
    pub liquidity_vault: Box<Account<'info, TokenAccount>>,
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
