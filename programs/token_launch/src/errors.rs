use anchor_lang::prelude::*;

/// Single error enum for the whole program (the original had five `ErrorCode`
/// enums glob-re-exported into one namespace, which does not compile).
#[error_code]
pub enum LaunchError {
    #[msg("Invalid total supply provided.")]
    InvalidSupply,
    #[msg("Invalid amount provided.")]
    InvalidAmount,
    #[msg("Presale has ended / sold out.")]
    PresaleEnded,
    #[msg("Purchase exceeds remaining presale allocation.")]
    ExceedsPresaleAllocation,
    #[msg("Cost exceeds the buyer's maximum (slippage).")]
    SlippageExceeded,
    #[msg("Insufficient liquidity.")]
    InsufficientLiquidity,
    #[msg("Invalid price.")]
    InvalidPrice,
    #[msg("No tokens available for release.")]
    NoTokensAvailable,
    #[msg("Invalid airdrop time window.")]
    InvalidTimeWindow,
    #[msg("Airdrop has not started yet.")]
    AirdropNotStarted,
    #[msg("Airdrop has already ended.")]
    AirdropAlreadyEnded,
    #[msg("Airdrop already claimed.")]
    AlreadyClaimed,
    #[msg("Whitelist would exceed total airdrop allocation.")]
    ExceedsTotalTokens,
    #[msg("Arithmetic overflow.")]
    MathOverflow,
    #[msg("Unauthorized.")]
    Unauthorized,
}
