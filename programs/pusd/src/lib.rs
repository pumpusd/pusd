use anchor_lang::prelude::*;

pub mod accounts;
pub mod errors;
pub mod instructions;
pub mod state;

use crate::instructions::{
    handle_add_collateral, handle_burn, handle_initialize, handle_liquidate, handle_mint,
    handle_open_or_fund_position, handle_toggle_pause, AddCollateral, BurnPusd, Initialize,
    Liquidate, MintPusd, OpenOrFundPosition, TogglePause,
};

declare_id!("PUSD111111111111111111111111111111111111111");

#[program]
pub mod pusd {
    use super::*;

    /// Initialize the protocol and bind the PUSD mint.
    pub fn initialize(ctx: Context<Initialize>, global_debt_ceiling: u64) -> Result<()> {
        handle_initialize(ctx, global_debt_ceiling)
    }

    /// Register a collateral type and its vault.
    pub fn add_collateral(
        ctx: Context<AddCollateral>,
        initial_ltv_bps: u16,
        maintenance_ltv_bps: u16,
        liq_bonus_bps: u16,
        debt_ceiling: u64,
        active: bool,
    ) -> Result<()> {
        handle_add_collateral(
            ctx,
            initial_ltv_bps,
            maintenance_ltv_bps,
            liq_bonus_bps,
            debt_ceiling,
            active,
        )
    }

    /// Create (if needed) or fund a position by depositing collateral.
    pub fn open_or_fund_position(
        ctx: Context<OpenOrFundPosition>,
        deposit_amount: u64,
    ) -> Result<()> {
        handle_open_or_fund_position(ctx, deposit_amount)
    }

    /// Mint PUSD against deposited collateral.
    /// TEMPORARY: price & decimals passed in until oracles are wired.
    pub fn mint_pusd(
        ctx: Context<MintPusd>,
        mint_pusd_6dp: u64,
        collateral_price_usd_6dp: u128,
        collateral_decimals: u8,
    ) -> Result<()> {
        handle_mint(
            ctx,
            mint_pusd_6dp,
            collateral_price_usd_6dp,
            collateral_decimals,
        )
    }

    /// Burn/repay PUSD (withdrawal can be a separate instruction).
    pub fn burn_pusd(ctx: Context<BurnPusd>, burn_pusd_6dp: u64) -> Result<()> {
        handle_burn(ctx, burn_pusd_6dp)
    }

    /// Liquidate an unhealthy position.
    /// TEMPORARY: price & decimals passed in until oracles are wired.
    pub fn liquidate(
        ctx: Context<Liquidate>,
        repay_pusd_6dp: u64,
        collateral_price_usd_6dp: u128,
        collateral_decimals: u8,
    ) -> Result<()> {
        handle_liquidate(
            ctx,
            repay_pusd_6dp,
            collateral_price_usd_6dp,
            collateral_decimals,
        )
    }

    /// Emergency pause/unpause minting.
    pub fn toggle_pause(ctx: Context<TogglePause>, paused: bool) -> Result<()> {
        handle_toggle_pause(ctx, paused)
    }
}

