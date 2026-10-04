//! # Position views — convenient per-position inspection
//!
//! Demonstrates the helper methods on [`MarginAccount`]:
//!
//! - [`MarginAccount::position_pnl`] — gross PnL for one position.
//! - [`MarginAccount::position_liquidation_price`] — mode-aware liquidation price.
//! - [`MarginAccount::position_view`] — full snapshot.
//! - [`MarginAccount::all_position_views`] — snapshot of everything.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example position_view
//! ```

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header("Position views — practical examples");

    // ────────── Setup ──────────

    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));
    prices.insert("ETH".into(), dec!(3000));

    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    // Long BTC
    account.open_position(
        TradingPosition::from_margin(
            1,
            "BTC",
            PositionSide::Long,
            dec!(1000),
            dec!(50000),
            NonZeroU32::new(10).unwrap(),
            dec!(0.0125),
        )?,
        FeeTier::Taker,
    )?;

    // Short ETH
    account.open_position(
        TradingPosition::from_margin(
            2,
            "ETH",
            PositionSide::Short,
            dec!(500),
            dec!(3000),
            NonZeroU32::new(5).unwrap(),
            dec!(0.02),
        )?,
        FeeTier::Taker,
    )?;

    // ────────── 1. position_pnl ──────────

    print_section("1. Gross PnL of a single position");

    let pnl_btc = account.position_pnl(1, &prices)?;
    let pnl_eth = account.position_pnl(2, &prices)?;

    println!("BTC (Long):  {:?}", pnl_btc);
    println!("ETH (Short): {:?}", pnl_eth);
    println!("Unknown id:  {:?}", account.position_pnl(999, &prices)?);

    // ────────── 2. position_liquidation_price ──────────

    print_section("2. Liquidation price for one position");

    println!("Cross mode: liquidation price depends on the whole account.\n");

    let liq_btc = account.position_liquidation_price(1, &prices)?;
    let liq_eth = account.position_liquidation_price(2, &prices)?;

    println!("BTC liq. price: {:?}", liq_btc.map(|d| d.round_dp(2)));
    println!("ETH liq. price: {:?}", liq_eth.map(|d| d.round_dp(2)));

    // Switch to Isolated and compare
    account.set_margin_mode(MarginMode::Isolated);
    let liq_btc_iso = account.position_liquidation_price(1, &prices)?;

    println!("\nIsolated mode: each position liquidates on its own.\n");
    println!("BTC liq. price: {:?}", liq_btc_iso.map(|d| d.round_dp(2)));

    // Back to Cross for the rest
    account.set_margin_mode(MarginMode::Cross);

    // ────────── 3. position_view ──────────

    print_section("3. Full PositionView for one position");

    let view = account
        .position_view(1, &prices)?
        .ok_or("position 1 not found")?;

    println!("Symbol:             {}", view.position.symbol);
    println!("Side:               {:?}", view.position.side);
    println!("Size:               {}", view.position.size);
    println!("Entry price:        {}", view.position.entry_price);
    println!("Current price:      {}", view.current_price);
    println!("Gross PnL:          {}", view.gross_pnl.round_dp(4));
    println!("Net PnL:            {}", view.net_pnl.round_dp(4));
    println!("Maintenance margin: {}", view.maintenance_margin.round_dp(4));
    println!(
        "Liquidation price:  {:?}",
        view.liquidation_price.map(|d| d.round_dp(2))
    );

    // ────────── 4. all_position_views ──────────

    print_section("4. All positions at a glance");

    let views = account.all_position_views(&prices);
    println!(
        "{:<6} {:<8} {:>10} {:>14} {:>12}",
        "ID", "Symbol", "Cur. price", "Gross PnL", "Liq. price"
    );
    println!("{}", "─".repeat(56));
    for v in &views {
        println!(
            "{:<6} {:<8} {:>10} {:>14} {:>12}",
            v.position.id,
            v.position.symbol,
            v.current_price,
            v.gross_pnl.round_dp(2),
            v.liquidation_price
                .map(|d| d.round_dp(2).to_string())
                .unwrap_or_else(|| "—".to_string())
        );
    }

    // ────────── 5. Reacting to price changes ──────────

    print_section("5. Watching a position as the price moves");

    let scenarios = [dec!(50000), dec!(49000), dec!(48000), dec!(47000)];
    println!(
        "{:>10} {:>14} {:>16} {:>16}",
        "Price", "Gross PnL", "Net PnL", "Distance to liq"
    );
    println!("{}", "─".repeat(60));

    for price in scenarios {
        prices.insert("BTC".into(), price);
        let v = account.position_view(1, &prices)?.unwrap();

        let distance = match v.liquidation_price {
            Some(liq) => (price - liq).round_dp(2).to_string(),
            None => "—".to_string(),
        };

        println!(
            "{:>10} {:>14} {:>16} {:>16}",
            price,
            v.gross_pnl.round_dp(2),
            v.net_pnl.round_dp(2),
            distance
        );
    }

    // ────────── 6. Combining with account-level stats ──────────

    print_section("6. Per-position + account-wide stats together");

    prices.insert("BTC".into(), dec!(48000));

    println!("Account-level:");
    println!("  Balance:      {}", account.balance().round_dp(4));
    println!("  Equity:       {}", account.equity(&prices)?.round_dp(4));
    println!("  Free margin:  {}", account.free_margin(&prices)?.round_dp(4));
    println!(
        "  Margin level: {:?}%",
        account.margin_level_percent(&prices)?.map(|d| d.round_dp(2))
    );
    println!("  Margin call:  {}", account.is_margin_call(&prices)?);
    println!("  Stop out:     {}", account.is_stop_out(&prices)?);

    println!("\nPer-position:");
    for v in account.all_position_views(&prices) {
        println!(
            "  #{} {}: gross PnL = {}, liq at {:?}",
            v.position.id,
            v.position.symbol,
            v.gross_pnl.round_dp(2),
            v.liquidation_price.map(|d| d.round_dp(2))
        );
    }

    // ────────── 7. Why views are not stored ──────────

    print_section("7. Why PositionView is created on the fly");

    println!("Views are recomputed against the latest prices every call.");
    println!("Nothing is cached — no chance to go stale.\n");

    prices.insert("BTC".into(), dec!(55000));
    let fresh = account.position_view(1, &prices)?.unwrap();
    println!("After price → 55,000: gross PnL = {}", fresh.gross_pnl.round_dp(2));

    prices.insert("BTC".into(), dec!(45000));
    let fresher = account.position_view(1, &prices)?.unwrap();
    println!("After price → 45,000: gross PnL = {}", fresher.gross_pnl.round_dp(2));

    println!("\nSame position, two different views — always consistent.");

    print_header("Done");
    Ok(())
}

// ───────────────────────── Pretty-printing helpers ─────────────────────────

fn print_header(title: &str) {
    let line = "═".repeat(70);
    println!("\n{}", line);
    println!("  {}", title);
    println!("{}\n", line);
}

fn print_section(title: &str) {
    let line = "─".repeat(70);
    println!("\n{}\n  {}\n{}", line, title, line);
}