use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Burn, Mint, MintTo, Token, TokenAccount, Transfer,
};

use crate::accounts::*;
use crate::errors::ErrorCode;
use crate::state::{
    apply_liquidation_bonus_bps, check_mint_within_initial_ltv, compute_health_bps,
    is_above_maintenance, token_amount_to_usd_6dp, CollateralAdded, Initialized, Liquidated,
    Minted, PauseToggled, Burned, ParameterUpdated, PUSD_DECIMALS,
};

/// ===============================
/// Handlers
/// ===============================

/// Initialize the protocol and bind the PUSD mint (mint authority must be the Protocol PDA).
pub fn handle_initialize(ctx: Context<Initialize>, global_debt_ceiling: u64) -> Result<()> {
    let protocol = &mut ctx.accounts.protocol;

    protocol.authority = ctx.accounts.authority.key();
    protocol.bump = *ctx.bumps.get("protocol").ok_or(ErrorCode::InvalidPda)?;
    protocol.pusd_mint = ctx.accounts.pusd_mint.key();
    protocol.global_debt_ceiling = global_debt_ceiling;
    protocol.mint_paused = false;

    // Sanity: The PUSD mint authority must be the protocol PDA
    require!(
        ctx.accounts.pusd_mint.mint_authority == COption::Some(protocol.key()),
        ErrorCode::Unauthorized
    );

    emit!(Initialized {
        protocol: protocol.key(),
        pusd_mint: protocol.pusd_mint,
        authority: protocol.authority,
        global_debt_ceiling
    });

    Ok(())
}

/// Register a collateral type and its vault.
pub fn handle_add_collateral(
    ctx: Context<AddCollateral>,
    initial_ltv_bps: u16,
    maintenance_ltv_bps: u16,
    liq_bonus_bps: u16,
    debt_ceiling: u64,
    active: bool,
) -> Result<()> {
    // Basic param checks
    require!(initial_ltv_bps > 0 && maintenance_ltv_bps > 0, ErrorCode::InvalidParameter);
    require!(maintenance_ltv_bps <= initial_ltv_bps, ErrorCode::InvalidParameter);

    let cfg = &mut ctx.accounts.collateral_config;
    cfg.protocol = ctx.accounts.protocol.key();
    cfg.collateral_mint = ctx.accounts.collateral_mint.key();
    cfg.vault = ctx.accounts.vault.key();
    cfg.initial_ltv_bps = initial_ltv_bps;
    cfg.maintenance_ltv_bps = maintenance_ltv_bps;
    cfg.liq_bonus_bps = liq_bonus_bps;
    cfg.debt_ceiling = debt_ceiling;
    cfg.active = active;
    cfg.bump = *ctx.bumps.get("collateral_config").ok_or(ErrorCode::InvalidPda)?;

    emit!(CollateralAdded {
        protocol: cfg.protocol,
        collateral_mint: cfg.collateral_mint,
        vault: cfg.vault,
        initial_ltv_bps,
        maintenance_ltv_bps,
        liq_bonus_bps,
        debt_ceiling,
    });

    Ok(())
}

/// Create (if not exists) or fund a position by depositing collateral.
pub fn handle_open_or_fund_position(
    ctx: Context<OpenOrFundPosition>,
    deposit_amount: u64,
) -> Result<()> {
    require!(deposit_amount > 0, ErrorCode::ZeroAmount);
    let cfg = &ctx.accounts.collateral_config;
    require!(cfg.active, ErrorCode::CollateralInactive);

    // Transfer collateral from user to protocol vault
    let cpi_accounts = Transfer {
        from: ctx.accounts.user_collateral_ata.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, deposit_amount)?;

    // Initialize or update position
    let pos = &mut ctx.accounts.position;
    if pos.owner == Pubkey::default() {
        pos.owner = ctx.accounts.owner.key();
        pos.collateral_config = cfg.key();
        pos.collateral_amount = deposit_amount;
        pos.debt_pusd = 0;
        pos.last_accrual_ts = Clock::get()?.unix_timestamp;
        pos.bump = *ctx.bumps.get("position").ok_or(ErrorCode::InvalidPda)?;
    } else {
        require!(pos.owner == ctx.accounts.owner.key(), ErrorCode::Unauthorized);
        pos.collateral_amount = pos
            .collateral_amount
            .checked_add(deposit_amount)
            .ok_or(ErrorCode::MathOverflow)?;
    }

    Ok(())
}

