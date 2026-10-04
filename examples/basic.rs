//! Basic usage: open a position, print account stats.

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Market prices
    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    // Account with $10,000
    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    // Open 0.1 BTC long with 10x leverage
    let position = TradingPosition::from_margin(
        1,
        "BTC",
        PositionSide::Long,
        dec!(1000),       // margin
        dec!(50000),      // entry price
        NonZeroU32::new(10).unwrap(),
        dec!(0.0125),     // maintenance margin rate (40x max → 1.25%)
    )?;

    account.open_position(position, FeeTier::Taker)?;

    println!("=== After opening ===");
    println!("Balance:      {}", account.balance());
    println!("Used margin:  {}", account.total_used_margin());
    println!("Equity:       {}", account.equity(&prices)?);
    println!("Free margin:  {}", account.free_margin(&prices)?);
    println!("Margin level: {:?}%", account.margin_level_percent(&prices)?);

    // Price goes up 5%
    prices.insert("BTC".into(), dec!(52500));

    println!("\n=== After price moved to 52,500 ===");
    println!("Unrealized PnL: {}", account.total_unrealized_pnl(&prices)?);
    println!("Equity:         {}", account.equity(&prices)?);
    println!("Free margin:    {}", account.free_margin(&prices)?);
    println!("Margin level:   {:?}%", account.margin_level_percent(&prices)?);

    Ok(())
}