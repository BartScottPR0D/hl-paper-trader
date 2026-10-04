//! Margin account simulation for Hyperliquid-style perpetual futures.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use thiserror::Error;

use crate::trading_position::{PositionId, PositionSide, TradingPosition};

/// Price map: ticker → price.
///
/// Filled from Hyperliquid (`allMids` or `activeAssetCtx` — either works).
pub type Prices = HashMap<String, Decimal>;

// ───────────────────────── Fees ─────────────────────────

/// Order type — determines the Hyperliquid fee rate.
///
/// Actual rates are kept inside [`FeeTier::rate`] and are not exposed
/// as public constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeTier {
    /// Passive limit order — 0.015%.
    Maker,
    /// Aggressive order, filled immediately — 0.045%.
    Taker,
    /// Liquidation — no fee charged.
    Liquidation,
}

impl FeeTier {
    /// Fee rate as a fraction (`0.00045` = 0.045%).
    pub fn rate(self) -> Decimal {
        match self {
            FeeTier::Maker => dec!(0.00015),       // 0.015%
            FeeTier::Taker => dec!(0.00045),       // 0.045%
            FeeTier::Liquidation => Decimal::ZERO, // no fee
        }
    }
}

// ───────────────────────── Margin mode ─────────────────────────

/// Margin mode for the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginMode {
    /// Cross margin — the whole account equity backs every position.
    Cross,
    /// Isolated margin — each position is backed only by its own margin.
    Isolated,
}

// ───────────────────────── Default thresholds ─────────────────────────

/// Default margin call ratio: level ≤ 100%.
pub const DEFAULT_MARGIN_CALL_RATIO: Decimal = Decimal::ONE;

/// Default stop-out ratio: level ≤ 50%.
pub const DEFAULT_STOP_OUT_RATIO: Decimal = dec!(0.50);

// ───────────────────────── Errors ─────────────────────────

/// Errors produced by [`MarginAccount`] operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccountError {
    /// Balance was negative at construction.
    #[error("balance must be non-negative, got {0}")]
    NegativeBalance(Decimal),

    /// A required market price for a symbol was not found in [`Prices`].
    #[error("missing market price for symbol: {0}")]
    MissingPrice(String),

    /// Position id not found in the account.
    #[error("position not found: {0}")]
    PositionNotFound(PositionId),

    /// Another position with the same id already exists.
    #[error("position id {0} already exists")]
    DuplicatePositionId(PositionId),

    /// Not enough balance to open or withdraw.
    #[error("insufficient balance: requested {requested}, available {available}")]
    InsufficientBalance {
        /// Amount requested.
        requested: Decimal,
        /// Amount available.
        available: Decimal,
    },
}

// ───────────────────────── Margin account ─────────────────────────

/// A simulated margin trading account.
///
/// Holds a cash balance, a set of open positions, and the current margin mode.
/// All computations use [`Decimal`] for exact arithmetic.
#[derive(Debug, Clone)]
pub struct MarginAccount {
    balance: Decimal,
    positions: Vec<TradingPosition>,
    margin_mode: MarginMode,
    margin_call_ratio: Decimal,
    stop_out_ratio: Decimal,
}

impl MarginAccount {
    // ────────── Constructors ──────────

    /// Creates a new empty account.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::NegativeBalance`] if `balance < 0`.
    pub fn new(balance: Decimal, margin_mode: MarginMode) -> Result<Self, AccountError> {
        if balance < Decimal::ZERO {
            return Err(AccountError::NegativeBalance(balance));
        }
        Ok(Self {
            balance,
            positions: Vec::new(),
            margin_mode,
            margin_call_ratio: DEFAULT_MARGIN_CALL_RATIO,
            stop_out_ratio: DEFAULT_STOP_OUT_RATIO,
        })
    }

    /// Overrides the default margin-call and stop-out ratios.
    #[must_use]
    pub fn with_ratios(mut self, margin_call: Decimal, stop_out: Decimal) -> Self {
        self.margin_call_ratio = margin_call;
        self.stop_out_ratio = stop_out;
        self
    }

