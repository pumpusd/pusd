use anchor_lang::prelude::*;

/// ===== Constants & Fixed-Point Conventions =====
/// PumpUSD uses 6-decimal fixed point for USD amounts (like USDC).
pub const USD_DECIMALS: u32 = 6;
pub const PUSD_DECIMALS: u32 = 6;

/// Basis points (bps): 10_000 = 100.00%
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Maximum staleness allowed for oracle prices (example; enforce in ix)
pub const DEFAULT_MAX_ORACLE_STALENESS_SECS: i64 = 90;

/// Safe ceiling used when converting/scaling to avoid accidental overflow
pub const U64_MAX_AS_U128: u128 = u64::MAX as u128;

/// ===== Oracle Price Model (generic) =====
/// Lightweight struct compatible with common oracle shapes (e.g., Pyth-like).
/// Store raw price with an exponent (price = price * 10^expo).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct OraclePrice {
    /// e.g., if price = 23.45 USD, stored as price=2345, expo=-2
    pub price: i64,
    /// exponent base-10 for the price
    pub expo: i32,
    /// confidence interval (same exponent as price); optional use
    pub conf: u64,
    /// unix timestamp (seconds) when the price was published
    pub publish_time: i64,
}

impl OraclePrice {
    /// Returns true if price is considered fresh under the provided max_age.
    pub fn is_fresh(&self, now_ts: i64, max_age_secs: i64) -> bool {
        if self.publish_time <= 0 {
            return false;
        }
        now_ts.saturating_sub(self.publish_time) <= max_age_secs
    }

    /// Convert the oracle price to a 6-decimal USD fixed-point (u128).
    /// Returns (value in 6dp, ok flag). If conversion fails or price <= 0, ok=false.
    pub fn to_usd_6dp(&self) -> (u128, bool) {
        if self.price <= 0 {
            return (0, false);
        }
        // Convert i64 price * 10^expo to 6-decimals.
        // value_6dp = price * 10^(expo + 6)
        let price_i128 = self.price as i128;
        let target_exp = self.expo as i128 + PUSD_DECIMALS as i128;

        if target_exp >= 0 {
            // multiply by 10^target_exp
            let pow = ten_pow_i128(target_exp as u32);
            // price_i128 * pow should be non-negative here
            let v = price_i128.checked_mul(pow as i128);
            match v {
                Some(x) if x >= 0 => (x as u128, true),
                _ => (0, false),
            }
        } else {
            // divide by 10^(-target_exp)
            let pow = ten_pow_i128((-target_exp) as u32);
            if pow == 0 {
                return (0, false);
            }
            if price_i128 <= 0 {
                return (0, false);
            }
            ((price_i128 as u128).checked_div(pow as u128).unwrap_or(0), true)
        }
    }
}

/// ===== Math Helpers =====

/// 10^p for small p (<= 19 fits in i128/u128 comfortably).
fn ten_pow_u128(p: u32) -> u128 {
    const POWS: [u128; 20] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
        10_000_000_000_000_000_000,
    ];
    if (p as usize) < POWS.len() {
        POWS[p as usize]
    } else {
        // Fallback (shouldn't be used with realistic exponents in DeFi pricing)
        let mut v: u128 = 1;
        for _ in 0..p {
            v = v.saturating_mul(10);
        }
        v
    }
}

fn ten_pow_i128(p: u32) -> i128 {
    // reuse u128 pow then cast; safe for small p
    ten_pow_u128(p) as i128
}

/// Scale a token amount to 6-decimals USD using price and the token mint decimals.
/// amount: u64 (token smallest units)
/// token_decimals: decimals of the token mint (0..=9 typical)
/// price_6dp: u128 (USD in 6dp per 1 whole token)
pub fn token_amount_to_usd_6dp(amount: u64, token_decimals: u8, price_6dp: u128) -> Option<u128> {
    // value = amount * price / 10^token_decimals, all in integers
    let amt_u128 = amount as u128;
    let denom = ten_pow_u128(token_decimals as u32);
    if denom == 0 {
        return None;
    }
    amt_u128
        .checked_mul(price_6dp)?
        .checked_div(denom)
}

