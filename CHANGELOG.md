# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-05

### Added

- `MarginAccount` with Cross and Isolated margin modes.
- `TradingPosition` with `new` and `from_margin` constructors.
- `FeeTier` (Maker / Taker / Liquidation) with internal rate table.
- Unrealized PnL, margin, equity, and free margin computation.
- Hourly funding applied via `apply_funding`.
- One-way netting (`open_netting`) as on Hyperliquid.
- Margin-call and stop-out detection per margin mode.
- Liquidation price estimation: isolated and cross.
- Partial close (`close_partial`) with proportional fee / funding split.
- `PositionView` and per-position helper methods.
- `PortfolioSummary` and `positions_summary` for dashboards and logging.
- Seven usage examples under `examples/`.
- Full rustdoc on all public items.


[Unreleased]: https://github.com/BartScottPR0D/hl-paper-trader/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/BartScottPR0D/hl-paper-trader/releases/tag/v0.1.0