    // ────────── Getters ──────────

    /// Current cash balance (realized only, without unrealized PnL).
    pub fn balance(&self) -> Decimal {
        self.balance
    }

    /// All open positions.
    pub fn positions(&self) -> &[TradingPosition] {
        &self.positions
    }

    /// Current margin mode.
    pub fn margin_mode(&self) -> MarginMode {
        self.margin_mode
    }

    /// Changes the margin mode. Does not affect existing positions.
    pub fn set_margin_mode(&mut self, mode: MarginMode) {
        self.margin_mode = mode;
    }

    /// Margin-call ratio threshold.
    pub fn margin_call_ratio(&self) -> Decimal {
        self.margin_call_ratio
    }

    /// Stop-out ratio threshold.
    pub fn stop_out_ratio(&self) -> Decimal {
        self.stop_out_ratio
    }

    // ────────── Balance ──────────

    /// Adds funds to the balance. Non-positive amounts are ignored.
    pub fn deposit(&mut self, amount: Decimal) {
        if amount > Decimal::ZERO {
            self.balance += amount;
        }
    }

    /// Withdraws funds from the balance.
    ///
    /// Non-positive amounts are a no-op. Returns
    /// [`AccountError::InsufficientBalance`] if `amount > balance`.
    pub fn withdraw(&mut self, amount: Decimal) -> Result<(), AccountError> {
        if amount <= Decimal::ZERO {
            return Ok(());
        }
        if amount > self.balance {
            return Err(AccountError::InsufficientBalance {
                requested: amount,
                available: self.balance,
            });
        }
        self.balance -= amount;
        Ok(())
    }

    // ────────── Open / close ──────────

    /// Opens a new position with an up-front fee.
    ///
    /// The method performs two safety checks:
    ///
    /// 1. The balance covers the entry fee.
    /// 2. After deducting the fee, the balance still covers the total used
    ///    margin (existing positions + the new one).
    ///
    /// On any failure the account is left unchanged — no fee is charged and
    /// no position is added.
    ///
    /// # Errors
    ///
    /// - [`AccountError::DuplicatePositionId`] if the id is already in use.
    /// - [`AccountError::InsufficientBalance`] if the fee or margin is not
    ///   covered by the balance.
    pub fn open_position(
        &mut self,
        mut position: TradingPosition,
        fee: FeeTier,
    ) -> Result<(), AccountError> {
        if self.positions.iter().any(|p| p.id == position.id) {
            return Err(AccountError::DuplicatePositionId(position.id));
        }

        let fee_amount = position.notional() * fee.rate();

        // Check 1: can we cover the entry fee?
        if fee_amount > self.balance {
            return Err(AccountError::InsufficientBalance {
                requested: fee_amount,
                available: self.balance,
            });
        }

        // Check 2: after opening, free margin must stay non-negative.
        // free_margin = (balance - fee) - (already_used + new_used) >= 0
        let new_balance = self.balance - fee_amount;
        let new_used = self.total_used_margin() + position.calculate_used_margin();

        if new_used > new_balance {
            return Err(AccountError::InsufficientBalance {
                requested: new_used,
                available: new_balance,
            });
        }

        self.balance = new_balance;
        position.entry_fee = fee_amount;
        self.positions.push(position);
        Ok(())
    }

