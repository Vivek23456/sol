import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  getAccount,
  getMint,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import { TokenLaunch } from "../target/types/token_launch";

// Mirrors programs/token_launch/src/constants.rs
const ONE_TOKEN = new BN(1_000_000); // 6 decimals
const tokens = (n: number | string) => new BN(n).mul(ONE_TOKEN);
const PRESALE_ALLOCATION = tokens("150000000000");
const LIQUIDITY_ALLOCATION = tokens("100000000000");
const DEV_FUND_ALLOCATION = tokens("100000000000");
const TOTAL_SUPPLY = tokens("400000000000");
const AIRDROP_ALLOCATION = TOTAL_SUPPLY.sub(PRESALE_ALLOCATION)
  .sub(LIQUIDITY_ALLOCATION)
  .sub(DEV_FUND_ALLOCATION);
const TIER1_PRICE = 100_000; // lamports per whole token (0.0001 SOL)

describe("token_launch", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.tokenLaunch as Program<TokenLaunch>;
  const conn = provider.connection;
  const admin = (provider.wallet as anchor.Wallet).payer;

  const pda = (...seeds: (Buffer | Uint8Array)[]) =>
    PublicKey.findProgramAddressSync(seeds, program.programId)[0];
  const config = pda(Buffer.from("config"));
  const mint = pda(Buffer.from("mint"));
  const vaultAuthority = pda(Buffer.from("vault_authority"));
  const presaleVault = pda(Buffer.from("presale_vault"));
  const liquidityVault = pda(Buffer.from("liquidity_vault"));
  const devVault = pda(Buffer.from("dev_vault"));
  const airdropVault = pda(Buffer.from("airdrop_vault"));
  const presaleAccount = pda(Buffer.from("presale"));
  const liquidityState = pda(Buffer.from("liquidity"));
  const fundAccount = pda(Buffer.from("dev_fund"));
  const airdropAccount = pda(Buffer.from("airdrop"));
  const whitelistEntry = (user: PublicKey) =>
    pda(Buffer.from("whitelist"), airdropAccount.toBuffer(), user.toBuffer());

  const treasury = Keypair.generate();
  const buyer = Keypair.generate();
  const attacker = Keypair.generate();
  const beneficiary = Keypair.generate();
  const airdropUser = Keypair.generate();

  /** Funds wallets from the admin (the local validator pre-funds it). */
  const fund = async (wallets: [Keypair, number][]) => {
    const tx = new Transaction();
    for (const [kp, sol] of wallets) {
      tx.add(
        SystemProgram.transfer({ fromPubkey: admin.publicKey, toPubkey: kp.publicKey, lamports: sol * LAMPORTS_PER_SOL })
      );
    }
    await provider.sendAndConfirm(tx);
  };
  const ata = async (owner: Keypair) =>
    (await getOrCreateAssociatedTokenAccount(conn, admin, mint, owner.publicKey)).address;
  const balance = async (account: PublicKey) =>
    new BN((await getAccount(conn, account)).amount.toString());

  /** Asserts the transaction fails and its error/logs mention `needle`. */
  const expectFail = async (p: Promise<unknown>, needle: string) => {
    try {
      await p;
    } catch (e: any) {
      const text = [String(e), ...(e?.logs ?? []), ...(e?.transactionLogs ?? [])].join("\n");
      expect(text).to.include(needle);
      return;
    }
    expect.fail(`expected failure containing "${needle}"`);
  };

  before(async () => {
    await fund([[buyer, 10], [attacker, 10], [airdropUser, 1], [beneficiary, 1]]);
  });

  // ------------------------------------------------------------------ token
  describe("token", () => {
    it("rejects a total supply that does not cover the fixed allocations", async () => {
      await expectFail(
        program.methods
          .initialize(tokens("350000000000"))
          .accountsPartial({ config, mint, vaultAuthority, treasury: treasury.publicKey, admin: admin.publicKey })
          .rpc(),
        "InvalidSupply"
      );
    });

    it("initializes config + mint with a PDA mint authority", async () => {
      await program.methods
        .initialize(TOTAL_SUPPLY)
        .accountsPartial({ config, mint, vaultAuthority, treasury: treasury.publicKey, admin: admin.publicKey })
        .rpc();

      const cfg = await program.account.config.fetch(config);
      expect(cfg.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(cfg.treasury.toBase58()).to.equal(treasury.publicKey.toBase58());
      expect(cfg.airdropAllocation.toString()).to.equal(AIRDROP_ALLOCATION.toString());

      const m = await getMint(conn, mint);
      expect(m.decimals).to.equal(6);
      expect(m.mintAuthority!.toBase58()).to.equal(vaultAuthority.toBase58());
    });

    it("blocks a non-admin from funding the vaults", async () => {
      await expectFail(
        program.methods
          .initializeVaults()
          .accountsPartial({
            config, mint, vaultAuthority, presaleVault, liquidityVault, devVault, airdropVault,
            admin: attacker.publicKey,
          })
          .signers([attacker])
          .rpc(),
        "ConstraintHasOne"
      );
    });

    it("mints the fixed supply into the vaults and revokes the mint authority", async () => {
      await program.methods
        .initializeVaults()
        .accountsPartial({
          config, mint, vaultAuthority, presaleVault, liquidityVault, devVault, airdropVault,
          admin: admin.publicKey,
        })
        .rpc();

      expect((await balance(presaleVault)).toString()).to.equal(PRESALE_ALLOCATION.toString());
      expect((await balance(liquidityVault)).toString()).to.equal(LIQUIDITY_ALLOCATION.toString());
      expect((await balance(devVault)).toString()).to.equal(DEV_FUND_ALLOCATION.toString());
      expect((await balance(airdropVault)).toString()).to.equal(AIRDROP_ALLOCATION.toString());

      const m = await getMint(conn, mint);
      expect(m.supply.toString()).to.equal(TOTAL_SUPPLY.toString());
      expect(m.mintAuthority).to.equal(null); // fixed supply, nobody can mint again
    });
  });

  // ---------------------------------------------------------------- presale
  describe("presale", () => {
    let buyerAta: PublicKey;
    const buyAccounts = () => ({
      config, presaleAccount, presaleVault, vaultAuthority,
      buyerTokenAccount: buyerAta, treasury: treasury.publicKey, buyer: buyer.publicKey,
    });

    before(async () => {
      buyerAta = await ata(buyer);
    });

    it("only the admin can initialize the presale", async () => {
      await expectFail(
        program.methods
          .initializePresale()
          .accountsPartial({ config, presaleAccount, admin: attacker.publicKey })
          .signers([attacker])
          .rpc(),
        "ConstraintHasOne"
      );
      await program.methods.initializePresale().accountsPartial({ config, presaleAccount, admin: admin.publicKey }).rpc();
    });

    it("buyer actually pays SOL to the treasury (tokens are no longer free)", async () => {
      const amount = tokens(1_000);
      const expectedCost = 1_000 * TIER1_PRICE; // 0.1 SOL
      const before = await conn.getBalance(treasury.publicKey);

      await program.methods
        .buyTokens(amount, new BN(expectedCost))
        .accountsPartial(buyAccounts())
        .signers([buyer])
        .rpc();

      expect((await conn.getBalance(treasury.publicKey)) - before).to.equal(expectedCost);
      expect((await balance(buyerAta)).toString()).to.equal(amount.toString());
      const p = await program.account.presaleAccount.fetch(presaleAccount);
      expect(p.totalSold.toString()).to.equal(amount.toString());
    });

    it("enforces the buyer's slippage limit", async () => {
      await expectFail(
        program.methods
          .buyTokens(tokens(1_000), new BN(1_000 * TIER1_PRICE - 1))
          .accountsPartial(buyAccounts())
          .signers([buyer])
          .rpc(),
        "SlippageExceeded"
      );
    });

    it("rejects zero and over-allocation purchases", async () => {
      await expectFail(
        program.methods.buyTokens(new BN(0), new BN(1)).accountsPartial(buyAccounts()).signers([buyer]).rpc(),
        "InvalidAmount"
      );
      await expectFail(
        program.methods
          .buyTokens(PRESALE_ALLOCATION, new BN("18446744073709551615"))
          .accountsPartial(buyAccounts())
          .signers([buyer])
          .rpc(),
        "ExceedsPresaleAllocation"
      );
    });

    it("rejects payment to a treasury other than the configured one", async () => {
      await expectFail(
        program.methods
          .buyTokens(tokens(1), new BN(TIER1_PRICE))
          .accountsPartial({ ...buyAccounts(), treasury: attacker.publicKey })
          .signers([buyer])
          .rpc(),
        "ConstraintHasOne"
      );
    });
  });

  // -------------------------------------------------------------- liquidity
  describe("liquidity", () => {
    const PRICE = 150_000;
    let buyerAta: PublicKey;
    const purchaseAccounts = () => ({
      config, state: liquidityState, liquidityVault, vaultAuthority,
      buyerTokenAccount: buyerAta, treasury: treasury.publicKey, buyer: buyer.publicKey,
    });

    before(async () => {
      buyerAta = await ata(buyer);
    });

    it("only the admin can initialize; liquidity is backed by the vault", async () => {
      await expectFail(
        program.methods
          .initializeLiquidity(new BN(1))
          .accountsPartial({ config, state: liquidityState, liquidityVault, admin: attacker.publicKey })
          .signers([attacker])
          .rpc(),
        "ConstraintHasOne"
      );
      await program.methods
        .initializeLiquidity(new BN(PRICE))
        .accountsPartial({ config, state: liquidityState, liquidityVault, admin: admin.publicKey })
        .rpc();
      const s = await program.account.liquidityState.fetch(liquidityState);
      expect(s.totalLiquidity.toString()).to.equal(LIQUIDITY_ALLOCATION.toString());
    });

    it("purchase charges the admin price, transfers tokens and persists state", async () => {
      const amount = tokens(500);
      const before = await conn.getBalance(treasury.publicKey);
      const tokensBefore = await balance(buyerAta);

      await program.methods
        .purchaseTokens(amount, new BN(PRICE))
        .accountsPartial(purchaseAccounts())
        .signers([buyer])
        .rpc();

      expect((await conn.getBalance(treasury.publicKey)) - before).to.equal(500 * PRICE);
      expect((await balance(buyerAta)).sub(tokensBefore).toString()).to.equal(amount.toString());
      const s = await program.account.liquidityState.fetch(liquidityState);
      expect(s.soldTokens.toString()).to.equal(amount.toString());
      expect(s.totalLiquidity.toString()).to.equal(LIQUIDITY_ALLOCATION.sub(amount).toString());
    });

    it("buyer cannot pick their own (lower) price", async () => {
      await expectFail(
        program.methods
          .purchaseTokens(tokens(1), new BN(1))
          .accountsPartial(purchaseAccounts())
          .signers([buyer])
          .rpc(),
        "SlippageExceeded"
      );
    });
  });

  // --------------------------------------------------------------- dev fund
  describe("dev fund", () => {
    it("only the admin can allocate", async () => {
      await expectFail(
        program.methods
          .allocateTokens(attacker.publicKey)
          .accountsPartial({ config, fundAccount, devVault, admin: attacker.publicKey })
          .signers([attacker])
          .rpc(),
        "ConstraintHasOne"
      );
      await program.methods
        .allocateTokens(beneficiary.publicKey)
        .accountsPartial({ config, fundAccount, devVault, admin: admin.publicKey })
        .rpc();
      const f = await program.account.fundAccount.fetch(fundAccount);
      expect(f.beneficiary.toBase58()).to.equal(beneficiary.publicKey.toBase58());
      expect(f.totalAllocated.toString()).to.equal(DEV_FUND_ALLOCATION.toString());
    });

    it("nothing is released before the first month has vested", async () => {
      const recipient = await ata(beneficiary);
      await expectFail(
        program.methods
          .releaseTokens()
          .accountsPartial({ config, fundAccount, devVault, vaultAuthority, recipientTokenAccount: recipient })
          .rpc(),
        "NoTokensAvailable"
      );
    });

    it("releases can only go to the beneficiary's token account", async () => {
      const attackerAta = await ata(attacker);
      await expectFail(
        program.methods
          .releaseTokens()
          .accountsPartial({ config, fundAccount, devVault, vaultAuthority, recipientTokenAccount: attackerAta })
          .rpc(),
        "ConstraintTokenOwner"
      );
    });
  });

  // ---------------------------------------------------------------- airdrop
  describe("airdrop", () => {
    const PER_USER = tokens(1_000);

    it("admin initializes the airdrop window", async () => {
      const now = Math.floor(Date.now() / 1000);
      await program.methods
        .initializeAirdrop(PER_USER, new BN(now - 60), new BN(3600))
        .accountsPartial({ config, airdropAccount, airdropVault, admin: admin.publicKey })
        .rpc();
      const a = await program.account.airdropAccount.fetch(airdropAccount);
      expect(a.totalTokens.toString()).to.equal(AIRDROP_ALLOCATION.toString());
    });

    it("users cannot whitelist themselves", async () => {
      await expectFail(
        program.methods
          .whitelistUser(attacker.publicKey)
          .accountsPartial({
            config, airdropAccount, whitelistEntry: whitelistEntry(attacker.publicKey), admin: attacker.publicKey,
          })
          .signers([attacker])
          .rpc(),
        "ConstraintHasOne"
      );
    });

    it("admin whitelists a user exactly once", async () => {
      const accounts = {
        config, airdropAccount, whitelistEntry: whitelistEntry(airdropUser.publicKey), admin: admin.publicKey,
      };
      await program.methods.whitelistUser(airdropUser.publicKey).accountsPartial(accounts).rpc();
      await expectFail(program.methods.whitelistUser(airdropUser.publicKey).accountsPartial(accounts).rpc(), "already in use");
      const a = await program.account.airdropAccount.fetch(airdropAccount);
      expect(a.whitelistedCount.toNumber()).to.equal(1);
    });

    it("whitelisted user claims a fixed share once; second claim fails", async () => {
      const recipientAta = await ata(airdropUser);
      const accounts = {
        config, airdropAccount, whitelistEntry: whitelistEntry(airdropUser.publicKey), airdropVault,
        vaultAuthority, recipientTokenAccount: recipientAta, recipient: airdropUser.publicKey,
      };
      await program.methods.claimAirdrop().accountsPartial(accounts).signers([airdropUser]).rpc();
      expect((await balance(recipientAta)).toString()).to.equal(PER_USER.toString());

      await expectFail(
        program.methods.claimAirdrop().accountsPartial(accounts).signers([airdropUser]).rpc(),
        "AlreadyClaimed"
      );
      expect((await balance(recipientAta)).toString()).to.equal(PER_USER.toString());
    });

    it("non-whitelisted users cannot claim", async () => {
      const attackerAta = await ata(attacker);
      await expectFail(
        program.methods
          .claimAirdrop()
          .accountsPartial({
            config, airdropAccount, whitelistEntry: whitelistEntry(attacker.publicKey), airdropVault,
            vaultAuthority, recipientTokenAccount: attackerAta, recipient: attacker.publicKey,
          })
          .signers([attacker])
          .rpc(),
        "AccountNotInitialized"
      );
    });
  });
});
