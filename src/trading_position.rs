//! Trading position model.
//!
//! A [`TradingPosition`] represents a single open position on a [`MarginAccount`](crate::MarginAccount).
//! It carries all static data (symbol, side, size, entry price, leverage,
//! maintenance margin rate) and accumulates dynamic data over its lifetime
//! (entry fee, funding payments).

use rust_decimal::Decimal;
use std::num::NonZeroU32;
use thiserror::Error;

/// Unique position identifier.
pub type PositionId = u64;

/// Direction of a trading position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSide {
    /// Long position — profits when price goes up.
    Long,
    /// Short position — profits when price goes down.
    Short,
}

/// Errors produced when constructing or splitting a [`TradingPosition`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PositionError {
    /// Position size must be strictly positive.
    #[error("size must be positive, got {0}")]
    NonPositiveSize(Decimal),

    /// Entry price must be strictly positive.
    #[error("entry price must be positive, got {0}")]
    NonPositiveEntryPrice(Decimal),

    /// Margin amount must be strictly positive.
    #[error("margin must be positive, got {0}")]
    NonPositiveMargin(Decimal),

    /// Maintenance margin rate must be in `[0, 1)`.
    #[error("maintenance margin rate must be in [0, 1), got {0}")]
    InvalidMaintenanceMarginRate(Decimal),

    /// Attempted to close more than the current position size.
    #[error("close size {requested} exceeds position size {available}")]
    CloseSizeExceedsPosition {
        /// Size requested to close.
        requested: Decimal,
        /// Current position size.
        available: Decimal,
    },
}

/// An open trading position inside a [`crate::MarginAccount`].
///
/// All monetary values use [`Decimal`] to avoid floating-point drift.
///
/// `entry_fee` and `accumulated_funding` accumulate over the position's
/// lifetime and are subtracted from the gross PnL to obtain the net result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradingPosition {
    /// Unique identifier within the account.
    pub id: PositionId,
    /// Trading pair symbol (e.g. `"BTC"`, `"ETH"`).
    pub symbol: String,
    /// Direction — Long or Short.
    pub side: PositionSide,
    /// Size in the base asset (e.g. `0.5` BTC).
    pub size: Decimal,
    /// Price at which the position was opened.
    pub entry_price: Decimal,
    /// Leverage. Guaranteed non-zero by the type system.
    pub leverage: NonZeroU32,
    /// Maintenance margin rate (e.g. `0.0125` for 40x max leverage).
    pub maintenance_margin_rate: Decimal,

    /// Fee paid on entry (in quote currency).
    pub entry_fee: Decimal,
    /// Accumulated funding paid over the position's lifetime.
    /// Positive value means the position has paid funding; negative means received.
    pub accumulated_funding: Decimal,
}

