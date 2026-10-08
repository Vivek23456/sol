# Solana Assessment: Bugs Found and Fixes

The original code was five separate `#[program]` modules plus a raw `entrypoint!`, pasted into one crate. It would not compile. Even with the compile errors fixed, it had several critical security bugs: tokens were free in the presale, airdrop claims were unlimited, anyone could whitelist themselves, liquidity state was never saved, and the mint authority stayed with one wallet so there was no vesting guarantee.

The fix is a single Anchor program (`programs/token_launch`). It mints a **fixed supply** into four **PDA-owned vaults** and then revokes the mint authority for good. Each feature hands out tokens from its own vault.

```
initialize(total_supply)      -> Config PDA + Mint PDA (authority = vault_authority PDA)
initialize_vaults()           -> presale / liquidity / dev / airdrop vaults, mint allocations, revoke mint authority
presale:   initialize_presale, buy_tokens(amount, max_cost_lamports)
liquidity: initialize_liquidity(price), purchase_tokens(amount, max_price)
dev fund:  allocate_tokens(beneficiary), release_tokens()        (permissionless crank)
airdrop:   initialize_airdrop(amount_per_user, start, duration), whitelist_user(user), claim_airdrop()
```

Tokenomics: presale 150B (3 tiers of 50B at 0.0001 / 0.0002 / 0.0003 SOL), liquidity 100B, dev fund 100B (24-month linear vesting), airdrop = total supply − 350B.

---

## Cross-cutting / `lib.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|

| 1| `pub mod dev_team;`, but the file is `dev_fund.rs` | Compile | `pub mod dev_fund;` |
| 2| Five `#[program]` modules in one crate means five entrypoints; there's also a manual `entrypoint!` that routes nothing (`process_instruction` just returns `Ok`) | Compile / logic | One `#[program] token_launch` that dispatches to the module handlers; manual entrypoint removed |
| 3| Glob re-exports collide: two `Initialize` structs and five `ErrorCode` enums | Compile | Unique context names and one `LaunchError` enum (`errors.rs`) |
| 4| `use solana_program::...` without the dependency; `System` imported from `anchor_spl::token`, where it doesn't exist | Compile | Use `anchor_lang::prelude` |
| 5| **9 decimals vs. u64.** `u64::MAX` ≈ 1.84e19 base units, so 9 decimals allows only ~18.4B whole tokens. 50B tokens = 5e19 base units, which overflows. All the thresholds (`50_000_000_000`) were actually 50 tokens, not 50B | High | `DECIMALS = 6`; every amount is expressed as `N * ONE_TOKEN` (`constants.rs`), and a unit test proves it fits |
| 6| No `Config`/admin anywhere, so every "init" instruction could be front-run by anyone | High | `Config` PDA stores `admin`, `treasury` and `mint`; admin instructions use `has_one = admin` |

## `token.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 8 | `token::initialize_mint(program, mint, &key, None, 9)`: wrong signature (it needs a `CpiContext` with `InitializeMint` + rent) | Compile | Anchor `init` + `mint::decimals` / `mint::authority` constraints |
| 9 | `init` on `Account<Mint>` / `Account<TokenAccount>` with `space = 8 + LEN`. SPL accounts have no 8-byte discriminator and must be owned by the Token program | Compile / runtime | `mint::` / `token::` init constraints (no manual space) |
| 10 | `authority` is the `payer` but isn't `mut`; `rent` is missing | Compile | `#[account(mut)]`, `Sysvar<Rent>` |
| 11 | `cpi_program` is moved into `initialize_mint` and then reused | Compile | n/a (rewritten) |
| 12 | **Mint authority stays with a user wallet**, so unlimited minting is possible and the "total supply" means nothing | Critical | Mint authority = `vault_authority` PDA; the whole supply is minted into the vaults, then `set_authority(MintTokens, None)` |
| 13 | `total_supply` was only checked against 0 | Low | Must exceed the fixed 350B allocations |

## `presale.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 14 | **SOL is never transferred.** The code only reads `buyer.lamports()` and then mints, so tokens are free | Critical | `system_program::transfer` buyer → `treasury` (checked against `config.treasury`) |
| 15 | `mint_to` with `authority = buyer`: either it always fails, or (if the buyer is the mint authority) the buyer mints for free. Also wrong arity (no `CpiContext`) | Critical / compile | Transfer from the pre-minted presale vault, signed by the PDA |
| 16 | **Tier picked from `total_sold` before the purchase**, so one transaction buying 150B pays the tier-1 price for everything | High | `math::presale_cost` splits the purchase across tier boundaries |
| 17 | No cap: `total_sold + amount` can exceed 150B | High | `require!(end <= PRESALE_ALLOCATION)` |
| 18 | `f64` pricing on-chain: imprecise and lossy (`as u64` truncates, so small buys can cost 0) | Medium | Integer lamports per token, `u128` math, rounds **up** |
| 19 | No `initialize_presale`; `presale_account` isn't a PDA | Medium | Admin-only init with PDA `["presale"]` |
| 20 | `buyer_token_account` and `mint` are unvalidated, so any token account can be passed | Medium | `token::mint = config.mint, token::authority = buyer` |
| 21 | No slippage protection for the buyer when a tier flips | Low | `max_cost_lamports` argument |
| 22 | Buyer pays but isn't `mut`; no `system_program` | Compile / runtime | Fixed |

