//! # Portfolio summary — one object for the whole account
//!
//! Demonstrates [`MarginAccount::positions_summary`]:
//! a single call that aggregates PnL, margin, and risk across all positions.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example portfolio_summary
//! ```

use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};
use rust_decimal_macros::dec;
use std::num::NonZeroU32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header("Portfolio summary — dashboard in one call");

    // ────────── Setup ──────────

    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));
    prices.insert("ETH".into(), dec!(3000));
    prices.insert("SOL".into(), dec!(150));

    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    account.open_position(
        TradingPosition::from_margin(
            1, "BTC", PositionSide::Long,
            dec!(1000), dec!(50000),
            NonZeroU32::new(10).unwrap(), dec!(0.0125),
        )?,
        FeeTier::Taker,
    )?;

    account.open_position(
        TradingPosition::from_margin(
            2, "ETH", PositionSide::Short,
            dec!(500), dec!(3000),
            NonZeroU32::new(5).unwrap(), dec!(0.02),
        )?,
        FeeTier::Taker,
    )?;

    account.open_position(
        TradingPosition::from_margin(
            3, "SOL", PositionSide::Long,
            dec!(300), dec!(150),
            NonZeroU32::new(3).unwrap(), dec!(0.05),
        )?,
        FeeTier::Taker,
    )?;

    // ────────── 1. Fresh summary ──────────

    print_section("1. Summary right after opening");

    let summary = account.positions_summary(&prices)?;

    println!("Positions:              {}", summary.position_count);
    println!("Total gross PnL:        {}", summary.total_gross_pnl.round_dp(4));
    println!("Total net PnL:          {}", summary.total_net_pnl.round_dp(4));
    println!("Total used margin:      {}", summary.total_used_margin.round_dp(4));
    println!(
        "Total maintenance:      {}",
        summary.total_maintenance_margin.round_dp(4)
    );
    println!(
        "Nearest to liq:         {:?}",
        summary.nearest_liquidation_id
    );
    println!(
        "Distance to liq:        {:?}",
        summary
            .nearest_liquidation_distance
            .map(|d| d.round_dp(2))
    );
    println!(
        "Distance (fraction):    {:?}",
        summary
            .nearest_liquidation_distance_frac
            .map(|d| d.round_dp(4))
    );

    // ────────── 2. Price moves ──────────

    print_section("2. Watching the summary as prices move");

    let scenarios: &[(&str, rust_decimal::Decimal)] = &[
        ("BTC", dec!(50000)),
        ("BTC", dec!(49000)),
        ("BTC", dec!(47500)),
        ("BTC", dec!(46000)),
    ];

    println!(
        "{:>10} {:>14} {:>16} {:>18} {:>22}",
        "BTC", "Gross PnL", "Net PnL", "Nearest liq id", "Distance to liq (%)"
    );
    println!("{}", "─".repeat(84));

    for (sym, price) in scenarios {
        prices.insert((*sym).into(), *price);
        let s = account.positions_summary(&prices)?;

        let dist_pct = s
            .nearest_liquidation_distance_frac
            .map(|d| (d * dec!(100)).round_dp(2))
            .map(|d| format!("{}%", d))
            .unwrap_or_else(|| "—".to_string());

        println!(
            "{:>10} {:>14} {:>16} {:>18} {:>22}",
            price,
            s.total_gross_pnl.round_dp(2),
            s.total_net_pnl.round_dp(2),
            s.nearest_liquidation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "—".into()),
            dist_pct,
        );
    }

    // ────────── 3. Risk threshold check ──────────

    print_section("3. Risk threshold checks");

    prices.insert("BTC".into(), dec!(49500));
    let s = account.positions_summary(&prices)?;

    let thresholds = [dec!(0.20), dec!(0.10), dec!(0.05), dec!(0.02)];
    for t in thresholds {
        let pct = (t * dec!(100)).round_dp(0);
        println!(
            "  Close to liquidation (< {}%)?  {}",
            pct,
            if s.is_close_to_liquidation(t) { "YES" } else { "no" }
        );
    }

    // ────────── 4. Combining with account-level stats ──────────

    print_section("4. Summary + account-level stats side by side");

    println!("Account-level:");
    println!("  Balance:            {}", account.balance().round_dp(4));
    println!("  Equity:             {}", account.equity(&prices)?.round_dp(4));
    println!(
        "  Free margin:        {}",
        account.free_margin(&prices)?.round_dp(4)
    );
    println!(
        "  Margin level:       {:?}%",
        account.margin_level_percent(&prices)?.map(|d| d.round_dp(2))
    );
    println!("  Margin call:        {}", account.is_margin_call(&prices)?);
    println!("  Stop out:           {}", account.is_stop_out(&prices)?);

    println!("\nPortfolio summary:");
    println!("  Positions:          {}", s.position_count);
    println!(
        "  Total gross PnL:    {}",
        s.total_gross_pnl.round_dp(4)
    );
    println!(
        "  Total maintenance:  {}",
        s.total_maintenance_margin.round_dp(4)
    );
    println!("  Total used margin:  {}", s.total_used_margin.round_dp(4));

    // ────────── 5. Empty account ──────────

    print_section("5. Empty account");

    let empty = MarginAccount::new(dec!(1000), MarginMode::Cross)?;
    let empty_summary = empty.positions_summary(&prices)?;

    println!("Positions:              {}", empty_summary.position_count);
    println!(
        "Total gross PnL:        {}",
        empty_summary.total_gross_pnl
    );
    println!(
        "Nearest liquidation:    {:?}",
        empty_summary.nearest_liquidation_id
    );
    println!(
        "Close to liq (<5%)?     {}",
        empty_summary.is_close_to_liquidation(dec!(0.05))
    );

    // ────────── 6. Log-friendly one-liner ──────────

    print_section("6. Log-friendly one-liner");

    prices.insert("BTC".into(), dec!(50000));
    let s = account.positions_summary(&prices)?;

    println!(
        "[portfolio] n={} pnl={} net={} used={} mm={} nearest_liq={:?} dist={:?}",
        s.position_count,
        s.total_gross_pnl.round_dp(2),
        s.total_net_pnl.round_dp(2),
        s.total_used_margin.round_dp(2),
        s.total_maintenance_margin.round_dp(2),
        s.nearest_liquidation_id,
        s.nearest_liquidation_distance_frac
            .map(|d| (d * dec!(100)).round_dp(2)),
    );

    print_header("Done");
    Ok(())
}

// ───────────────────────── Pretty-printing helpers ─────────────────────────

fn print_header(title: &str) {
    let line = "═".repeat(78);
    println!("\n{}", line);
    println!("  {}", title);
    println!("{}\n", line);
}

fn print_section(title: &str) {
    let line = "─".repeat(78);
    println!("\n{}\n  {}\n{}", line, title, line);
}