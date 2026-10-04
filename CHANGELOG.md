# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Margin level as percentage (`margin_level_percent`).

## [0.1.0] - 2026-10-05

### Added
- Initial release.
- `MarginAccount` with Cross and Isolated modes.
- `TradingPosition` with `new` and `from_margin` constructors.
- `FeeTier` (Maker / Taker / Liquidation).
- Funding rate application.
- One-way netting.
- Stop-out and liquidation logic.

[Unreleased]: https://github.com/BartScottPR0D/hl-paper-trader/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/BartScottPR0D/hl-paper-trader/releases/tag/v0.1.0