## `liquidity.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 23 | `state` isn't `mut` in `PurchaseTokens`, so changes are never persisted (or the transaction fails writing a read-only account) | High | `#[account(mut)]` |
| 24 | **No payment and no token transfer**; the instruction only decrements a counter | Critical | SOL → treasury, tokens from the liquidity vault |
| 25 | `price` is supplied by the buyer and never used | Critical | Price stored in state by the admin; the buyer's `max_price` is only a slippage guard |
| 26 | `initialize` is permissionless, and `total_liquidity` is an arbitrary argument not backed by tokens | High | Admin-only; `total_liquidity = liquidity_vault.amount` |
| 27 | `ErrorCode::InsufficientLiquidity` is used but never defined; Token program and imports unused | Compile | Defined in `LaunchError` |
| 28 | Unchecked `-=` / `+=` | Low | `checked_sub` / `checked_add` |
| 29 | `space = 8 + 64` (needs 8 + 16 at the time) | Low | `InitSpace` |

## `dev_fund.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 30 | `elapsed_months` is reassigned but not `mut`; `i64` vs `u64` mismatch with the constants | Compile | Rewritten in `math::vested_amount` |
| 31 | **"Seconds in a month" is `24*3600*365`, which is seconds in a year**, so vesting runs 12× slower (24 *years*) | High | `SECONDS_PER_MONTH = 2_629_746` (average Gregorian month) |
| 32 | `TOKENS_PER_MONTH = 100B / 24` leaves remainder dust that never vests; it also ignores the `amount` passed to `allocate_tokens` | Medium | Vested = `total_allocated * months / 24`, and 100% at month 24 |
| 33 | `fund_token_account` is used in `release_tokens` but missing from `ReleaseTokens` | Compile | Added (as the PDA dev vault) |
| 34 | **Vesting isn't enforced**: the transfer authority is a wallet that owns the fund account, so it can just transfer everything directly | Critical | Tokens sit in a PDA vault; only the program can release them |
| 35 | No beneficiary stored, and the recipient is any token account | High | `beneficiary` stored; `recipient_token_account` must be owned by it |
| 36 | `allocate_tokens` is permissionless and never moves tokens in; `FundAccount::LEN` doesn't exist; payer isn't `mut`; no `system_program` | High / compile | Admin-only; allocation = dev vault balance; `InitSpace` |
| 37 | Negative elapsed time (clock earlier than start) cast into the math | Low | Returns 0 |
| 38 | `token::transfer` has the wrong arity (no `CpiContext`); unchecked `+=` | Compile | Fixed |

## `airdrop.rs`

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 39 | **No claimed flag**, so a whitelisted user can call `distribute_airdrop` repeatedly and drain the pool | Critical | Per-user `WhitelistEntry { claimed }` |
| 40 | **Self-whitelisting**: `user: Signer` whitelists itself, with no admin check | Critical | Admin-only `whitelist_user(user)` (`has_one = admin`) |
| 41 | `tokens_per_user = total / whitelisted.len()` is recomputed at every claim, so early claimers get more, later whitelisting dilutes everyone, and late users get nothing | High | Fixed `amount_per_user`; whitelisting is capped so every listed user is fully funded |
| 42 | `HashSet<Pubkey>` in an account: not supported by the IDL, O(n) (de)serialization compute, and `32 * 1000` bytes is over the 10 KB CPI `init` limit, so `initialize` fails | High / compile | One PDA per user `["whitelist", airdrop, user]`; duplicates are rejected because the PDA already exists |
| 43 | `ctx.accounts.recipient` is used but not in the struct; `recipient_token_account` is unconstrained | Compile / high | `recipient: Signer` + `token::authority = recipient` |
| 44 | Transfer authority is an admin wallet, so the admin must co-sign every claim | Medium | PDA vault authority |
| 45 | `start_time + duration` can overflow, and `duration <= 0` is accepted | Low | Checked |
| 46 | `total_tokens` isn't backed by real tokens | Medium | `total_tokens = airdrop_vault.amount` |
| 47 | Space math uses 16 bytes per i64/u64 | Low | `InitSpace` |

---

## Verification

- `math-tests/`: 7 host unit tests run against the program's real `constants.rs` and `math.rs`. They cover tier pricing, purchases that straddle tiers, the presale cap, rounding up, the u64 supply bound, the vesting schedule (seconds-per-month, cap, no dust, clock before start) and monotonicity. Run them with `cd math-tests && cargo test`. **All pass.**
- `anchor build` compiles the program (Anchor 0.31.1, Solana CLI 2.1.7) and generates the IDL and TS types.
- `anchor test` starts a local validator, deploys the program and runs `tests/token_launch.ts`. **All 20 integration tests pass.** They cover the fixed supply and revoked mint authority, admin-only init for every module, the presale paying SOL to the treasury, slippage and allocation caps, the treasury check, liquidity state being persisted at the admin price, no early dev-fund release and beneficiary-only releases, no self-whitelisting, single-use whitelisting, one claim per user, and non-whitelisted users being rejected.

### Build notes
- **`anchor-spl` default features** must stay enabled. Anchor 0.31's `mint::`/`token::` init constraints refer to `token_interface`/`token_2022`.
- **`Cargo.lock`** is pinned (`blake3` 1.8.2, `serde` 1.0.219, `proc-macro-crate` 3.2.0, …) because Solana platform-tools v1.43 ships rustc/Cargo 1.79, which can't build newer edition-2024 or MSRV-1.85 crates.
- **Windows:**
  - `.cargo/config.toml` sets an absolute `target-dir`. It works around `anchor build` passing `\?\` paths, which rustc 1.79 can't `include!` from. Edit or delete it on another machine.
  - `solana-test-validator` needs Developer Mode (or an elevated shell) to create its ledger symlink.
