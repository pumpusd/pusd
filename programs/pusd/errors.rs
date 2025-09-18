use anchor_lang::prelude::*;

/// PumpUSD program errors
#[error_code]
pub enum ErrorCode {
    // -------- Access / Auth --------
    #[msg("Unauthorized: caller is not the required authority.")]
    Unauthorized,

    #[msg("Invalid PDA seeds or bump.")]
    InvalidPda,

    #[msg("Governance timelock has not elapsed.")]
    TimelockNotElapsed,

    // -------- Params / Config --------
    #[msg("Protocol minting is currently paused.")]
    MintPaused,

    #[msg("Collateral type is not active.")]
    CollateralInactive,

    #[msg("Unsupported collateral mint.")]
    UnsupportedCollateral,

    #[msg("Global debt ceiling reached.")]
    GlobalDebtCeilingReached,

    #[msg("Per-collateral debt ceiling reached.")]
    CollateralDebtCeilingReached,

    #[msg("Parameter out of allowed bounds.")]
    InvalidParameter,

    // -------- Amounts / Math --------
    #[msg("Amount must be greater than zero.")]
    ZeroAmount,

    #[msg("Invalid amount.")]
    InvalidAmount,

    #[msg("Overflow during arithmetic operation.")]
    MathOverflow,

    #[msg("Underflow during arithmetic operation.")]
    MathUnderflow,

    #[msg("Division by zero.")]
    DivisionByZero,

    // -------- Position / Vault --------
    #[msg("Position already exists.")]
    PositionAlreadyExists,

    #[msg("Position not found.")]
    PositionNotFound,

    #[msg("Insufficient collateral for this operation.")]
    InsufficientCollateral,

    #[msg("LTV would exceed initial limit.")]
    LtvExceeded,

    #[msg("Health below maintenance threshold; action not allowed.")]
    HealthBelowMaintenance,

    #[msg("Provided vault account does not match config.")]
    VaultMismatch,

    // -------- Liquidation --------
    #[msg("Position is not eligible for liquidation.")]
    NotLiquidatable,

    #[msg("Requested liquidation amount is too large.")]
    LiquidationTooLarge,

    #[msg("Slippage or price impact exceeded limits.")]
    SlippageExceeded,

    // -------- Oracles / Pricing --------
    #[msg("Oracle account(s) invalid or missing.")]
    InvalidOracle,

    #[msg("Oracle price is stale.")]
    OracleStale,

    #[msg("Oracle price out of acceptable bounds.")]
    PriceOutOfBounds,

    // -------- SPL / Token --------
    #[msg("Token mint does not match expected mint.")]
    MintMismatch,

    #[msg("Token account owner mismatch.")]
    TokenOwnerMismatch,

    // -------- Misc --------
    #[msg("Feature not implemented.")]
    NotImplemented,
}