    /// Closes a position at `market_price` and settles PnL into the balance.
    ///
    /// Settlement formula:
    ///
    /// ```text
    /// settlement = gross_pnl - entry_fee - exit_fee - accumulated_funding
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::PositionNotFound`] if the id is not found.
    pub fn close_position(
        &mut self,
        id: PositionId,
        market_price: Decimal,
        fee: FeeTier,
    ) -> Result<TradingPosition, AccountError> {
        let idx = self
            .positions
            .iter()
            .position(|p| p.id == id)
            .ok_or(AccountError::PositionNotFound(id))?;
        let pos = self.positions.remove(idx);

        let gross = pos.gross_pnl(market_price);
        let exit_fee = pos.size * market_price * fee.rate();
        let funding = pos.accumulated_funding;

        let settlement = gross - pos.entry_fee - exit_fee - funding;
        self.balance += settlement;

        let mut closed = pos;
        closed.accumulated_funding += exit_fee; // keep for reporting
        Ok(closed)
    }

    /// Partially closes a position by `close_size` base units.
    ///
    /// If the remaining size becomes zero, the position is removed.
    /// Returns the closed part as a standalone [`TradingPosition`].
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::PositionNotFound`] if the id is not found.
    pub fn close_partial(
        &mut self,
        id: PositionId,
        close_size: Decimal,
        market_price: Decimal,
        fee: FeeTier,
    ) -> Result<TradingPosition, AccountError> {
        let idx = self
            .positions
            .iter()
            .position(|p| p.id == id)
            .ok_or(AccountError::PositionNotFound(id))?;

        let pos = self.positions[idx].clone();
        let (closed, remaining) = pos
            .split_by_size(close_size)
            .map_err(|_| AccountError::PositionNotFound(id))?;

        let gross = closed.gross_pnl(market_price);
        let exit_fee = closed.size * market_price * fee.rate();
        let settlement = gross - closed.entry_fee - exit_fee - closed.accumulated_funding;
        self.balance += settlement;

        if remaining.size == Decimal::ZERO {
            self.positions.remove(idx);
        } else {
            self.positions[idx] = remaining;
        }

        Ok(closed)
    }

    /// Returns a reference to a position by id.
    pub fn get_position(&self, id: PositionId) -> Option<&TradingPosition> {
        self.positions.iter().find(|p| p.id == id)
    }

    /// Opens a position with one-way netting, as on Hyperliquid.
    ///
    /// If an opposite-side position on the same symbol exists, it is reduced
    /// first (or fully closed) and the realized PnL is settled into the balance.
    /// The remainder of the incoming position, if any, is opened as a new one.
    ///
    /// Returns `Ok(None)` if the incoming position was fully absorbed by the
    /// opposite-side position; otherwise returns the newly opened position.
    ///
    /// # Errors
    ///
    /// Same as [`MarginAccount::open_position`] for the residual part.
    pub fn open_netting(
        &mut self,
        mut incoming: TradingPosition,
        market_price: Decimal,
        fee: FeeTier,
    ) -> Result<Option<TradingPosition>, AccountError> {
        if let Some(idx) = self
            .positions
            .iter()
            .position(|p| p.symbol == incoming.symbol && p.side != incoming.side)
        {
            let existing = self.positions[idx].clone();
            let reduce = incoming.size.min(existing.size);

            // Settle PnL for the closed part of the opposite-side position.
            let gross = match existing.side {
                PositionSide::Long => (market_price - existing.entry_price) * reduce,
                PositionSide::Short => (existing.entry_price - market_price) * reduce,
            };
            let ratio_closed = reduce / existing.size;
            let entry_fee_part = existing.entry_fee * ratio_closed;
            let funding_part = existing.accumulated_funding * ratio_closed;

            self.balance += gross - entry_fee_part - funding_part;

            let remaining_existing = existing.size - reduce;
            if remaining_existing == Decimal::ZERO {
                self.positions.remove(idx);
            } else {
                let ratio = remaining_existing / existing.size;
                let mut updated = existing.clone();
                updated.size = remaining_existing;
                updated.entry_fee = existing.entry_fee * ratio;
                updated.accumulated_funding = existing.accumulated_funding * ratio;
                self.positions[idx] = updated;
            }

            incoming.size -= reduce;
            if incoming.size == Decimal::ZERO {
                return Ok(None);
            }
        }

        self.open_position(incoming.clone(), fee)?;
        Ok(Some(incoming))
    }

    // ────────── Aggregates ──────────

    /// Total initial margin across all open positions.
    pub fn total_used_margin(&self) -> Decimal {
        self.positions
            .iter()
            .map(TradingPosition::calculate_used_margin)
            .sum()
    }

    /// Total unrealized PnL across all open positions.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingPrice`] if any position's symbol is
    /// absent from `prices`.
    pub fn total_unrealized_pnl(&self, prices: &Prices) -> Result<Decimal, AccountError> {
        let mut total = Decimal::ZERO;
        for pos in &self.positions {
            let price = Self::price_for(prices, &pos.symbol)?;
            total += pos.gross_pnl(price);
        }
        Ok(total)
    }

    /// Account equity: `balance + total_unrealized_pnl`.
    pub fn equity(&self, prices: &Prices) -> Result<Decimal, AccountError> {
        Ok(self.balance + self.total_unrealized_pnl(prices)?)
    }

    /// Free margin: `equity - total_used_margin`.
    pub fn free_margin(&self, prices: &Prices) -> Result<Decimal, AccountError> {
        Ok(self.equity(prices)? - self.total_used_margin())
    }

    /// Margin level: `equity / total_used_margin`.
    ///
    /// Returns `Ok(None)` if there are no open positions.
    pub fn margin_level(&self, prices: &Prices) -> Result<Option<Decimal>, AccountError> {
        let used = self.total_used_margin();
        if used == Decimal::ZERO {
            return Ok(None);
        }
        Ok(Some(self.equity(prices)? / used))
    }

    /// Margin level as a percentage (`100.0` = 100%).
    ///
    /// Returns `Ok(None)` if there are no open positions.
    pub fn margin_level_percent(
        &self,
        prices: &Prices,
    ) -> Result<Option<Decimal>, AccountError> {
        Ok(self.margin_level(prices)?.map(|l| l * dec!(100)))
    }

    // ────────── Funding ──────────

    /// Applies hourly funding to every open position.
    ///
    /// `funding_rate` is the hourly rate. The current price is used as a
    /// proxy for the oracle price.
    ///
    /// Returns the total payment made by the account:
    /// positive means the account paid, negative means it received.
    pub fn apply_funding(
        &mut self,
        prices: &Prices,
        funding_rate: Decimal,
    ) -> Result<Decimal, AccountError> {
        let mut total_paid = Decimal::ZERO;
        for pos in self.positions.iter_mut() {
            let price = Self::price_for(prices, &pos.symbol)?;
            pos.apply_funding(price, funding_rate);

            let payment = pos.size * price * funding_rate;
            let signed = match pos.side {
                PositionSide::Long => payment,
                PositionSide::Short => -payment,
            };
            total_paid += signed;
        }
        self.balance -= total_paid;
        Ok(total_paid)
    }

    // ────────── Margin call / stop-out ──────────

    /// Returns `true` if the account is in a margin-call state.
    ///
    /// - **Cross**: overall level ≤ `margin_call_ratio`.
    /// - **Isolated**: any position's level ≤ `margin_call_ratio`.
    pub fn is_margin_call(&self, prices: &Prices) -> Result<bool, AccountError> {
        match self.margin_mode {
            MarginMode::Cross => Ok(self
                .margin_level(prices)?
                .is_some_and(|l| l <= self.margin_call_ratio)),
            MarginMode::Isolated => Ok(!self
                .positions_below_ratio(prices, self.margin_call_ratio)?
                .is_empty()),
        }
    }

    /// Returns `true` if the account should be stopped out.
    ///
    /// - **Cross**: overall level ≤ `stop_out_ratio`.
    /// - **Isolated**: any position's level ≤ `stop_out_ratio`.
    pub fn is_stop_out(&self, prices: &Prices) -> Result<bool, AccountError> {
        match self.margin_mode {
            MarginMode::Cross => Ok(self
                .margin_level(prices)?
                .is_some_and(|l| l <= self.stop_out_ratio)),
            MarginMode::Isolated => Ok(!self
                .positions_below_ratio(prices, self.stop_out_ratio)?
                .is_empty()),
        }
    }

    /// Returns the ids of positions that should be stopped out.
    ///
    /// - **Cross**: all positions, if the account as a whole is stopped out.
    /// - **Isolated**: only the positions whose level is at or below the
    ///   stop-out ratio.
    pub fn stop_out_positions(
        &self,
        prices: &Prices,
    ) -> Result<Vec<PositionId>, AccountError> {
        match self.margin_mode {
            MarginMode::Cross => {
                if self.is_stop_out(prices)? {
                    Ok(self.positions.iter().map(|p| p.id).collect())
                } else {
                    Ok(Vec::new())
                }
            }
            MarginMode::Isolated => self.positions_below_ratio(prices, self.stop_out_ratio),
        }
    }

    /// Liquidates all positions flagged by [`MarginAccount::stop_out_positions`].
    ///
    /// Positions are closed at the current market price with
    /// [`FeeTier::Liquidation`] (no fee).
    ///
    /// Returns the list of liquidated position ids.
    pub fn liquidate_stop_out(
        &mut self,
        prices: &Prices,
    ) -> Result<Vec<PositionId>, AccountError> {
        let ids = self.stop_out_positions(prices)?;
        for id in &ids {
            let symbol = self
                .positions
                .iter()
                .find(|p| p.id == *id)
                .map(|p| p.symbol.clone());
            let Some(symbol) = symbol else { continue };
            let price = Self::price_for(prices, &symbol)?;
            let _ = self.close_position(*id, price, FeeTier::Liquidation);
        }
        Ok(ids)
    }

    // ────────── Liquidation price (cross) ──────────

    /// Estimates the liquidation price of the given symbol in Cross mode.
    ///
    /// Solves for the price `p` at which
    /// `equity(p) == maintenance_margin(p)`, keeping all other positions
    /// fixed at their current mark prices.
    ///
    /// Returns `Ok(None)` if the symbol is not found or the formula degenerates.
    pub fn liquidation_price_cross(
        &self,
        symbol: &str,
        prices: &Prices,
    ) -> Result<Option<Decimal>, AccountError> {
        let pos = match self.positions.iter().find(|p| p.symbol == symbol) {
            Some(p) => p,
            None => return Ok(None),
        };

        let side_sign = match pos.side {
            PositionSide::Long => Decimal::ONE,
            PositionSide::Short => -Decimal::ONE,
        };
        let l = pos.maintenance_margin_rate;
        let denom = Decimal::ONE - l * side_sign;
        if denom == Decimal::ZERO {
            return Ok(None);
        }

        let mut other_pnl = Decimal::ZERO;
        for p in self.positions.iter().filter(|p| p.id != pos.id) {
            let price = Self::price_for(prices, &p.symbol)?;
            other_pnl += p.gross_pnl(price);
        }

        let a = pos.size * side_sign - pos.size * l;
        let b = self.balance + other_pnl - pos.entry_price * pos.size * side_sign;
        if a == Decimal::ZERO {
            return Ok(None);
        }
        Ok(Some(-b / a))
    }

    // ────────── Internal helpers ──────────

    fn price_for(prices: &Prices, symbol: &str) -> Result<Decimal, AccountError> {
        prices
            .get(symbol)
            .copied()
            .ok_or_else(|| AccountError::MissingPrice(symbol.to_string()))
    }

    fn positions_below_ratio(
        &self,
        prices: &Prices,
        ratio: Decimal,
    ) -> Result<Vec<PositionId>, AccountError> {
        let mut ids = Vec::new();
        for pos in &self.positions {
            let price = Self::price_for(prices, &pos.symbol)?;
            let used = pos.calculate_used_margin();
            if used == Decimal::ZERO {
                continue;
            }
            let level = pos.isolated_equity(price) / used;
            if level <= ratio {
                ids.push(pos.id);
            }
        }
        Ok(ids)
    }
}


