//! # Maintenance Margin Rate — deep dive
//!
//! This example explains what `maintenance_margin_rate` is, where it comes
//! from, how it is calculated, and what it affects in the paper-trading engine.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example maintenance_margin
//! ```
//!
//! ## What is maintenance margin?
//!
//! Maintenance margin is the **minimum equity** that must remain on the
//! account for an open position to stay alive. If equity drops below it,
//! the position is subject to liquidation.
//!
//! ## Where does the rate come from?
//!
//! Hyperliquid does not expose `maintenance_margin_rate` directly.
//! It is derived from the maximum allowed leverage of the asset:
//!
//! ```text
//! maintenance_margin_rate = 1 / (2 * max_leverage)
//! ```
//!
//! The `max_leverage` per asset comes from the `metaAndAssetCtxs` endpoint:
//!
//! ```json
//! {
//!   "universe": [
//!     { "name": "BTC", "maxLeverage": 40 },
//!     { "name": "ETH", "maxLeverage": 25 },
//!     { "name": "SOL", "maxLeverage": 20 }
//!   ]
//! }
//! ```
//!
//! ## What does it affect?
//!
//! 1. `TradingPosition::maintenance_margin()` — the absolute margin required.
//! 2. `TradingPosition::liquidation_price_isolated()` — liquidation price
//!    for isolated positions.
//! 3. `MarginAccount::liquidation_price_cross()` — liquidation price for
//!    cross margin accounts.
//!
//! It does **not** affect margin-call or stop-out ratios directly — those
//! use `margin_call_ratio` and `stop_out_ratio` on the account level.

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

