use anchor_lang::prelude::*;
use anchor_lang::solana_program::{system_instruction, clock::Clock};
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("DVh3z1LQs6QXEtkc5TvzRq7v9fzoENc8UzeDedoiMAap");

#[program]
pub mod rebalancing_execution {
    use super::*;

    /// Initialize a new basket (with metadata) and all on‐chain config.
    pub fn initialize_basket(
        ctx: Context<InitializeBasket>,
        name: String,
        description: String,
        initial_threshold: u64,
        initial_strategy: u8,
        initial_assets: Vec<Pubkey>,
        quorum_percentage: u8,
        cooldown_seconds: u64,
        base_reward: u64,
        lamports_reward: u64,
        slash_factor: u64,
        mint_auth_bump: u8,
        fee_vault_bump: u8,
    ) -> Result<()> {
        let cfg = &mut ctx.accounts.basket;
        cfg.initializer = ctx.accounts.authority.key();
        cfg.name = name;
        cfg.description = description;
        cfg.rebal_mint = ctx.accounts.rebal_mint.key();
        cfg.threshold = initial_threshold;
        cfg.strategy = initial_strategy;
        cfg.eligible_assets = initial_assets;
        cfg.quorum_percentage = quorum_percentage;
        cfg.cooldown_seconds = cooldown_seconds;
        cfg.base_reward = base_reward;
        cfg.lamports_reward = lamports_reward;
        cfg.slash_factor = slash_factor;
        cfg.last_rebalance_ts = 0;
        cfg.whitelist = Vec::new();
        cfg.mint_auth_bump = mint_auth_bump;
        cfg.fee_vault_bump = fee_vault_bump;
        Ok(())
    }

    /// Create a threshold‐change proposal (takes a supply snapshot & sets expiry).
    pub fn propose_threshold(
        ctx: Context<ProposeThreshold>,
        new_threshold: u64,
        expiration_ts: i64,
    ) -> Result<()> {
        let cfg = &ctx.accounts.basket;
        let p = &mut ctx.accounts.threshold_proposal;
        p.proposer = ctx.accounts.proposer.key();
        p.basket = cfg.key();
        p.proposed_threshold = new_threshold;
        p.yes_votes = 0;
        p.no_votes = 0;
        p.snapshot_supply = ctx.accounts.rebal_mint.supply;
        p.quorum_percentage = cfg.quorum_percentage;
        p.expiration = expiration_ts;
        p.voters = Vec::new();
        emit!(ProposalCreated {
            basket: cfg.key(),
            kind: ProposalType::Threshold,
            proposer: p.proposer,
            expiration: p.expiration,
        });
        Ok(())
    }

    /// Vote on a threshold proposal.
    pub fn vote_threshold(
        ctx: Context<VoteThreshold>,
        accept: bool,
    ) -> Result<()> {
        // 1) expiry check
        let clock = Clock::get()?;
        let expiration = ctx.accounts.threshold_proposal.expiration;
        require!(clock.unix_timestamp <= expiration, ErrorCode::ProposalExpired);

        // 2) double‐voting check
        let staker_key = ctx.accounts.staker.key();
        let past_voters = &ctx.accounts.threshold_proposal.voters;
        require!(!past_voters.contains(&staker_key), ErrorCode::AlreadyVoted);

        // 3) determine weight
        let weight = ctx.accounts.staker_tokens.amount;

        // 4) lock tokens into escrow
        let cpi_ctx = ctx.accounts.into_transfer_to_escrow_context();
        token::transfer(cpi_ctx, weight)?;

        // 5) now mutably borrow the proposal
        let p = &mut ctx.accounts.threshold_proposal;
        if accept {
            p.yes_votes = p.yes_votes.checked_add(weight).unwrap();
        } else {
            p.no_votes = p.no_votes.checked_add(weight).unwrap();
        }
        p.voters.push(staker_key);

        emit!(Voted {
            basket: p.basket,
            kind: ProposalType::Threshold,
            voter: staker_key,
            weight,
            accept,
        });
        Ok(())
    }

