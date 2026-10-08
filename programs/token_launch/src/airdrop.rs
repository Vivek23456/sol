use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::LaunchError;
use crate::token::Config;

/// Airdrop campaign. The unbounded `HashSet<Pubkey>` of the original is
/// replaced by one small `WhitelistEntry` PDA per user (O(1) lookups, no
/// 10 KB account-size limit, and a per-user `claimed` flag).
#[account]
#[derive(InitSpace)]
pub struct AirdropAccount {
    pub start_time: i64,
    pub end_time: i64,
    pub total_tokens: u64,
    /// Fixed share per user, so early/late whitelisting cannot change payouts.
    pub amount_per_user: u64,
    pub whitelisted_count: u64,
    /// Tokens already sent out (sum over all claims).
    pub distributed_tokens: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct WhitelistEntry {
    pub user: Pubkey,
    pub claimed: bool,
    pub bump: u8,
}

pub fn initialize_airdrop(
    ctx: Context<InitializeAirdrop>,
    amount_per_user: u64,
    start_time: i64,
    duration: i64,
) -> Result<()> {
    require!(amount_per_user > 0, LaunchError::InvalidAmount);
    require!(duration > 0, LaunchError::InvalidTimeWindow);
    let end_time = start_time.checked_add(duration).ok_or(LaunchError::MathOverflow)?;

    let airdrop = &mut ctx.accounts.airdrop_account;
    airdrop.start_time = start_time;
    airdrop.end_time = end_time;
    airdrop.total_tokens = ctx.accounts.airdrop_vault.amount; // backed by real tokens
    require!(amount_per_user <= airdrop.total_tokens, LaunchError::ExceedsTotalTokens);
    airdrop.amount_per_user = amount_per_user;
    airdrop.whitelisted_count = 0;
    airdrop.distributed_tokens = 0;
    airdrop.bump = ctx.bumps.airdrop_account;
    Ok(())
}

/// Admin-only. Duplicate whitelisting fails because the PDA already exists.
pub fn whitelist_user(ctx: Context<WhitelistUser>, user: Pubkey) -> Result<()> {
    let airdrop = &mut ctx.accounts.airdrop_account;
    require!(Clock::get()?.unix_timestamp < airdrop.end_time, LaunchError::AirdropAlreadyEnded);

    let count = airdrop.whitelisted_count.checked_add(1).ok_or(LaunchError::MathOverflow)?;
    let committed = count.checked_mul(airdrop.amount_per_user).ok_or(LaunchError::MathOverflow)?;
    // Every whitelisted user is guaranteed their full share.
    require!(committed <= airdrop.total_tokens, LaunchError::ExceedsTotalTokens);
    airdrop.whitelisted_count = count;

    let entry = &mut ctx.accounts.whitelist_entry;
    entry.user = user;
    entry.claimed = false;
    entry.bump = ctx.bumps.whitelist_entry;

    emit!(UserWhitelisted { user });
    Ok(())
}

/// The whitelisted user claims their own share exactly once.
pub fn claim_airdrop(ctx: Context<ClaimAirdrop>) -> Result<()> {
    let airdrop = &mut ctx.accounts.airdrop_account;
    let now = Clock::get()?.unix_timestamp;
    require!(now >= airdrop.start_time, LaunchError::AirdropNotStarted);
    require!(now < airdrop.end_time, LaunchError::AirdropAlreadyEnded);

    let entry = &mut ctx.accounts.whitelist_entry;
    require!(!entry.claimed, LaunchError::AlreadyClaimed);

    let amount = airdrop.amount_per_user;
    let distributed = airdrop.distributed_tokens.checked_add(amount).ok_or(LaunchError::MathOverflow)?;
    require!(distributed <= airdrop.total_tokens, LaunchError::ExceedsTotalTokens);

    // Effects before interaction.
    entry.claimed = true;
    airdrop.distributed_tokens = distributed;

    let bump = [ctx.accounts.config.vault_authority_bump];
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &bump];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.airdrop_vault.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    emit!(TokenDistributed { recipient: ctx.accounts.recipient.key(), amount });
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeAirdrop<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Box<Account<'info, Config>>,
    #[account(init, payer = admin, space = 8 + AirdropAccount::INIT_SPACE, seeds = [AIRDROP_SEED], bump)]
    pub airdrop_account: Account<'info, AirdropAccount>,
    #[account(seeds = [AIRDROP_VAULT_SEED], bump)]
    pub airdrop_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct WhitelistUser<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [AIRDROP_SEED], bump = airdrop_account.bump)]
    pub airdrop_account: Account<'info, AirdropAccount>,
    #[account(
        init,
        payer = admin,
        space = 8 + WhitelistEntry::INIT_SPACE,
        seeds = [WHITELIST_SEED, airdrop_account.key().as_ref(), user.as_ref()],
        bump
    )]
    pub whitelist_entry: Account<'info, WhitelistEntry>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimAirdrop<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [AIRDROP_SEED], bump = airdrop_account.bump)]
    pub airdrop_account: Account<'info, AirdropAccount>,
    #[account(
        mut,
        seeds = [WHITELIST_SEED, airdrop_account.key().as_ref(), recipient.key().as_ref()],
        bump = whitelist_entry.bump,
        constraint = whitelist_entry.user == recipient.key() @ LaunchError::Unauthorized,
    )]
    pub whitelist_entry: Account<'info, WhitelistEntry>,
    #[account(mut, seeds = [AIRDROP_VAULT_SEED], bump)]
    pub airdrop_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: PDA signer.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, token::mint = config.mint, token::authority = recipient)]
    pub recipient_token_account: Box<Account<'info, TokenAccount>>,
    pub recipient: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct UserWhitelisted {
    pub user: Pubkey,
}

#[event]
pub struct TokenDistributed {
    pub recipient: Pubkey,
    pub amount: u64,
}