// ───────────────────────── Position views ─────────────────────────

/// A read-only projection of a single position with current market data.
///
/// Created on the fly by [`MarginAccount::position_view`]. Not stored —
/// so it can never become stale or out of sync with the account.
#[derive(Debug, Clone)]
pub struct PositionView<'a> {
    /// The underlying position.
    pub position: &'a TradingPosition,
    /// Current market price for the position's symbol.
    pub current_price: Decimal,
    /// Gross unrealized PnL (before fees and funding).
    pub gross_pnl: Decimal,
    /// Net unrealized PnL (after entry fee and accumulated funding).
    pub net_pnl: Decimal,
    /// Maintenance margin at the current price.
    pub maintenance_margin: Decimal,
    /// Estimated liquidation price, if computable.
    ///
    /// - **Isolated**: from [`TradingPosition::liquidation_price_isolated`].
    /// - **Cross**: from [`MarginAccount::liquidation_price_cross`].
    pub liquidation_price: Option<Decimal>,
}

/// Convenience methods for inspecting individual positions.
impl MarginAccount {
    /// Gross unrealized PnL of a single position.
    ///
    /// Returns `Ok(None)` if the position id is not found.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingPrice`] if the position's symbol is not
    /// in `prices`.
    pub fn position_pnl(
        &self,
        id: PositionId,
        prices: &Prices,
    ) -> Result<Option<Decimal>, AccountError> {
        match self.get_position(id) {
            Some(pos) => {
                let price = Self::price_for(prices, &pos.symbol)?;
                Ok(Some(pos.gross_pnl(price)))
            }
            None => Ok(None),
        }
    }

