# REBAL

# 🎯 **REBAL — Rebalancing Execution Token**

## ⭐️ **Overview**

**REBAL** is a Solana smart contract (program) built using the **Anchor framework**. It powers a decentralized, self-custodied index rebalancing system that rewards bots in `$REBAL` tokens when they execute weight adjustments across on-chain digital asset baskets.

It also enables `$REBAL` token stakers to participate in governance by voting on:

- Rebalancing strategies (e.g. periodic, threshold-based)
- Allocation deviation thresholds
- Eligible assets per basket

This program is designed for **real-time index tracking**, **bot execution rewards**, and **community-driven basket configuration**.
This Program was developed in Solana Playground IDE and will be exported for vscode for next version

**devnet**:(https://explorer.solana.com/address/DVh3z1LQs6QXEtkc5TvzRq7v9fzoENc8UzeDedoiMAap?cluster=devnet)

---

## 🔧 **Key Features**

### 🗳 Governance
- **Snapshot voting**: Captures token supply at proposal time to prevent vote manipulation.
- **Vote locking**: Temporarily locks staked tokens in escrow during voting.
- **Quorum enforcement**: Requires a minimum % of staked tokens for a proposal to pass.
- **Proposal expiration**: Ensures proposals are finalized in a timely manner.

### ⚖️ Rebalancing Incentives
- **Dynamic rewards**: Higher $REBAL rewards for correcting larger deviations.
- **Cooldown timers**: Prevent bots from spamming rebalances for free tokens.
- **Lamport reimbursements**: Covers transaction fees for approved bots.
- **Slashing**: Reduces rewards if bot action deviates too far from optimal rebalancing range.

### 🛡 Security
- **Program-derived mint authority**: Minting $REBAL is only possible via a secure PDA.
- **Whitelist for rebalancers**: Restricts reward eligibility to approved bots (optional).
- **Proposal safety**: Invalid or expired proposals are automatically rejected.

### 🧠 DevEx & UX
- **Anchor events**: Emits logs for all proposal, vote, and rebalance actions.
- **Named baskets**: Supports multiple indexes, each with metadata and unique config.
- **Typescript test file**: Built-in test setup using Solana Playground's built-in globals.

---

## 🚀 **How It Works**

1. **Admin initializes a basket** with config (name, description, eligible assets, thresholds).
2. **Users stake $REBAL** and vote on proposals for:
   - Strategy type
   - Rebalance threshold
   - Eligible token list
3. **Bots monitor baskets**, rebalance them based on config, then call `executeRebalance`.
4. **Bots are rewarded** with:
   - `$REBAL` tokens (via PDA mint)
   - Optional lamport reimbursements from a funded fee vault.

---

## 🧪 **Testing (Solana Playground)**

The test file:
- Initializes a SPL token mint
- Derives program PDAs
- Airdrops SOL to the fee vault
- Calls `initializeBasket`
- Fetches & asserts correct basket config


---

## 🧾 **Program Instructions**

- `initialize_basket(...)`: Initializes a new basket.
- `propose_threshold(...)`, `vote_threshold(...)`, `finalize_threshold(...)`
- `propose_strategy(...)`, `vote_strategy(...)`, `finalize_strategy(...)`
- `propose_assets(...)`, `vote_assets(...)`, `finalize_assets(...)`
- `execute_rebalance(...)`: Called by rebalancer bots to claim rewards after execution.

---
