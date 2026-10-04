//! Funding rate: apply hourly funding and observe balance changes.

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    // Long position
    let long = TradingPosition::from_margin(
        1, "BTC", PositionSide::Long,
        dec!(1000), dec!(50000),
        NonZeroU32::new(10).unwrap(), dec!(0.0125),
    )?;
    account.open_position(long, FeeTier::Taker)?;

    // Short position
    let short = TradingPosition::from_margin(
        2, "ETH", PositionSide::Short,
        dec!(1000), dec!(3000),
        NonZeroU32::new(10).unwrap(), dec!(0.0125),
    )?;
    account.open_position(short, FeeTier::Taker)?;

    println!("Balance before funding: {}", account.balance());

    // Positive funding rate: longs pay shorts
    let rate = dec!(0.0000125); // 0.00125% per hour
    let paid = account.apply_funding(&prices, rate)?;

    println!("Funding paid (positive = account paid): {}", paid);
    println!("Balance after funding:  {}", account.balance());

    // Apply funding for 24 hours
    for _ in 0..23 {
        account.apply_funding(&prices, rate)?;
    }
    println!("\nAfter 24h of funding:");
    println!("Balance: {}", account.balance());

    Ok(())
}