    /// Estimated liquidation price for a single position, aware of the
    /// current [`MarginMode`].
    ///
    /// - **Isolated**: uses [`TradingPosition::liquidation_price_isolated`].
    /// - **Cross**: uses [`MarginAccount::liquidation_price_cross`], which
    ///   takes other positions into account.
    ///
    /// Returns `Ok(None)` if the position id is not found or the price
    /// cannot be computed (e.g. degenerate formula).
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingPrice`] if any required symbol is not
    /// in `prices`.
    pub fn position_liquidation_price(
        &self,
        id: PositionId,
        prices: &Prices,
    ) -> Result<Option<Decimal>, AccountError> {
        let pos = match self.get_position(id) {
            Some(p) => p,
            None => return Ok(None),
        };
        match self.margin_mode {
            MarginMode::Isolated => {
                let price = Self::price_for(prices, &pos.symbol)?;
                Ok(pos.liquidation_price_isolated(price))
            }
            MarginMode::Cross => self.liquidation_price_cross(&pos.symbol, prices),
        }
    }

    /// Builds a full [`PositionView`] for a single position: current price,
    /// PnL (gross and net), maintenance margin, and liquidation price — all
    /// computed against the latest `prices`.
    ///
    /// Returns `Ok(None)` if the position id is not found.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingPrice`] if any required symbol is not
    /// in `prices`.
    pub fn position_view(
        &self,
        id: PositionId,
        prices: &Prices,
    ) -> Result<Option<PositionView<'_>>, AccountError> {
        let pos = match self.get_position(id) {
            Some(p) => p,
            None => return Ok(None),
        };
        let price = Self::price_for(prices, &pos.symbol)?;

        let liquidation_price = match self.margin_mode {
            MarginMode::Isolated => pos.liquidation_price_isolated(price),
            MarginMode::Cross => self.liquidation_price_cross(&pos.symbol, prices)?,
        };

        Ok(Some(PositionView {
            position: pos,
            current_price: price,
            gross_pnl: pos.gross_pnl(price),
            net_pnl: pos.net_pnl(price),
            maintenance_margin: pos.maintenance_margin(price),
            liquidation_price,
        }))
    }

    /// Builds a [`PositionView`] for every open position.
    ///
    /// Positions whose symbols are missing from `prices` are skipped.
    /// If you need strict error reporting, use [`MarginAccount::position_view`]
    /// per position.
    pub fn all_position_views(&self, prices: &Prices) -> Vec<PositionView<'_>> {
        self.positions
            .iter()
            .filter_map(|pos| self.position_view(pos.id, prices).ok().flatten())
            .collect()
    }
}

