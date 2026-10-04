//! # hl-paper-trader
//!
//! Paper-trading engine for [Hyperliquid](https://hyperliquid.xyz) perpetual futures.
//!
//! Simulates unrealized PnL, fees, funding, margin requirements, and liquidation
//! for a margin account with one or more open positions.
//!
//! ## Features
//!
//! - **Cross and Isolated margin modes** with realistic stop-out logic.
//! - **One-way netting**: only one net position per symbol, as on Hyperliquid.
//! - **Fee tiers**: [`FeeTier::Maker`], [`FeeTier::Taker`], [`FeeTier::Liquidation`].
//! - **Hourly funding** applied via [`MarginAccount::apply_funding`].
//! - **Liquidation price** estimation for both Cross and Isolated modes.
//! - All monetary values use [`rust_decimal::Decimal`] for exact arithmetic.
//!
//! ## Quick start
//!
//! ```no_run
//! use std::num::NonZeroU32;
//! use rust_decimal_macros::dec;
//! use hl_paper_trader::{
//!     FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut prices: Prices = Prices::new();
//! prices.insert("BTC".into(), dec!(50000));
//!
//! let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;
//!
//! let position = TradingPosition::from_margin(
//!     1,
//!     "BTC",
//!     PositionSide::Long,
//!     dec!(1000),              // margin
//!     dec!(50000),             // entry price
//!     NonZeroU32::new(10).unwrap(),
//!     dec!(0.0125),            // maintenance margin rate
//! )?;
//!
//! account.open_position(position, FeeTier::Taker)?;
//!
//! let equity = account.equity(&prices)?;
//! let level = account.margin_level_percent(&prices)?;
//! println!("Equity: {equity}, Margin level: {level:?}");
//! # Ok(())
//! # }
//! ```

pub mod margin_account;
pub mod trading_position;

pub use margin_account::{
    AccountError, FeeTier, MarginAccount, MarginMode, Prices,
    DEFAULT_MARGIN_CALL_RATIO, DEFAULT_STOP_OUT_RATIO,
};
pub use trading_position::{
    PositionError, PositionId, PositionSide, TradingPosition,
};