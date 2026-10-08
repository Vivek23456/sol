# Solana Token Launch: Assessment Fix

The assessment was a set of buggy Anchor/Rust smart-contract files (`token.rs`, `presale.rs`, `liquidity.rs`, `dev_fund.rs`, `airdrop.rs`, `lib.rs`). They are still in the repo root for reference. The original code **did not compile**. Fixing the compile errors would still have left serious security bugs: tokens were free in the presale, airdrop claims were unlimited, anyone could whitelist themselves, and vesting could be bypassed.

This repo fixes it as **one Anchor program** (`programs/token_launch`). It builds with `anchor build` and passes **20 on-chain integration tests** with `anchor test`.

**Every bug (47 in total) is listed with its severity and fix in [`BUGS_AND_FIXES.md`](BUGS_AND_FIXES.md).**

---

## How it works

The program creates an SPL token with a **fixed supply**. It mints the whole supply into four vaults controlled by the program (PDAs), then **permanently removes the mint authority**. Each feature can only hand out tokens from its own vault, under the program's rules.

| Allocation | Amount | Rules |
|---|---|---|
| Presale | 150B | 3 tiers of 50B at 0.0001 / 0.0002 / 0.0003 SOL; SOL goes to the treasury |
| Liquidity | 100B | Fixed price set by the admin; buyer gets a slippage guard |
| Dev fund | 100B | 24-month linear vesting; released only to the stored beneficiary |
| Airdrop | total supply − 350B | Admin whitelists users; each user claims a fixed amount once |

Token decimals are 6. The original used 9, and at 9 decimals 50B tokens don't fit in a `u64`.

### Instructions

```
initialize(total_supply)                             config PDA + mint (authority = PDA)
initialize_vaults()                                  create 4 vaults, mint allocations, revoke mint authority

initialize_presale()                                 admin only
buy_tokens(amount, max_cost_lamports)                pays SOL, receives tokens; price split across tiers

initialize_liquidity(price_lamports_per_token)       admin only
purchase_tokens(amount, max_price)

allocate_tokens(beneficiary)                         admin only; starts vesting
release_tokens()                                     anyone can call; tokens go only to the beneficiary

initialize_airdrop(amount_per_user, start, duration) admin only
whitelist_user(user)                                 admin only
claim_airdrop()                                      whitelisted user, once
```

---

## Project layout

```
programs/token_launch/src/
  lib.rs         program entrypoint, routes every instruction
  token.rs       config, mint, vaults, mint-authority revoke
  presale.rs     tiered presale
  liquidity.rs   fixed-price liquidity sale
  dev_fund.rs    vesting dev fund
  airdrop.rs     whitelist + one-time claims
  constants.rs   tokenomics, decimals, PDA seeds
  errors.rs      single error enum
  math.rs        tier pricing + vesting maths (pure functions)
tests/token_launch.ts  on-chain integration tests (Anchor + Mocha)
math-tests/            host unit tests for math.rs (tier crossing, 24-month vesting)
BUGS_AND_FIXES.md      every bug found and how it was fixed
*.rs (repo root)       original assessment files, unchanged
```

---

## Requirements

- Rust (stable)
- Solana CLI 2.1.x (`solana --version`)
- Anchor CLI 0.31.1 (`anchor --version`)
- Node.js 18+ and npm

Program ID (localnet): `2NrBKkt2FUntwsPXvjDdi8ry5FcgYLBSjXYmVrxCBMbf`

If you deploy with your own keypair, run `anchor keys sync` before building so `declare_id!` matches it.

## Build and test

```bash
npm install

# compile the smart contract -> target/deploy/token_launch.so
anchor build

# start a local validator, deploy, and run the 20 integration tests
anchor test

# maths unit tests (tier boundaries, 24-month vesting)
cd math-tests && cargo test
```

Expected `anchor test` result:

```
token_launch
  token      ✔ x4
  presale    ✔ x5
  liquidity  ✔ x3
  dev fund   ✔ x3
  airdrop    ✔ x5
20 passing
```

### What the tests check

- The supply is fixed: the vaults hold exactly 150B / 100B / 100B / the airdrop remainder, and the mint authority is `null`.
- Admin-only setup: a non-admin calling any init instruction fails.
- Presale: the buyer **really pays SOL** to the treasury, the slippage limit is enforced, zero or over-cap buys are rejected, and a fake treasury is rejected.
- Liquidity: the admin's price is charged, tokens are transferred, state is saved, and the buyer can't set their own price.
- Dev fund: nothing is released before the first month, and releases go only to the beneficiary.
- Airdrop: no self-whitelisting, no double whitelisting, one claim per user, and non-whitelisted users can't claim.

Behaviour the local validator can't easily reach is covered by the unit tests in `math-tests/`: a presale purchase crossing tier boundaries (it costs millions of SOL) and the full 24-month vesting schedule (it needs time to pass).

---

## Build notes

- **`Cargo.lock` is pinned** to dependency versions that work with the Rust 1.79 bundled in Solana platform-tools v1.43 (Solana CLI 2.1.x). If you use a newer Solana CLI, you can run `cargo update`.
- **Windows only:**
  - `.cargo/config.toml` sets an absolute `target-dir`. It works around a Windows path bug in `anchor build`. **On Linux or macOS, or in a different folder, delete or edit this file.**
  - `solana-test-validator` needs **Developer Mode** (Settings → System → For developers) or an Administrator terminal.

## Deployment

The program has been built and tested on a local validator only. To deploy to devnet:

```bash
solana config set --url devnet
solana airdrop 5
anchor deploy --provider.cluster devnet
```
