//! One-way netting: open opposite position, observe reduction.

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    // Open Long 0.2 BTC
    let long = TradingPosition::new(
        1, "BTC", PositionSide::Long,
        dec!(0.2), dec!(50000),
        NonZeroU32::new(10).unwrap(), dec!(0.0125),
    )?;
    account.open_position(long, FeeTier::Taker)?;
    println!("After Long 0.2 BTC:");
    println!("  Positions: {}", account.positions().len());
    println!("  Used margin: {}", account.total_used_margin());

    // Open Short 0.05 BTC — should reduce the Long
    let short = TradingPosition::new(
        2, "BTC", PositionSide::Short,
        dec!(0.05), dec!(50000),
        NonZeroU32::new(10).unwrap(), dec!(0.0125),
    )?;
    let result = account.open_netting(short, dec!(50000), FeeTier::Taker)?;

    println!("\nAfter Short 0.05 BTC (netting):");
    println!("  Result: {:?}", result.is_some());
    println!("  Positions: {}", account.positions().len());
    if let Some(pos) = account.positions().first() {
        println!("  Remaining size: {}", pos.size);
        println!("  Side: {:?}", pos.side);
    }

    Ok(())
}