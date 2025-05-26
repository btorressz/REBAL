import * as web3 from "@solana/web3.js";
import * as splToken from "@solana/spl-token";
import BN from "bn.js";
import assert from "assert";

describe("REBAL Program", () => {
  it("initializes a BasketConfig", async () => {
    // 1) Prepare keypairs & PDAs
    const basketKp = web3.Keypair.generate();
    const mintKp   = web3.Keypair.generate();

    // 2) Create the REBAL mint
    const mintRent = await pg.connection.getMinimumBalanceForRentExemption(
      splToken.MintLayout.span
    );
    const txInitMint = new web3.Transaction().add(
      // a) create account
      web3.SystemProgram.createAccount({
        fromPubkey: pg.wallet.publicKey,
        newAccountPubkey: mintKp.publicKey,
        space: splToken.MintLayout.span,
        lamports: mintRent,
        programId: splToken.TOKEN_PROGRAM_ID,
      }),
      // b) initialize mint (6 decimals, authority = our test wallet)
      splToken.createInitializeMintInstruction(
        mintKp.publicKey,
        6,
        pg.wallet.publicKey,
        null,
        splToken.TOKEN_PROGRAM_ID
      )
    );
    await pg.connection.sendTransaction(txInitMint, [mintKp]);

    // 3) Derive the two PDAs your program expects
    const [mintAuthPda, mintAuthBump] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("mint_auth"), basketKp.publicKey.toBuffer()],
      pg.program.programId
    );
    const [feeVaultPda, feeVaultBump] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fee_vault"), basketKp.publicKey.toBuffer()],
      pg.program.programId
    );

    // 4) Fund the fee vault PDA so execute_rebalance tests later won't run out of lamports
    const airdropSig = await pg.connection.requestAirdrop(
      feeVaultPda,
      web3.LAMPORTS_PER_SOL
    );
    await pg.connection.confirmTransaction(airdropSig);

    // 5) Call initializeBasket
    const name           = "Test Basket";
    const description    = "A test basket";
    const threshold      = new BN(5);
    const strategy       = 0;
    const eligibleAssets = [mintKp.publicKey];
    const quorum         = 10;
    const cooldown       = new BN(60);
    const baseReward     = new BN(1000);
    const lamportsReward = new BN(1_000);
    const slashFactor    = new BN(2);

    const tx2 = await pg.program.methods
      .initializeBasket(
        name,
        description,
        threshold,
        strategy,
        eligibleAssets,
        quorum,
        cooldown,
        baseReward,
        lamportsReward,
        slashFactor,
        mintAuthBump,
        feeVaultBump
      )
      .accounts({
        authority:     pg.wallet.publicKey,
        basket:        basketKp.publicKey,
        rebalMint:     mintKp.publicKey,
        mintAuth:      mintAuthPda,
        feeVault:      feeVaultPda,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([basketKp])
      .rpc();
    await pg.connection.confirmTransaction(tx2);

    // 6) Fetch and assert on‐chain state
    const basket = await pg.program.account.basketConfig.fetch(
      basketKp.publicKey
    );
    assert.equal(basket.name, name);
    assert.equal(basket.description, description);
    assert.ok(basket.threshold.eq(threshold));
    assert.equal(basket.strategy, strategy);
    assert.equal(
      basket.eligibleAssets[0].toBase58(),
      mintKp.publicKey.toBase58()
    );
  });
});
