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

// Derive (formerly Lyra)
pub const DERIVE_WSS_URL: &str = "wss://api.lyra.finance/ws";

// Aster (BNB Chain perp DEX — Binance Futures API compatible)
pub const ASTER_WSS_URL: &str = "wss://fstream.asterdex.com";

// Pacifica (Solana perp DEX — Hyperliquid-style CLOB)
pub const PACIFICA_WSS_URL: &str = "wss://ws.pacifica.fi/ws";

// Lighter (ZK-rollup perp DEX)
pub const LIGHTER_WSS_URL: &str = "wss://mainnet.zklighter.elliot.ai/stream";
pub const LIGHTER_TESTNET_WSS_URL: &str = "wss://testnet.zklighter.elliot.ai/stream";

// Paradex (StarkNet Appchain perp DEX)
pub const PARADEX_WSS_URL: &str = "wss://ws.api.prod.paradex.trade/v1";
pub const PARADEX_TESTNET_WSS_URL: &str = "wss://ws.api.testnet.paradex.trade/v1";

// Deribit
pub const DERIBIT_WSS_URL: &str = "wss://www.deribit.com/ws/api/v2";
pub const DERIBIT_TESTNET_WSS_URL: &str = "wss://test.deribit.com/ws/api/v2";