    /// Finalize threshold if quorum & majority met before expiry.
    pub fn finalize_threshold(
        ctx: Context<FinalizeThreshold>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let cfg = &mut ctx.accounts.basket;
        let p = &mut ctx.accounts.threshold_proposal;

        require!(clock.unix_timestamp <= p.expiration, ErrorCode::ProposalExpired);
        let total_votes = p.yes_votes.checked_add(p.no_votes).unwrap();
        require!(
            total_votes.checked_mul(100).unwrap()
                >= p.snapshot_supply.checked_mul(p.quorum_percentage as u64).unwrap(),
            ErrorCode::QuorumNotReached
        );
        require!(p.yes_votes > p.no_votes, ErrorCode::NotApproved);

        cfg.threshold = p.proposed_threshold;
        emit!(ProposalFinalized {
            basket: cfg.key(),
            kind: ProposalType::Threshold,
            approved: true,
        });
        Ok(())
    }

    /// Create a strategy‐change proposal.
    pub fn propose_strategy(
        ctx: Context<ProposeStrategy>,
        new_strategy: u8,
        expiration_ts: i64,
    ) -> Result<()> {
        let cfg = &ctx.accounts.basket;
        let p = &mut ctx.accounts.strategy_proposal;
        p.proposer = ctx.accounts.proposer.key();
        p.basket = cfg.key();
        p.proposed_strategy = new_strategy;
        p.yes_votes = 0;
        p.no_votes = 0;
        p.snapshot_supply = ctx.accounts.rebal_mint.supply;
        p.quorum_percentage = cfg.quorum_percentage;
        p.expiration = expiration_ts;
        p.voters = Vec::new();
        emit!(ProposalCreated {
            basket: cfg.key(),
            kind: ProposalType::Strategy,
            proposer: p.proposer,
            expiration: p.expiration,
        });
        Ok(())
    }

    /// Vote on a strategy proposal.
    pub fn vote_strategy(
        ctx: Context<VoteStrategy>,
        accept: bool,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let expiration = ctx.accounts.strategy_proposal.expiration;
        require!(clock.unix_timestamp <= expiration, ErrorCode::ProposalExpired);

        let staker_key = ctx.accounts.staker.key();
        let past_voters = &ctx.accounts.strategy_proposal.voters;
        require!(!past_voters.contains(&staker_key), ErrorCode::AlreadyVoted);

        let weight = ctx.accounts.staker_tokens.amount;
        let cpi_ctx = ctx.accounts.into_transfer_to_escrow_context();
        token::transfer(cpi_ctx, weight)?;

        let p = &mut ctx.accounts.strategy_proposal;
        if accept {
            p.yes_votes = p.yes_votes.checked_add(weight).unwrap();
        } else {
            p.no_votes = p.no_votes.checked_add(weight).unwrap();
        }
        p.voters.push(staker_key);

        emit!(Voted {
            basket: p.basket,
            kind: ProposalType::Strategy,
            voter: staker_key,
            weight,
            accept,
        });
        Ok(())
    }

    /// Finalize strategy if quorum & majority met before expiry.
    pub fn finalize_strategy(
        ctx: Context<FinalizeStrategy>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let cfg = &mut ctx.accounts.basket;
        let p = &mut ctx.accounts.strategy_proposal;

        require!(clock.unix_timestamp <= p.expiration, ErrorCode::ProposalExpired);
        let total_votes = p.yes_votes.checked_add(p.no_votes).unwrap();
        require!(
            total_votes.checked_mul(100).unwrap()
                >= p.snapshot_supply.checked_mul(p.quorum_percentage as u64).unwrap(),
            ErrorCode::QuorumNotReached
        );
        require!(p.yes_votes > p.no_votes, ErrorCode::NotApproved);

        cfg.strategy = p.proposed_strategy;
        emit!(ProposalFinalized {
            basket: cfg.key(),
            kind: ProposalType::Strategy,
            approved: true,
        });
        Ok(())
    }

