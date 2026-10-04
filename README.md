# hl-paper-trader

Paper-trading engine for [Hyperliquid](https://hyperliquid.xyz) perpetual futures.
Simulates PnL, fees, funding, margin, and liquidation in real time.

## Features

- Real-time price feed via Hyperliquid WebSocket (`allMids`, `activeAssetCtx`)
- Cross and Isolated margin modes
- One-way netting (as on Hyperliquid)
- Taker / Maker / Liquidation fee tiers
- Hourly funding rate
- Liquidation price estimation
- Realistic margin call / stop-out logic

## Status

Early development. API unstable.

## Usage

```rust
// example coming soon