/// Mint PUSD against deposited collateral.
/// TEMP: we pass `collateral_price_usd_6dp` (price of 1 whole collateral token in 6dp USD)
/// and `collateral_decimals` until oracle wiring is added.
pub fn handle_mint(
    ctx: Context<MintPusd>,
    mint_pusd_6dp: u64,
    collateral_price_usd_6dp: u128,
    collateral_decimals: u8,
) -> Result<()> {
    require!(mint_pusd_6dp > 0, ErrorCode::ZeroAmount);

    let protocol = &mut ctx.accounts.protocol;
    require!(!protocol.mint_paused, ErrorCode::MintPaused);

    let cfg = &ctx.accounts.collateral_config;
    require!(cfg.active, ErrorCode::CollateralInactive);
    require!(cfg.vault == ctx.accounts.vault.key(), ErrorCode::VaultMismatch);

    // Position must belong to owner and this collateral
    let pos = &mut ctx.accounts.position;
    require!(pos.owner == ctx.accounts.owner.key(), ErrorCode::Unauthorized);
    require!(pos.collateral_config == cfg.key(), ErrorCode::UnsupportedCollateral);

    // Compute collateral value and check LTV
    let collateral_value_6dp = token_amount_to_usd_6dp(
        pos.collateral_amount,
        collateral_decimals,
        collateral_price_usd_6dp,
    )
    .ok_or(ErrorCode::MathOverflow)?;

    let current_debt = pos.debt_pusd as u128;
    let mint_delta = mint_pusd_6dp as u128;

    let within_ltv = check_mint_within_initial_ltv(
        collateral_value_6dp,
        current_debt,
        mint_delta,
        cfg.initial_ltv_bps,
    );
    require!(within_ltv, ErrorCode::LtvExceeded);

    // Global (and per-collateral) debt checks – simplified: compare against protocol global ceiling
    let new_total_debt = (protocol.global_debt_ceiling as u128)
        .checked_sub(0)
        .ok_or(ErrorCode::MathUnderflow)?; // placeholder (you will track aggregate debt separately)
    let _ = new_total_debt; // silence warning for now

    // Mint PUSD to the user (PUSD mint authority must be the Protocol PDA)
    let seeds = [
        Protocol::SEED_PREFIX,
        protocol.authority.as_ref(),
        &[protocol.bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = MintTo {
        mint: ctx.accounts.pusd_mint.to_account_info(),
        to: ctx.accounts.user_pusd_ata.to_account_info(),
        authority: ctx.accounts.protocol.to_account_info(),
    };
    let cpi_ctx =
        CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
    token::mint_to(cpi_ctx, mint_pusd_6dp)?;

    // Update position debt
    pos.debt_pusd = pos
        .debt_pusd
        .checked_add(mint_pusd_6dp)
        .ok_or(ErrorCode::MathOverflow)?;
    pos.last_accrual_ts = Clock::get()?.unix_timestamp;

    emit!(Minted {
        owner: pos.owner,
        collateral_mint: cfg.collateral_mint,
        minted_pusd_6dp: mint_pusd_6dp,
        new_debt_pusd_6dp: pos.debt_pusd,
    });

    Ok(())
}

/// Burn/repay PUSD and (optionally) withdraw collateral later.
/// Here we only burn; withdrawals can be a separate instruction.
pub fn handle_burn(ctx: Context<BurnPusd>, burn_pusd_6dp: u64) -> Result<()> {
    require!(burn_pusd_6dp > 0, ErrorCode::ZeroAmount);

    // Burn PUSD from user
    let cpi_accounts = Burn {
        mint: ctx.accounts.pusd_mint.to_account_info(),
        from: ctx.accounts.user_pusd_ata.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::burn(cpi_ctx, burn_pusd_6dp)?;

    // Decrease position debt
    let pos = &mut ctx.accounts.position;
    require!(pos.owner == ctx.accounts.owner.key(), ErrorCode::Unauthorized);

    pos.debt_pusd = pos
        .debt_pusd
        .checked_sub(burn_pusd_6dp)
        .ok_or(ErrorCode::MathUnderflow)?;
    pos.last_accrual_ts = Clock::get()?.unix_timestamp;

    emit!(Burned {
        owner: pos.owner,
        collateral_mint: ctx.accounts.collateral_config.collateral_mint,
        burned_pusd_6dp: burn_pusd_6dp,
        new_debt_pusd_6dp: pos.debt_pusd,
    });

    Ok(())
}

/// Liquidate an unhealthy position by repaying PUSD in exchange for discounted collateral.
/// TEMP: pass `collateral_price_usd_6dp` and `collateral_decimals` until oracles are wired.
pub fn handle_liquidate(
    ctx: Context<Liquidate>,
    repay_pusd_6dp: u64,
    collateral_price_usd_6dp: u128,
    collateral_decimals: u8,
) -> Result<()> {
    require!(repay_pusd_6dp > 0, ErrorCode::ZeroAmount);

    let cfg = &ctx.accounts.collateral_config;
    let pos = &mut ctx.accounts.position;

    // Check liquidatability: health < maintenance
    let collateral_value_6dp = token_amount_to_usd_6dp(
        pos.collateral_amount,
        collateral_decimals,
        collateral_price_usd_6dp,
    )
    .ok_or(ErrorCode::MathOverflow)?;

    let healthy = is_above_maintenance(
        collateral_value_6dp,
        pos.debt_pusd as u128,
        cfg.maintenance_ltv_bps,
    );
    require!(!healthy, ErrorCode::NotLiquidatable);

    // Cap repay to current debt
    let repay = repay_pusd_6dp.min(pos.debt_pusd);

    // Burn PUSD from liquidator first (repay debt)
    let cpi_burn = Burn {
        mint: ctx.accounts.pusd_mint.to_account_info(),
        from: ctx.accounts.liquidator_pusd_ata.to_account_info(),
        authority: ctx.accounts.liquidator.to_account_info(),
    };
    let cpi_ctx_burn = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_burn);
    token::burn(cpi_ctx_burn, repay)?;

    // Determine collateral to seize with liquidation bonus
    // price_6dp = USD per 1 whole token; convert repay USD into token amount (inverse pricing).
    // tokens = (repay_6dp / price_6dp) * 10^decimals
    let price_6dp = collateral_price_usd_6dp;
    require!(price_6dp > 0, ErrorCode::PriceOutOfBounds);

    let repay_u128 = repay as u128;
    let num = repay_u128
        .checked_mul(crate::state::ten_pow_u128(collateral_decimals as u32))
        .ok_or(ErrorCode::MathOverflow)?;
    let base_tokens = num.checked_div(price_6dp).ok_or(ErrorCode::DivisionByZero)?;
    let seized_with_bonus =
        apply_liquidation_bonus_bps(base_tokens, cfg.liq_bonus_bps).ok_or(ErrorCode::MathOverflow)?;

    let seize_amount: u64 = seized_with_bonus
        .try_into()
        .map_err(|_| ErrorCode::InvalidAmount)?;

    // Bound by available collateral
    let seize_amount = seize_amount.min(pos.collateral_amount);
    require!(seize_amount > 0, ErrorCode::InvalidAmount);

    // Transfer collateral from vault to liquidator
    let seeds = [
        CollateralConfig::SEED_PREFIX,
        cfg.protocol.as_ref(),
        cfg.collateral_mint.as_ref(),
        &[cfg.bump],
    ];
    // NOTE: vault is owned by the program (via PDA authority). If vault authority is Protocol PDA,
    // change seeds accordingly. For now we assume vault authority is the CollateralConfig PDA.
    let signer = &[&seeds[..]];

    let cpi_transfer = Transfer {
        from: ctx.accounts.vault.to_account_info(),
        to: ctx.accounts.liquidator_collateral_ata.to_account_info(),
        authority: ctx.accounts.collateral_config.to_account_info(),
    };
    let cpi_ctx_transfer =
        CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_transfer, signer);
    token::transfer(cpi_ctx_transfer, seize_amount)?;

    // Update position
    pos.collateral_amount = pos
        .collateral_amount
        .checked_sub(seize_amount)
        .ok_or(ErrorCode::MathUnderflow)?;
    pos.debt_pusd = pos.debt_pusd.checked_sub(repay).ok_or(ErrorCode::MathUnderflow)?;

    emit!(Liquidated {
        liquidator: ctx.accounts.liquidator.key(),
        owner: pos.owner,
        collateral_mint: cfg.collateral_mint,
        repaid_pusd_6dp: repay,
        seized_collateral_amount: seize_amount,
    });

    Ok(())
}

/// Pause/unpause minting (emergency circuit breaker).
pub fn handle_toggle_pause(ctx: Context<TogglePause>, paused: bool) -> Result<()> {
    let protocol = &mut ctx.accounts.protocol;
    require_keys_eq!(protocol.authority, ctx.accounts.authority.key(), ErrorCode::Unauthorized);
    protocol.mint_paused = paused;

    emit!(PauseToggled {
        protocol: protocol.key(),
        paused,
    });

    Ok(())
}

/// ===============================
/// Accounts
/// ===============================

#[derive(Accounts)]
#[instruction(global_debt_ceiling: u64)]
pub struct Initialize<'info> {
    /// CHECK: PDA derived inside program; stored & used as signer for minting
    #[account(
        init,
        payer = authority,
        space = Protocol::LEN,
        seeds = [Protocol::SEED_PREFIX, authority.key().as_ref()],
        bump
    )]
    pub protocol: Account<'info, Protocol>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub pusd_mint: Account<'info, Mint>,

    /// System / SPL
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AddCollateral<'info> {
    #[account(mut, has_one = authority @ ErrorCode::Unauthorized)]
    pub protocol: Account<'info, Protocol>,
    pub authority: Signer<'info>,

    /// Collateral mint we are registering
    pub collateral_mint: Account<'info, Mint>,

    /// Collateral vault (SPL token account) owned by program PDA
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    /// New PDA to store config for this collateral
    #[account(
        init,
        payer = authority,
        space = CollateralConfig::LEN,
        seeds = [
            CollateralConfig::SEED_PREFIX,
            protocol.key().as_ref(),
            collateral_mint.key().as_ref()
        ],
        bump
    )]
    pub collateral_config: Account<'info, CollateralConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OpenOrFundPosition<'info> {
    #[account(mut)]
    pub protocol: Account<'info, Protocol>,

    pub collateral_mint: Account<'info, Mint>,

    #[account(
        has_one = protocol @ ErrorCode::Unauthorized,
        constraint = collateral_config.collateral_mint == collateral_mint.key() @ ErrorCode::MintMismatch
    )]
    pub collateral_config: Account<'info, CollateralConfig>,

    /// User's token account for the collateral they are depositing
    #[account(mut)]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    /// Protocol vault for the collateral (must match config)
    #[account(mut, constraint = vault.key() == collateral_config.vault @ ErrorCode::VaultMismatch)]
    pub vault: Account<'info, TokenAccount>,

    /// Position PDA for this (owner, collateral_mint)
    #[account(
        init_if_needed,
        payer = owner,
        space = Position::LEN,
        seeds = [Position::SEED_PREFIX, owner.key().as_ref(), collateral_mint.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct MintPusd<'info> {
    #[account(mut)]
    pub protocol: Account<'info, Protocol>,

    #[account(mut)]
    pub pusd_mint: Account<'info, Mint>,

    #[account(
        has_one = protocol @ ErrorCode::Unauthorized,
        constraint = collateral_config.vault == vault.key() @ ErrorCode::VaultMismatch
    )]
    pub collateral_config: Account<'info, CollateralConfig>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [Position::SEED_PREFIX, owner.key().as_ref(), collateral_config.collateral_mint.as_ref()],
        bump = position.bump
    )]
    pub position: Account<'info, Position>,

    #[account(mut)]
    pub user_pusd_ata: Account<'info, TokenAccount>,

    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BurnPusd<'info> {
    #[account(mut)]
    pub protocol: Account<'info, Protocol>,

    #[account(mut)]
    pub pusd_mint: Account<'info, Mint>,

    #[account(
        has_one = protocol @ ErrorCode::Unauthorized
    )]
    pub collateral_config: Account<'info, CollateralConfig>,

    #[account(
        mut,
        seeds = [Position::SEED_PREFIX, owner.key().as_ref(), collateral_config.collateral_mint.as_ref()],
        bump = position.bump
    )]
    pub position: Account<'info, Position>,

    #[account(mut)]
    pub user_pusd_ata: Account<'info, TokenAccount>,

    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub protocol: Account<'info, Protocol>,

    #[account(mut)]
    pub pusd_mint: Account<'info, Mint>,

    #[account(
        has_one = protocol @ ErrorCode::Unauthorized,
        constraint = collateral_config.vault == vault.key() @ ErrorCode::VaultMismatch
    )]
    pub collateral_config: Account<'info, CollateralConfig>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [Position::SEED_PREFIX, owner.key().as_ref(), collateral_config.collateral_mint.as_ref()],
        bump = position.bump
    )]
    pub position: Account<'info, Position>,

    /// Liquidator burns their PUSD to repay debt
    #[account(mut)]
    pub liquidator_pusd_ata: Account<'info, TokenAccount>,

    /// Liquidator receives seized collateral here
    #[account(mut)]
    pub liquidator_collateral_ata: Account<'info, TokenAccount>,

    /// The owner of the liquidated position (read-only for seeds)
    /// CHECK: provided to derive the position PDA seed above
    pub owner: UncheckedAccount<'info>,

    pub liquidator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct TogglePause<'info> {
    #[account(mut)]
    pub protocol: Account<'info, Protocol>,
    pub authority: Signer<'info>,
}