// ───────────────────────── Portfolio summary ─────────────────────────

/// Compact aggregation across all open positions.
///
/// Useful for dashboards, logging, and risk checks where you want a
/// single object with everything important instead of iterating over
/// positions manually.
#[derive(Debug, Clone)]
pub struct PortfolioSummary {
    /// Number of open positions.
    pub position_count: usize,

    /// Sum of gross unrealized PnL across all positions.
    pub total_gross_pnl: Decimal,
    /// Sum of net unrealized PnL (after entry fees and funding).
    pub total_net_pnl: Decimal,

    /// Sum of maintenance margin across all positions at current prices.
    pub total_maintenance_margin: Decimal,
    /// Sum of initial (used) margin across all positions.
    pub total_used_margin: Decimal,

    /// Id of the position closest to its liquidation price.
    /// `None` if no positions, or if no liquidation price is computable.
    pub nearest_liquidation_id: Option<PositionId>,
    /// Absolute distance (in quote currency) from the current price to the
    /// nearest liquidation price.
    pub nearest_liquidation_distance: Option<Decimal>,
    /// Same distance expressed as a fraction of the current price
    /// (`0.05` = 5%). Useful for threshold checks.
    pub nearest_liquidation_distance_frac: Option<Decimal>,
}

impl PortfolioSummary {
    /// Returns `true` if any position is closer to liquidation than `threshold`
    /// (fraction, e.g. `dec!(0.05)` for 5%).
    pub fn is_close_to_liquidation(&self, threshold: Decimal) -> bool {
        self.nearest_liquidation_distance_frac
            .is_some_and(|d| d <= threshold)
    }
}