    /// Create an assets‐change proposal.
    pub fn propose_assets(
        ctx: Context<ProposeAssets>,
        new_assets: Vec<Pubkey>,
        expiration_ts: i64,
    ) -> Result<()> {
        let cfg = &ctx.accounts.basket;
        let p = &mut ctx.accounts.assets_proposal;
        p.proposer = ctx.accounts.proposer.key();
        p.basket = cfg.key();
        p.proposed_assets = new_assets;
        p.yes_votes = 0;
        p.no_votes = 0;
        p.snapshot_supply = ctx.accounts.rebal_mint.supply;
        p.quorum_percentage = cfg.quorum_percentage;
        p.expiration = expiration_ts;
        p.voters = Vec::new();
        emit!(ProposalCreated {
            basket: cfg.key(),
            kind: ProposalType::Assets,
            proposer: p.proposer,
            expiration: p.expiration,
        });
        Ok(())
    }

    /// Vote on an assets proposal.
    pub fn vote_assets(
        ctx: Context<VoteAssets>,
        accept: bool,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let expiration = ctx.accounts.assets_proposal.expiration;
        require!(clock.unix_timestamp <= expiration, ErrorCode::ProposalExpired);

        let staker_key = ctx.accounts.staker.key();
        let past_voters = &ctx.accounts.assets_proposal.voters;
        require!(!past_voters.contains(&staker_key), ErrorCode::AlreadyVoted);

        let weight = ctx.accounts.staker_tokens.amount;
        let cpi_ctx = ctx.accounts.into_transfer_to_escrow_context();
        token::transfer(cpi_ctx, weight)?;

        let p = &mut ctx.accounts.assets_proposal;
        if accept {
            p.yes_votes = p.yes_votes.checked_add(weight).unwrap();
        } else {
            p.no_votes = p.no_votes.checked_add(weight).unwrap();
        }
        p.voters.push(staker_key);

        emit!(Voted {
            basket: p.basket,
            kind: ProposalType::Assets,
            voter: staker_key,
            weight,
            accept,
        });
        Ok(())
    }

    /// Finalize assets if quorum & majority met before expiry.
    pub fn finalize_assets(
        ctx: Context<FinalizeAssets>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let cfg = &mut ctx.accounts.basket;
        let p = &mut ctx.accounts.assets_proposal;

        require!(clock.unix_timestamp <= p.expiration, ErrorCode::ProposalExpired);
        let total_votes = p.yes_votes.checked_add(p.no_votes).unwrap();
        require!(
            total_votes.checked_mul(100).unwrap()
                >= p.snapshot_supply.checked_mul(p.quorum_percentage as u64).unwrap(),
            ErrorCode::QuorumNotReached
        );
        require!(p.yes_votes > p.no_votes, ErrorCode::NotApproved);

        cfg.eligible_assets = p.proposed_assets.clone();
        emit!(ProposalFinalized {
            basket: cfg.key(),
            kind: ProposalType::Assets,
            approved: true,
        });
        Ok(())
    }