/// Computes the maintenance margin rate from the asset's max leverage.
///
/// Hyperliquid formula: `rate = 1 / (2 * max_leverage)`.
fn maintenance_rate_from_max_leverage(max_leverage: u32) -> Decimal {
    Decimal::ONE / (Decimal::from(2) * Decimal::from(max_leverage))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header("Maintenance Margin Rate — reference");

    // ────────── 1. Where does the number come from? ──────────

    print_section("1. Deriving the rate from max leverage");

    println!("Formula: rate = 1 / (2 * max_leverage)\n");

    let assets: &[(&str, u32)] = &[
        ("BTC", 40),
        ("ETH", 25),
        ("SOL", 20),
        ("DOGE", 10),
        ("SHIB", 5),
        ("HPOS", 3),
    ];

    println!("{:<8} {:>14} {:>22} {:>18}", "Asset", "Max leverage", "Maintenance rate", "Rate (%)");
    println!("{}", "-".repeat(66));

    for (name, max_lev) in assets {
        let rate = maintenance_rate_from_max_leverage(*max_lev);
        let pct = rate * dec!(100);
        println!(
            "{:<8} {:>13}x {:>22} {:>17}%",
            name,
            max_lev,
            format!("{}", rate.round_dp(6)),
            format!("{}", pct.round_dp(4)),
        );
    }

    // ────────── 2. What does the rate mean in dollars? ──────────

    print_section("2. From rate to absolute margin");

    println!("For a $10,000 notional position:\n");

    for (name, max_lev) in assets {
        let rate = maintenance_rate_from_max_leverage(*max_lev);
        let notional = dec!(10000);
        let mm = notional * rate;
        println!(
            "{:<8} maintenance margin = ${:<10} ({}% of notional)",
            name,
            mm.round_dp(2),
            (rate * dec!(100)).round_dp(4),
        );
    }

    // ────────── 3. Impact on the trading position ──────────

    print_section("3. How the rate affects a TradingPosition");

    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    // Same position parameters, different maintenance rates.
    let configs: &[(&str, Decimal)] = &[
        ("Low rate (40x asset)", dec!(0.0125)),
        ("Mid rate (20x asset)", dec!(0.025)),
        ("High rate (5x asset)", dec!(0.1)),
    ];

    println!(
        "{:<25} {:>15} {:>20} {:>25}",
        "Scenario", "Maintenance", "Liq. price", "Distance from 50k"
    );
    println!("{}", "-".repeat(88));

    for (label, rate) in configs {
        let pos = TradingPosition::new(
            1,
            "BTC",
            PositionSide::Long,
            dec!(0.1),
            dec!(50000),
            NonZeroU32::new(10).unwrap(),
            *rate,
        )?;

        let price = dec!(50000);
        let mm = pos.maintenance_margin(price);
        let liq = pos.liquidation_price_isolated(price).unwrap_or(Decimal::ZERO);
        let distance = (liq - price).abs();

        println!(
            "{:<25} {:>15} {:>20} {:>25}",
            label,
            format!("${}", mm.round_dp(2)),
            format!("${}", liq.round_dp(2)),
            format!("${}", distance.round_dp(2)),
        );
    }

    println!("\nObservation:");
    println!("  Higher rate → higher maintenance margin → liquidation price closer");
    println!("  to entry. A 5x asset is liquidated much earlier than a 40x asset");

    // ────────── 4. Isolated liquidation price formula ──────────

    print_section("4. Isolated liquidation price formula");

    println!("From TradingPosition::liquidation_price_isolated:\n");
    println!("  p_liq = p - side_sign * margin_available / size / (1 - l * side_sign)");
    println!();
    println!("  where:");
    println!("    p               = current market price");
    println!("    side_sign       = +1 for Long, -1 for Short");
    println!("    margin_available = isolated_equity(p) - maintenance_margin(p)");
    println!("    l               = maintenance_margin_rate");
    println!();

    // Concrete numeric example
    let rate = dec!(0.0125);
    let pos = TradingPosition::new(
        1,
        "BTC",
        PositionSide::Long,
        dec!(0.1),
        dec!(50000),
        NonZeroU32::new(10).unwrap(),
        rate,
    )?;

    let p = dec!(50000);
    let isolated_eq = pos.isolated_equity(p);
    let mm = pos.maintenance_margin(p);
    let margin_available = isolated_eq - mm;
    let liq = pos.liquidation_price_isolated(p).unwrap();

    println!("Concrete example (Long, size=0.1 BTC, entry=50,000, 10x):");
    println!("  isolated_equity      = {}", isolated_eq);
    println!("  maintenance_margin   = size * p * l = 0.1 * 50000 * 0.0125 = {}", mm);
    println!("  margin_available     = {} - {} = {}", isolated_eq, mm, margin_available);
    println!("  liquidation price    = {}", liq);
    println!(
        "  Distance from entry  = {} ({:.2}% drop)",
        (p - liq).round_dp(2),
        ((p - liq) / p * dec!(100)).round_dp(2)
    );

    // ────────── 5. Cross liquidation price ──────────

    print_section("5. Cross margin liquidation price");

    println!("Cross mode uses the whole account equity as a buffer, so the");
    println!("liquidation price depends on other positions in the account.\n");

    let mut account = MarginAccount::new(dec!(5000), MarginMode::Cross)?;
    account.open_position(
        TradingPosition::new(
            1,
            "BTC",
            PositionSide::Long,
            dec!(0.1),
            dec!(50000),
            NonZeroU32::new(10).unwrap(),
            dec!(0.0125),
        )?,
        FeeTier::Taker,
    )?;

    let liq_cross = account.liquidation_price_cross("BTC", &prices)?;
    println!("One position only:");
    println!("  BTC long, 0.1 @ 50,000, balance = 5000");
    println!("  Cross liq. price = {:?}\n", liq_cross.map(|d| d.round_dp(2)));

    // Add a profitable position → cross liquidation price moves further
    prices.insert("ETH".into(), dec!(3000));
    account.open_position(
        TradingPosition::new(
            2,
            "ETH",
            PositionSide::Short,
            dec!(1),
            dec!(3100),
            NonZeroU32::new(10).unwrap(),
            dec!(0.0125),
        )?,
        FeeTier::Taker,
    )?;

    let liq_cross2 = account.liquidation_price_cross("BTC", &prices)?;
    println!("Two positions (BTC long + ETH short in profit):");
    println!("  Cross liq. price = {:?}", liq_cross2.map(|d| d.round_dp(2)));
    println!("  ETH profit adds buffer → BTC liquidates later");

    // ────────── 6. Practical recommendations ──────────

    print_section("6. Practical recommendations");

    println!("For paper-trading with Hyperliquid:");
    println!();
    println!("  • Query `metaAndAssetCtxs` to get `maxLeverage` per asset.");
    println!("  • Compute rate as `1 / (2 * maxLeverage)`.");
    println!("  • Cache it per symbol — it does not change often.");
    println!();
    println!("For quick prototyping:");
    println!();
    println!("  • Use `dec!(0.0125)` — the BTC/ETH 40x rate.");
    println!("  • Accepts slightly wrong liquidation prices for lower-leverage alts.");
    println!();
    println!("Tier system (large notional):");
    println!();
    println!("  • Hyperliquid raises the effective rate for notionals > $150M.");
    println!("  • For paper-trading with typical balances (< $1M) — ignore.");
    println!("  • If you simulate whales, apply tier tables from `marginTables`");

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