/// Aggregated inspection methods.
impl MarginAccount {
    /// Builds a [`PortfolioSummary`] from the current positions and prices.
    ///
    /// For every position, computes gross and net PnL, maintenance margin,
    /// used margin, and (depending on [`MarginMode`]) a liquidation price.
    /// The "nearest to liquidation" position is chosen by the smallest
    /// relative distance from the current price.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::MissingPrice`] if any position's symbol is
    /// not in `prices`. Unlike [`MarginAccount::all_position_views`], this
    /// method is **strict**: silent skips would produce misleading totals.
    pub fn positions_summary(
        &self,
        prices: &Prices,
    ) -> Result<PortfolioSummary, AccountError> {
        let mut total_gross = Decimal::ZERO;
        let mut total_net = Decimal::ZERO;
        let mut total_mm = Decimal::ZERO;
        let mut total_used = Decimal::ZERO;

        // (id, absolute distance, distance as fraction of current price)
        let mut nearest: Option<(PositionId, Decimal, Decimal)> = None;

        for pos in &self.positions {
            let price = Self::price_for(prices, &pos.symbol)?;

            total_gross += pos.gross_pnl(price);
            total_net += pos.net_pnl(price);
            total_mm += pos.maintenance_margin(price);
            total_used += pos.calculate_used_margin();

            let liq = match self.margin_mode {
                MarginMode::Isolated => pos.liquidation_price_isolated(price),
                MarginMode::Cross => self.liquidation_price_cross(&pos.symbol, prices)?,
            };

            if let Some(liq_price) = liq {
                let distance = (price - liq_price).abs();
                let distance_frac = if price == Decimal::ZERO {
                    Decimal::ZERO
                } else {
                    distance / price
                };

                let is_closer = match nearest {
                    Some((_, _, best_frac)) => distance_frac < best_frac,
                    None => true,
                };
                if is_closer {
                    nearest = Some((pos.id, distance, distance_frac));
                }
            }
        }

        let (nearest_id, nearest_distance, nearest_distance_frac) = match nearest {
            Some((id, d, df)) => (Some(id), Some(d), Some(df)),
            None => (None, None, None),
        };

        Ok(PortfolioSummary {
            position_count: self.positions.len(),
            total_gross_pnl: total_gross,
            total_net_pnl: total_net,
            total_maintenance_margin: total_mm,
            total_used_margin: total_used,
            nearest_liquidation_id: nearest_id,
            nearest_liquidation_distance: nearest_distance,
            nearest_liquidation_distance_frac: nearest_distance_frac,
        })
    }
}