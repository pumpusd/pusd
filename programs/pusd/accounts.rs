use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

/// Program-wide configuration and parameters.
/// PDA seed: ["protocol", authority]
#[account]
pub struct Protocol {
    /// Governance / admin authority (e.g., multisig)
    pub authority: Pubkey,

    /// Bump for the Protocol PDA
    pub bump: u8,

    /// PUSD mint address (SPL Token-2022 or Token)
    pub pusd_mint: Pubkey,

    /// Global debt ceiling in PUSD (6 decimals typical)
    pub global_debt_ceiling: u64,

    /// Whether minting is paused (emergency circuit breaker)
    pub mint_paused: bool,

    /// Reserved for future upgrades (alignment/padding)
    pub _reserved: [u8; 7],
}
impl Protocol {
    pub const SEED_PREFIX: &'static [u8] = b"protocol";
    pub const LEN: usize = 8  // discriminator
        + 32  // authority
        + 1   // bump
        + 32  // pusd_mint
        + 8   // global_debt_ceiling
        + 1   // mint_paused
        + 7;  // _reserved
}

/// Supported collateral configuration (one per asset).
/// PDA seed: ["collateral", protocol, collateral_mint]
#[account]
pub struct CollateralConfig {
    /// Protocol this collateral belongs to
    pub protocol: Pubkey,

    /// Collateral SPL mint
    pub collateral_mint: Pubkey,

    /// Vault token account (owned by PDA) where collateral is held
    pub vault: Pubkey,

    /// Initial LTV (bps). e.g., 6600 = 66.00%
    pub initial_ltv_bps: u16,

    /// Maintenance LTV (bps). Liquidation if health falls below this.
    pub maintenance_ltv_bps: u16,

    /// Liquidation bonus (bps). e.g., 500 = 5.00%
    pub liq_bonus_bps: u16,

    /// Per-collateral debt ceiling in PUSD (limits concentration)
    pub debt_ceiling: u64,

    /// Active flag (false disables new mints for this collateral)
    pub active: bool,

    /// Bump for the CollateralConfig PDA
    pub bump: u8,

    /// Reserved
    pub _reserved: [u8; 6],
}
impl CollateralConfig {
    pub const SEED_PREFIX: &'static [u8] = b"collateral";
    pub const LEN: usize = 8
        + 32  // protocol
        + 32  // collateral_mint
        + 32  // vault
        + 2   // initial_ltv_bps
        + 2   // maintenance_ltv_bps
        + 2   // liq_bonus_bps
        + 8   // debt_ceiling
        + 1   // active
        + 1   // bump
        + 6;  // _reserved
}

/// User position per collateral mint (simple 1:1 model).
/// PDA seed: ["position", owner, collateral_mint]
#[account]
pub struct Position {
    /// Position owner
    pub owner: Pubkey,

    /// The collateral config this position references
    pub collateral_config: Pubkey,

    /// Amount of collateral deposited (in smallest units)
    pub collateral_amount: u64,

    /// PUSD debt (principal). Fees can be handled off-chain and realized on actions.
    pub debt_pusd: u64,

    /// Last time (Unix ts) interest/fees were realized (optional use)
    pub last_accrual_ts: i64,

    /// Bump for the Position PDA
    pub bump: u8,

    /// Reserved
    pub _reserved: [u8; 7],
}
impl Position {
    pub const SEED_PREFIX: &'static [u8] = b"position";
    pub const LEN: usize = 8
        + 32  // owner
        + 32  // collateral_config
        + 8   // collateral_amount
        + 8   // debt_pusd
        + 8   // last_accrual_ts
        + 1   // bump
        + 7;  // _reserved
}

/// Governance account holding timelock and upgrade controls (optional).
/// PDA seed: ["governance", protocol]
#[account]
pub struct Governance {
    /// The protocol this governance controls
    pub protocol: Pubkey,

    /// Authority with power to queue/execute parameter changes
    pub authority: Pubkey,

    /// Timelock delay in seconds for critical changes
    pub timelock_secs: u64,

    /// Bump for the Governance PDA
    pub bump: u8,

    /// Reserved
    pub _reserved: [u8; 7],
}
impl Governance {
    pub const SEED_PREFIX: &'static [u8] = b"governance";
    pub const LEN: usize = 8
        + 32 // protocol
        + 32 // authority
        + 8  // timelock_secs
        + 1  // bump
        + 7; // _reserved
}

/// Helper: checked math for LTV (basis points).
pub fn compute_health_bps(
    collateral_value_usd_6dp: u128, // value of collateral in 6-decimal USD
    debt_pusd_6dp: u128,            // debt in 6-decimal PUSD
) -> u128 {
    if debt_pusd_6dp == 0 {
        return u128::MAX; // Infinite health when no debt
    }
    // health_bps = (collateral_value / debt) * 10000
    collateral_value_usd_6dp
        .saturating_mul(10_000)
        .saturating_div(debt_pusd_6dp)
}

/// Seeds helpers to be reused in instructions
pub fn protocol_seeds<'a>(authority: &'a Pubkey, bump: u8) -> [&'a [u8]; 3] {
    [Protocol::SEED_PREFIX, authority.as_ref(), &[bump]]
}

pub fn collateral_seeds<'a>(
    protocol: &'a Pubkey,
    collateral_mint: &'a Pubkey,
    bump: u8,
) -> [&'a [u8]; 4] {
    [
        CollateralConfig::SEED_PREFIX,
        protocol.as_ref(),
        collateral_mint.as_ref(),
        &[bump],
    ]
}

pub fn position_seeds<'a>(
    owner: &'a Pubkey,
    collateral_mint: &'a Pubkey,
    bump: u8,
) -> [&'a [u8]; 4] {
    [Position::SEED_PREFIX, owner.as_ref(), collateral_mint.as_ref(), &[bump]]
}

pub fn governance_seeds<'a>(protocol: &'a Pubkey, bump: u8) -> [&'a [u8]; 3] {
    [Governance::SEED_PREFIX, protocol.as_ref(), &[bump]]
}

