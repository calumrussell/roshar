// WebSocket URLs for exchanges

// Hyperliquid
pub const HL_WSS_URL: &str = "wss://api.hyperliquid.xyz/ws";
pub const HL_TESTNET_WSS_URL: &str = "wss://api.hyperliquid-testnet.xyz/ws";

// ByBit
pub const BYBIT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/linear";
pub const BYBIT_SPOT_WSS_URL: &str = "wss://stream.bybit.com/v5/public/spot";

// Kraken
pub const KRAKEN_WSS_URL: &str = "wss://futures.kraken.com/ws/v1";
pub const KRAKEN_SPOT_WSS_URL: &str = "wss://ws.kraken.com/v2";

// Binance
pub const BINANCE_WSS_URL: &str = "wss://fstream.binance.com/ws";
pub const BINANCE_SPOT_WSS_URL: &str = "wss://stream.binance.com:9443/ws";

// OKX
pub const OKX_WSS_URL: &str = "wss://ws.okx.com:8443/ws/v5/public";
pub const OKX_PRIVATE_WSS_URL: &str = "wss://ws.okx.com:8443/ws/v5/private";

// MEXC
pub const MEX_WSS_URL: &str = "wss://wbs.mexc.com/ws";

// KuCoin Futures
// Note: the actual connection URL is dynamic and must be obtained via
// POST https://api-futures.kucoin.com/api/v1/bullet-public (token appended as ?token=...).
// This constant is the base server endpoint for reference only.
pub const KUCOIN_FUTURES_WSS_URL: &str = "wss://ws-api-futures.kucoin.com/endpoint";
pub const KUCOIN_FUTURES_REST_URL: &str = "https://api-futures.kucoin.com";

// Bitget (USDT-margined perpetuals)
pub const BITGET_WSS_URL: &str = "wss://ws.bitget.com/v2/ws/public";
pub const BITGET_REST_URL: &str = "https://api.bitget.com";

// Coinbase Advanced Trade
pub const COINBASE_WSS_URL: &str = "wss://advanced-trade-ws.coinbase.com";
pub const COINBASE_REST_URL: &str = "https://api.coinbase.com";
