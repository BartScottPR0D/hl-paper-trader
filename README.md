# hl-paper-trader

Paper-trading engine for [Hyperliquid](https://hyperliquid.xyz) perpetual futures.
Simulates unrealized PnL, fees, funding, margin requirements, and liquidation
for a margin account with one or more open positions.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.98%2B-orange)](https://www.rust-lang.org)

## Features

- **Cross and Isolated margin modes** with separate stop-out logic.
- **One-way netting** — only one net position per symbol, as on Hyperliquid.
- **Fee tiers** — Maker (0.015%), Taker (0.045%), Liquidation (0%).
- **Hourly funding rate** applied per position.
- **Liquidation price** estimation for both Isolated and Cross modes.
- **Realistic margin call / stop-out** based on configurable ratios.
- **Position views** and **portfolio summary** for dashboards and logging.
- All monetary values use `rust_decimal::Decimal` for exact arithmetic.

## What this is not

- Not a price feed. You supply prices as a `HashMap<String, Decimal>`.
- Not a WebSocket client. Connect to Hyperliquid yourself.
- Not a full exchange simulator — commissions and funding are simplified
  where Hyperliquid's real rules are complex (e.g. tiered maintenance margin).

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
hl-paper-trader = { git = "https://github.com/your_username/hl-paper-trader.git", tag = "v0.1.0" }
```

Minimal example:

```rust
use std::num::NonZeroU32;
use rust_decimal_macros::dec;
use hl_paper_trader::{
    FeeTier, MarginAccount, MarginMode, PositionSide, Prices, TradingPosition,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut prices: Prices = Prices::new();
    prices.insert("BTC".into(), dec!(50000));

    let mut account = MarginAccount::new(dec!(10000), MarginMode::Cross)?;

    let position = TradingPosition::from_margin(
        1, "BTC", PositionSide::Long,
        dec!(1000),
        dec!(50000),
        NonZeroU32::new(10).unwrap(),
        dec!(0.0125),
    )?;

    account.open_position(position, FeeTier::Taker)?;

    println!("Equity:       {}", account.equity(&prices)?);
    println!("Free margin:  {}", account.free_margin(&prices)?);
    println!("Margin level: {:?}%", account.margin_level_percent(&prices)?);

    Ok(())
}
```

More examples in `examples/`:

| Example | What it shows |
|---|---|
| `basic` | Open a position, print account stats |
| `funding` | Apply hourly funding, observe balance |
| `netting` | One-way netting |
| `liquidation` | Drive a position into stop-out |
| `maintenance_margin` | Deep dive on the rate |
| `position_view` | Per-position helper methods |
| `portfolio_summary` | Aggregate dashboard in one call |

Run any of them with:

```bash
cargo run --example basic
```

## Documentation

Full API docs are available via `cargo doc`:

```bash
cargo doc --open
```

## Status

Early development (`0.x`). API may change between minor versions.

## License

Licensed under the [MIT License](LICENSE).