impl TradingPosition {
    /// Creates a new position from an explicit size.
    ///
    /// # Errors
    ///
    /// Returns an error if `size <= 0`, `entry_price <= 0`, or
    /// `maintenance_margin_rate` is outside `[0, 1)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PositionId,
        symbol: impl Into<String>,
        side: PositionSide,
        size: Decimal,
        entry_price: Decimal,
        leverage: NonZeroU32,
        maintenance_margin_rate: Decimal,
    ) -> Result<Self, PositionError> {
        if size <= Decimal::ZERO {
            return Err(PositionError::NonPositiveSize(size));
        }
        if entry_price <= Decimal::ZERO {
            return Err(PositionError::NonPositiveEntryPrice(entry_price));
        }
        if maintenance_margin_rate < Decimal::ZERO || maintenance_margin_rate >= Decimal::ONE {
            return Err(PositionError::InvalidMaintenanceMarginRate(
                maintenance_margin_rate,
            ));
        }
        Ok(Self {
            id,
            symbol: symbol.into(),
            side,
            size,
            entry_price,
            leverage,
            maintenance_margin_rate,
            entry_fee: Decimal::ZERO,
            accumulated_funding: Decimal::ZERO,
        })
    }

    /// Creates a new position from a margin amount.
    ///
    /// Computes size as:
    ///
    /// ```text
    /// size = margin * leverage / entry_price
    /// ```
    ///
    /// This is the inverse of [`TradingPosition::calculate_used_margin`].
    ///
    /// # Errors
    ///
    /// Returns an error if `margin <= 0`, `entry_price <= 0`, or
    /// `maintenance_margin_rate` is outside `[0, 1)`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_margin(
        id: PositionId,
        symbol: impl Into<String>,
        side: PositionSide,
        margin: Decimal,
        entry_price: Decimal,
        leverage: NonZeroU32,
        maintenance_margin_rate: Decimal,
    ) -> Result<Self, PositionError> {
        if margin <= Decimal::ZERO {
            return Err(PositionError::NonPositiveMargin(margin));
        }
        if entry_price <= Decimal::ZERO {
            return Err(PositionError::NonPositiveEntryPrice(entry_price));
        }
        if maintenance_margin_rate < Decimal::ZERO || maintenance_margin_rate >= Decimal::ONE {
            return Err(PositionError::InvalidMaintenanceMarginRate(
                maintenance_margin_rate,
            ));
        }

        let size = margin * Decimal::from(leverage.get()) / entry_price;
        if size <= Decimal::ZERO {
            return Err(PositionError::NonPositiveSize(size));
        }

        Ok(Self {
            id,
            symbol: symbol.into(),
            side,
            size,
            entry_price,
            leverage,
            maintenance_margin_rate,
            entry_fee: Decimal::ZERO,
            accumulated_funding: Decimal::ZERO,
        })
    }

    // ────────── Notional / margin ──────────

    /// Notional value at entry: `size * entry_price`.
    pub fn notional(&self) -> Decimal {
        self.size * self.entry_price
    }

    /// Initial margin: `notional / leverage`.
    pub fn calculate_used_margin(&self) -> Decimal {
        self.notional() / Decimal::from(self.leverage.get())
    }

    /// Maintenance margin at the given market price:
    /// `size * market_price * maintenance_margin_rate`.
    pub fn maintenance_margin(&self, market_price: Decimal) -> Decimal {
        self.size * market_price * self.maintenance_margin_rate
    }

    // ────────── PnL ──────────

    /// Gross unrealized PnL at `market_price` (excludes fees and funding).
    pub fn gross_pnl(&self, market_price: Decimal) -> Decimal {
        match self.side {
            PositionSide::Long => (market_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - market_price) * self.size,
        }
    }

    /// Net PnL: `gross_pnl - entry_fee - accumulated_funding`.
    pub fn net_pnl(&self, market_price: Decimal) -> Decimal {
        self.gross_pnl(market_price) - self.entry_fee - self.accumulated_funding
    }

    // ────────── Isolated equity ──────────

    /// Isolated equity: `initial_margin + gross_pnl`.
    pub fn isolated_equity(&self, market_price: Decimal) -> Decimal {
        self.calculate_used_margin() + self.gross_pnl(market_price)
    }

    /// Isolated margin level: `isolated_equity / initial_margin`.
    ///
    /// Returns `None` if initial margin is zero (cannot happen in practice,
    /// because leverage is non-zero).
    pub fn margin_level(&self, market_price: Decimal) -> Option<Decimal> {
        let used = self.calculate_used_margin();
        if used == Decimal::ZERO {
            None
        } else {
            Some(self.isolated_equity(market_price) / used)
        }
    }

    // ────────── Funding ──────────

    /// Applies a funding payment to this position.
    ///
    /// `funding_rate` is the hourly rate (positive means longs pay shorts).
    /// Formula used by Hyperliquid:
    ///
    /// ```text
    /// payment = size * oracle_price * funding_rate
    /// ```
    ///
    /// Longs pay when the rate is positive; shorts receive.
    pub fn apply_funding(&mut self, oracle_price: Decimal, funding_rate: Decimal) {
        let payment = self.size * oracle_price * funding_rate;
        let signed = match self.side {
            PositionSide::Long => payment,
            PositionSide::Short => -payment,
        };
        self.accumulated_funding += signed;
    }

    // ────────── Liquidation price (isolated) ──────────

    /// Estimates the liquidation price for an isolated position.
    ///
    /// Uses the Hyperliquid approximation:
    ///
    /// ```text
    /// p_liq = p - side_sign * margin_available / size / (1 - l * side_sign)
    /// ```
    ///
    /// where `side_sign = +1` for Long and `-1` for Short,
    /// `l = maintenance_margin_rate`.
    pub fn liquidation_price_isolated(&self, market_price: Decimal) -> Option<Decimal> {
        let maintenance = self.maintenance_margin(market_price);
        let margin_available = self.isolated_equity(market_price) - maintenance;
        if margin_available <= Decimal::ZERO {
            return Some(market_price);
        }
        let side_sign = match self.side {
            PositionSide::Long => Decimal::ONE,
            PositionSide::Short => -Decimal::ONE,
        };
        let l = self.maintenance_margin_rate;
        let denom = Decimal::ONE - l * side_sign;
        if denom == Decimal::ZERO {
            return None;
        }
        Some(market_price - side_sign * margin_available / self.size / denom)
    }

    // ────────── Partial close ──────────

    /// Splits the position by size into `(closed_part, remaining_part)`.
    ///
    /// The entry fee and accumulated funding are distributed proportionally
    /// between the two parts.
    ///
    /// # Errors
    ///
    /// Returns [`PositionError::CloseSizeExceedsPosition`] if `close_size`
    /// is not in `(0, self.size]`.
    pub fn split_by_size(mut self, close_size: Decimal) -> Result<(Self, Self), PositionError> {
        if close_size <= Decimal::ZERO || close_size > self.size {
            return Err(PositionError::CloseSizeExceedsPosition {
                requested: close_size,
                available: self.size,
            });
        }
        let ratio = close_size / self.size;

        let mut closed = self.clone();
        closed.size = close_size;
        closed.entry_fee = self.entry_fee * ratio;
        closed.accumulated_funding = self.accumulated_funding * ratio;

        self.size -= close_size;
        self.entry_fee -= closed.entry_fee;
        self.accumulated_funding -= closed.accumulated_funding;

        Ok((closed, self))
    }
}