//! Liquidation: drive a position into stop-out and liquidate.

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    // Small balance to trigger liquidation quickly
    let mut account = MarginAccount::new(dec!(1000), MarginMode::Cross)?;

    // 0.1 BTC long with 10x → used margin = 500, notional = 5000
    let position = TradingPosition::new(
        1, "BTC", PositionSide::Long,
        dec!(0.1), dec!(50000),
        NonZeroU32::new(10).unwrap(), dec!(0.0125),
    )?;
    account.open_position(position, FeeTier::Taker)?;

    println!("Balance:     {}", account.balance());
    println!("Used margin: {}", account.total_used_margin());
    println!("Margin level: {:?}%", account.margin_level_percent(&prices)?);

    // Price drops
    prices.insert("BTC".into(), dec!(49000));

    println!("\n=== After price drop to 49,000 ===");
    println!("Unrealized PnL: {}", account.total_unrealized_pnl(&prices)?);
    println!("Equity:         {}", account.equity(&prices)?);
    println!("Margin level:   {:?}%", account.margin_level_percent(&prices)?);
    println!("Is stop out:    {}", account.is_stop_out(&prices)?);

    // Liquidate if needed
    if account.is_stop_out(&prices)? {
        let liquidated = account.liquidate_stop_out(&prices)?;
        println!("\nLiquidated: {:?}", liquidated);
        println!("Balance after liquidation: {}", account.balance());
        println!("Positions: {}", account.positions().len());
    }

    Ok(())
}