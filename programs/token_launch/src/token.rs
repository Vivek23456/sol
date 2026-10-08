use anchor_lang::prelude::*;
use anchor_spl::token::{self, spl_token::instruction::AuthorityType, Mint, MintTo, SetAuthority, Token, TokenAccount};

use crate::constants::*;
use crate::errors::LaunchError;

/// Global program state.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    /// Wallet that receives SOL from presale / liquidity sales.
    pub treasury: Pubkey,
    pub mint: Pubkey,
    pub total_supply: u64,
    pub airdrop_allocation: u64,
    pub vaults_funded: bool,
    pub bump: u8,
    pub vault_authority_bump: u8,
}


pub fn initialize(ctx: Context<Initialize>, total_supply: u64) -> Result<()> {
    // Supply must cover the fixed allocations and leave something for the airdrop.
    require!(total_supply > FIXED_ALLOCATIONS, LaunchError::InvalidSupply);

    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.treasury = ctx.accounts.treasury.key();
    config.mint = ctx.accounts.mint.key();
    config.total_supply = total_supply;
    config.airdrop_allocation = total_supply - FIXED_ALLOCATIONS;
    config.vaults_funded = false;
    config.bump = ctx.bumps.config;
    config.vault_authority_bump = ctx.bumps.vault_authority;
    Ok(())
}

pub fn initialize_vaults(ctx: Context<InitializeVaults>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.vaults_funded, LaunchError::Unauthorized);
    let bump = [config.vault_authority_bump];
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &bump];
    let signer: &[&[&[u8]]] = &[seeds];
    let program = ctx.accounts.token_program.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let authority = ctx.accounts.vault_authority.to_account_info();

    let allocations = [
        (ctx.accounts.presale_vault.to_account_info(), PRESALE_ALLOCATION),
        (ctx.accounts.liquidity_vault.to_account_info(), LIQUIDITY_ALLOCATION),
        (ctx.accounts.dev_vault.to_account_info(), DEV_FUND_ALLOCATION),
        (ctx.accounts.airdrop_vault.to_account_info(), config.airdrop_allocation),
    ];
    for (to, amount) in allocations {
        token::mint_to(
            CpiContext::new_with_signer(
                program.clone(),
                MintTo { mint: mint.clone(), to, authority: authority.clone() },
                signer,
            ),
            amount,
        )?;
    }

    // Fixed supply: nobody can ever mint again.
    token::set_authority(
        CpiContext::new_with_signer(
            program,
            SetAuthority { current_authority: authority, account_or_mint: mint },
            signer,
        ),
        AuthorityType::MintTokens,
        None,
    )?;

    config.vaults_funded = true;
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    /// SPL mint: no 8-byte discriminator, created via Anchor's `mint::` constraints.
    #[account(
        init,
        payer = admin,
        seeds = [MINT_SEED],
        bump,
        mint::decimals = DECIMALS,
        mint::authority = vault_authority,
    )]
    pub mint: Account<'info, Mint>,

    /// CHECK: data-less PDA used only as a signer.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    /// CHECK: any system account; only ever receives lamports.
    pub treasury: UncheckedAccount<'info>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeVaults<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin, has_one = mint)]
    pub config: Box<Account<'info, Config>>,

    #[account(mut)]
    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: PDA signer.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(init, payer = admin, seeds = [PRESALE_VAULT_SEED], bump,
        token::mint = mint, token::authority = vault_authority)]
    pub presale_vault: Box<Account<'info, TokenAccount>>,

    #[account(init, payer = admin, seeds = [LIQUIDITY_VAULT_SEED], bump,
        token::mint = mint, token::authority = vault_authority)]
    pub liquidity_vault: Box<Account<'info, TokenAccount>>,

    #[account(init, payer = admin, seeds = [DEV_VAULT_SEED], bump,
        token::mint = mint, token::authority = vault_authority)]
    pub dev_vault: Box<Account<'info, TokenAccount>>,

    #[account(init, payer = admin, seeds = [AIRDROP_VAULT_SEED], bump,
        token::mint = mint, token::authority = vault_authority)]
    pub airdrop_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