    /// Bot calls this after performing on‐chain rebalancing.
    pub fn execute_rebalance(
        ctx: Context<ExecuteRebalance>,
        current_deviation: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let cfg = &mut ctx.accounts.basket;

        // 1) Cooldown enforcement
        require!(
            clock.unix_timestamp
                .checked_sub(cfg.last_rebalance_ts)
                .unwrap()
                >= cfg.cooldown_seconds as i64,
            ErrorCode::CooldownActive
        );

        // 2) Whitelist check
        require!(
            cfg.whitelist.is_empty()
                || cfg.whitelist.contains(&ctx.accounts.bot_signer.key()),
            ErrorCode::NotWhitelisted
        );

        // 3) Dynamic reward calculation & slashing
        let mut reward_amount = cfg
            .base_reward
            .checked_mul(current_deviation)
            .unwrap()
            .checked_div(cfg.threshold)
            .unwrap();
        if current_deviation > cfg.threshold {
            reward_amount = reward_amount.checked_div(cfg.slash_factor).unwrap();
        }

        // 4) Mint via PDA authority
        let basket_key = cfg.key();
        let mint_bump = cfg.mint_auth_bump;
        let seeds = &[b"mint_auth", basket_key.as_ref(), &[mint_bump]];
        let signer_seeds = &[&seeds[..]];
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.rebal_mint.to_account_info(),
                    to: ctx.accounts.bot_token_account.to_account_info(),
                    authority: ctx.accounts.mint_auth.to_account_info(),
                },
                signer_seeds,
            ),
            reward_amount,
        )?;

        // 5) Lamport reimbursement
        let lamports_reward = cfg.lamports_reward;
        let fee_vault_bump = cfg.fee_vault_bump;
        let ix = system_instruction::transfer(
            &ctx.accounts.fee_vault.key(),
            &ctx.accounts.bot_signer.key(),
            lamports_reward,
        );
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.fee_vault.to_account_info(),
                ctx.accounts.bot_signer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[b"fee_vault", basket_key.as_ref(), &[fee_vault_bump]]],
        )?;

        // 6) Update timestamp & emit event (fixed variable name)
        cfg.last_rebalance_ts = clock.unix_timestamp;
        emit!(RebalanceExecuted {
            basket: cfg.key(),
            bot: ctx.accounts.bot_signer.key(),
            token_reward: reward_amount,
            lamport_reward: lamports_reward,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

// ─── Accounts ─────────────────────────────────────────────────────────────

#[account]
pub struct BasketConfig {
    pub initializer: Pubkey,
    pub name: String,
    pub description: String,
    pub rebal_mint: Pubkey,
    pub threshold: u64,
    pub strategy: u8,
    pub eligible_assets: Vec<Pubkey>,
    pub quorum_percentage: u8,
    pub cooldown_seconds: u64,
    pub base_reward: u64,
    pub lamports_reward: u64,
    pub slash_factor: u64,
    pub last_rebalance_ts: i64,
    pub whitelist: Vec<Pubkey>,
    pub mint_auth_bump: u8,
    pub fee_vault_bump: u8,
}

#[account]
pub struct ThresholdProposal {
    pub proposer: Pubkey,
    pub basket: Pubkey,
    pub proposed_threshold: u64,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub snapshot_supply: u64,
    pub quorum_percentage: u8,
    pub expiration: i64,
    pub voters: Vec<Pubkey>,
}

#[account]
pub struct StrategyProposal {
    pub proposer: Pubkey,
    pub basket: Pubkey,
    pub proposed_strategy: u8,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub snapshot_supply: u64,
    pub quorum_percentage: u8,
    pub expiration: i64,
    pub voters: Vec<Pubkey>,
}

#[account]
pub struct AssetsProposal {
    pub proposer: Pubkey,
    pub basket: Pubkey,
    pub proposed_assets: Vec<Pubkey>,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub snapshot_supply: u64,
    pub quorum_percentage: u8,
    pub expiration: i64,
    pub voters: Vec<Pubkey>,
}

// ─── Contexts ──────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeBasket<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + 32 + 4 + 64 + 4 + 256 + 1000)]
    pub basket: Account<'info, BasketConfig>,
    pub rebal_mint: Account<'info, Mint>,
    /// PDA (["mint_auth", basket]) with bump
    pub mint_auth: UncheckedAccount<'info>,
    /// PDA (["fee_vault", basket]) with bump
    pub fee_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProposeThreshold<'info> {
    #[account(mut)] pub proposer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    pub rebal_mint: Account<'info, Mint>,
    #[account(init, payer = proposer, space = 8 + 32*2 + 8*5 + 4 + 256)]
    pub threshold_proposal: Account<'info, ThresholdProposal>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct VoteThreshold<'info> {
    pub staker: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub threshold_proposal: Account<'info, ThresholdProposal>,
    #[account(mut, constraint = staker_tokens.mint == basket.rebal_mint)]
    pub staker_tokens: Account<'info, TokenAccount>,
    #[account(mut)] pub escrow: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> VoteThreshold<'info> {
    fn into_transfer_to_escrow_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.staker_tokens.to_account_info(),
            to: self.escrow.to_account_info(),
            authority: self.staker.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct FinalizeThreshold<'info> {
    #[account(mut)] pub finalizer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub threshold_proposal: Account<'info, ThresholdProposal>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ProposeStrategy<'info> {
    #[account(mut)] pub proposer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    pub rebal_mint: Account<'info, Mint>,
    #[account(init, payer = proposer, space = 8 + 32*2 + 8*5 + 4 + 256)]
    pub strategy_proposal: Account<'info, StrategyProposal>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct VoteStrategy<'info> {
    pub staker: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub strategy_proposal: Account<'info, StrategyProposal>,
    #[account(mut, constraint = staker_tokens.mint == basket.rebal_mint)]
    pub staker_tokens: Account<'info, TokenAccount>,
    #[account(mut)] pub escrow: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> VoteStrategy<'info> {
    fn into_transfer_to_escrow_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.staker_tokens.to_account_info(),
            to: self.escrow.to_account_info(),
            authority: self.staker.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct FinalizeStrategy<'info> {
    #[account(mut)] pub finalizer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub strategy_proposal: Account<'info, StrategyProposal>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ProposeAssets<'info> {
    #[account(mut)] pub proposer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    pub rebal_mint: Account<'info, Mint>,
    #[account(init, payer = proposer, space = 8 + 32*2 + 8*2 + 4 + 512)]
    pub assets_proposal: Account<'info, AssetsProposal>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct VoteAssets<'info> {
    pub staker: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub assets_proposal: Account<'info, AssetsProposal>,
    #[account(mut, constraint = staker_tokens.mint == basket.rebal_mint)]
    pub staker_tokens: Account<'info, TokenAccount>,
    #[account(mut)] pub escrow: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> VoteAssets<'info> {
    fn into_transfer_to_escrow_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.staker_tokens.to_account_info(),
            to: self.escrow.to_account_info(),
            authority: self.staker.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct FinalizeAssets<'info> {
    #[account(mut)] pub finalizer: Signer<'info>,
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, has_one = basket)]
    pub assets_proposal: Account<'info, AssetsProposal>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ExecuteRebalance<'info> {
    #[account(mut)] pub basket: Account<'info, BasketConfig>,
    #[account(mut, constraint = rebal_mint.key() == basket.rebal_mint)]
    pub rebal_mint: Account<'info, Mint>,
    #[account(seeds = [b"mint_auth", basket.key().as_ref()], bump = basket.mint_auth_bump)]
    pub mint_auth: UncheckedAccount<'info>,
    #[account(mut)] pub bot_token_account: Account<'info, TokenAccount>,
    pub bot_signer: Signer<'info>,
    #[account(mut, seeds = [b"fee_vault", basket.key().as_ref()], bump = basket.fee_vault_bump)]
    pub fee_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

// ─── Events & Errors ───────────────────────────────────────────────────────

#[event]
pub struct ProposalCreated {
    pub basket: Pubkey,
    pub kind: ProposalType,
    pub proposer: Pubkey,
    pub expiration: i64,
}

#[event]
pub struct Voted {
    pub basket: Pubkey,
    pub kind: ProposalType,
    pub voter: Pubkey,
    pub weight: u64,
    pub accept: bool,
}

#[event]
pub struct ProposalFinalized {
    pub basket: Pubkey,
    pub kind: ProposalType,
    pub approved: bool,
}

#[event]
pub struct RebalanceExecuted {
    pub basket: Pubkey,
    pub bot: Pubkey,
    pub token_reward: u64,
    pub lamport_reward: u64,
    pub timestamp: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum ProposalType {
    Threshold,
    Strategy,
    Assets,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Proposal did not receive enough yes votes")] NotApproved,
    #[msg("Proposal expired")] ProposalExpired,
    #[msg("Quorum not reached")] QuorumNotReached,
    #[msg("Already voted")] AlreadyVoted,
    #[msg("Cooldown still active")] CooldownActive,
    #[msg("Bot not whitelisted")] NotWhitelisted,
    #[msg("Proposal does not belong to this basket")] BadBasket,
}