/// Compute health in basis points: (collateral_value / debt) * 10_000
/// If debt == 0, returns u128::MAX (infinite health).
pub fn compute_health_bps(collateral_value_usd_6dp: u128, debt_pusd_6dp: u128) -> u128 {
    if debt_pusd_6dp == 0 {
        return u128::MAX;
    }
    collateral_value_usd_6dp
        .saturating_mul(BPS_DENOMINATOR)
        .saturating_div(debt_pusd_6dp)
}

/// Check whether minting an additional `mint_delta_pusd_6dp` would exceed initial LTV.
/// Returns true if within limit, false if it would exceed.
/// LTV_bps is initial LTV (e.g., 6600 = 66.00%)
pub fn check_mint_within_initial_ltv(
    collateral_value_usd_6dp: u128,
    current_debt_pusd_6dp: u128,
    mint_delta_pusd_6dp: u128,
    initial_ltv_bps: u16,
) -> bool {
    let new_debt = current_debt_pusd_6dp.saturating_add(mint_delta_pusd_6dp);
    // LTV = debt / collateral
    if collateral_value_usd_6dp == 0 {
        return false;
    }
    // debt * 10_000 <= collateral * LTV_bps
    new_debt
        .saturating_mul(BPS_DENOMINATOR)
        <= collateral_value_usd_6dp.saturating_mul(initial_ltv_bps as u128)
}

/// Check whether position health is above maintenance (i.e., not liquidatable).
/// Returns true if healthy: health_bps >= maintenance_ltv_bps
pub fn is_above_maintenance(
    collateral_value_usd_6dp: u128,
    debt_pusd_6dp: u128,
    maintenance_ltv_bps: u16,
) -> bool {
    // health_bps = (collateral/debt)*10_000; compare with maintenance threshold
    let health_bps = compute_health_bps(collateral_value_usd_6dp, debt_pusd_6dp);
    health_bps >= maintenance_ltv_bps as u128
}

/// Apply a liquidation bonus to the seized collateral.
/// If bonus_bps = 500 (5%), the keeper pays debt * (1 - discount) and receives
/// collateral accordingly. This helper returns the multiplier (1 + bonus_bps).
pub fn apply_liquidation_bonus_bps(amount: u128, bonus_bps: u16) -> Option<u128> {
    // seized = amount * (1 + bonus_bps/10_000)
    let num = BPS_DENOMINATOR.saturating_add(bonus_bps as u128);
    amount.checked_mul(num)?.checked_div(BPS_DENOMINATOR)
}

/// ===== Events =====
/// Emit these from your instructions for better indexing/analytics UX.

#[event]
pub struct Initialized {
    /// Protocol PDA
    pub protocol: Pubkey,
    /// PUSD mint
    pub pusd_mint: Pubkey,
    /// Authority (governance)
    pub authority: Pubkey,
    /// Global debt ceiling (6dp)
    pub global_debt_ceiling: u64,
}

#[event]
pub struct CollateralAdded {
    pub protocol: Pubkey,
    pub collateral_mint: Pubkey,
    pub vault: Pubkey,
    pub initial_ltv_bps: u16,
    pub maintenance_ltv_bps: u16,
    pub liq_bonus_bps: u16,
    pub debt_ceiling: u64,
}

#[event]
pub struct Minted {
    pub owner: Pubkey,
    pub collateral_mint: Pubkey,
    pub minted_pusd_6dp: u64,
    pub new_debt_pusd_6dp: u64,
}

#[event]
pub struct Burned {
    pub owner: Pubkey,
    pub collateral_mint: Pubkey,
    pub burned_pusd_6dp: u64,
    pub new_debt_pusd_6dp: u64,
}

#[event]
pub struct Liquidated {
    pub liquidator: Pubkey,
    pub owner: Pubkey,
    pub collateral_mint: Pubkey,
    pub repaid_pusd_6dp: u64,
    pub seized_collateral_amount: u64,
}

#[event]
pub struct PauseToggled {
    pub protocol: Pubkey,
    pub paused: bool,
}

#[event]
pub struct ParameterUpdated {
    pub protocol: Pubkey,
    pub field: [u8; 16], // e.g., "ltv", "debt_cap" (pack a short label)
    pub old_value: u64,
    pub new_value: u64,
}

