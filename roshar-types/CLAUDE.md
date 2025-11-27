# Exchange Types

## Purpose
Library crate that provides unified type definitions and message parsing for multiple cryptocurrency exchanges. Acts as the central type system for websocket message handling across Hyperliquid, MEXC, ByBit, and Kraken exchanges.

## Inputs
- JSON strings from exchange websocket feeds
- Exchange identifier strings ("hl", "mex", "bybit", "bybit-spot", "kraken-spot", "kraken")

## How it runs
- **Library crate** - No binary, used as dependency by other crates
- Exports unified `SupportedMessages` enum that can parse messages from any supported exchange
- Provides constants for websocket URLs for each exchange

## Main functionality
- Message parsing via `SupportedMessages::from_message()` and `from_message_with_date()`
- Type definitions for order book, trades, and market data across exchanges
- Exchange-specific implementations for Hyperliquid, MEXC, ByBit (spot/derivatives), and Kraken (spot/derivatives)

## Test coverage
Has test coverage across 6 modules:
- `common.rs` - Core shared types
- `kraken/multi_collateral.rs` - Kraken multi-collateral trading
- `kraken/order_management.rs` - Kraken order management
- `hyperliquid/exchange.rs` - Hyperliquid exchange integration
- `hyperliquid/data.rs` - Hyperliquid data structures
- `hyperliquid/info.rs` - Hyperliquid info API

## Dependencies
Key dependencies include `serde` for JSON parsing, `tokio` for async support, `reqwest` for HTTP, and exchange-specific SDKs like `hyperliquid_rust_sdk`.