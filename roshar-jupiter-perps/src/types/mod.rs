//! Request and response bodies of the Jupiter Perps API (`https://perps-api.jup.ag/v2`).
//!
//! Field names and value formats follow the API. USD values, prices, token amounts and lamports
//! are decimal strings: most are raw integers with 6 decimals (USD) or the mint's decimals
//! (tokens); market stats, pool info and trade history use formatted decimals. Each field's doc
//! says which.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Mint of the SOL perpetual market (wrapped SOL).
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
/// Mint of the ETH perpetual market (Wormhole WETH).
pub const ETH_MINT: &str = "7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs";
/// Mint of the BTC perpetual market (Wormhole WBTC).
pub const BTC_MINT: &str = "3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh";
/// USDC mint (collateral for shorts, input/receive token).
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// A field the API always sends but documents as nullable. Without this, serde would accept a
/// missing key as `None`.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

/// A perpetual market (`asset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Asset {
    Btc,
    Eth,
    Sol,
}

impl Asset {
    /// The market mint the API's `mint` / `marketMint` parameters take for this asset.
    pub fn mint(self) -> &'static str {
        match self {
            Asset::Btc => BTC_MINT,
            Asset::Eth => ETH_MINT,
            Asset::Sol => SOL_MINT,
        }
    }
}

/// Token symbols. Input and receive tokens accept `BTC`, `ETH`, `SOL` and `USDC`; position
/// collateral and TP/SL desired tokens can also be `USDT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Token {
    Btc,
    Eth,
    Sol,
    Usdc,
    Usdt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Long,
    Short,
}

/// `tp` (take profit) or `sl` (stop loss).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TpslRequestType {
    Tp,
    Sl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeAction {
    Increase,
    Decrease,
}

/// `Market`: placed by the trader. `Trigger`: limit or TP/SL executed by a keeper.
/// `Liquidation`: liquidated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeOrderType {
    Market,
    Trigger,
    Liquidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionName {
    #[serde(rename = "SOL-PERP")]
    SolPerp,
    #[serde(rename = "ETH-PERP")]
    EthPerp,
    #[serde(rename = "BTC-PERP")]
    BtcPerp,
}

/// `transactionType` in TP/SL responses. `instant` transactions need the keeper's co-signature:
/// submit them through [`crate::TradingApi::execute`] (`requireKeeperSignature` is then `true`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Legacy,
    Instant,
}

/// `transactionType` accepted by `POST /tpsl` and `PATCH /tpsl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TpslTransactionTypeRequest {
    Instant,
    Mixed,
}

/// `action` of `POST /transaction/execute`: what the signed transaction does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecuteAction {
    CancelTpsl,
    CreateTpsl,
    UpdateTpsl,
    Tpsl,
    TpslWithFee,
    CancelLimitOrder,
    CreateLimitOrder,
    UpdateLimitOrder,
    LimitOrder,
    IncreasePosition,
    IncreasePositionWithFee,
    DecreasePosition,
    DecreasePositionWithFee,
}

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

/// The API's error body: `{"code": "...", "message": "...", "metadata": ...}`. `metadata` is
/// absent on validation and 500 errors, `null` or an object (e.g. min/max trigger price) on 422s.
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A non-2xx reply with the API's error body.
#[derive(Debug, Clone)]
pub struct JupiterPerpsError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

impl fmt::Display for JupiterPerpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Jupiter Perps error ({}): {}: {}",
            self.status, self.code, self.message
        )?;
        if let Some(metadata) = &self.metadata {
            write!(f, " {metadata}")?;
        }
        Ok(())
    }
}

impl std::error::Error for JupiterPerpsError {}

// ---------------------------------------------------------------------------------------------
// Market data
// ---------------------------------------------------------------------------------------------

/// `GET /market-stats`. Formatted decimals (USD, percent).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStats {
    pub price: String,
    #[serde(rename = "priceChange24H")]
    pub price_change_24h: String,
    #[serde(rename = "priceHigh24H")]
    pub price_high_24h: String,
    #[serde(rename = "priceLow24H")]
    pub price_low_24h: String,
    /// 24 h volume, USD.
    pub volume: String,
}

/// `GET /pool-info`: per-market liquidity, utilisation, hourly borrow rates and fees.
/// Formatted decimals (USD, percent).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfo {
    pub long_available_liquidity: String,
    /// Base hourly borrow rate (%) for longs.
    pub long_borrow_rate_percent: String,
    pub long_utilization_percent: String,
    pub short_available_liquidity: String,
    /// Base hourly borrow rate (%) for shorts.
    pub short_borrow_rate_percent: String,
    pub short_utilization_percent: String,
    pub open_fee_percent: String,
    /// Seconds a position request may stay active before it is treated as stale.
    pub max_request_execution_sec: String,
    pub max_price_impact_fee_percent: String,
}

/// `GET /jlp-info`: JLP pool totals and per-custody state.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JlpInfo {
    pub aum_usd: String,
    pub aum_usd_formatted: String,
    pub aum_limit_usd: String,
    pub aum_limit_usd_formatted: String,
    pub jlp_price_usd: String,
    pub jlp_price_usd_formatted: String,
    pub jlp_total_supply: String,
    pub jlp_total_supply_formatted: String,
    pub jlp_apr_bps: String,
    pub jlp_apr_pct: String,
    pub jlp_apy_bps: String,
    pub jlp_apy_pct: String,
    /// UNIX seconds.
    pub jlp_apr_last_updated_timestamp: String,
    pub jlp_realized_fee_usd: String,
    pub custodies: Vec<JlpCustody>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JlpCustody {
    pub pubkey: String,
    pub symbol: String,
    pub name: String,
    pub mint: String,
    pub aum_usd: String,
    pub aum_usd_formatted: String,
    pub aum_token_amount: String,
    pub aum_token_amount_formatted: String,
    pub current_weightage_pct: String,
    pub target_weightage_pct: String,
    pub utilization_pct: String,
    /// Token amount owned by the pool (mint decimals).
    pub owned: String,
    /// Token amount locked for traders' collateral and leverage (mint decimals).
    pub locked: String,
    /// Open long size, USD (6 decimals).
    pub guaranteed_usd: String,
    /// Open short size, USD (6 decimals).
    pub global_short_sizes: String,
    pub global_short_average_price: String,
    pub short_pnl_delta: String,
    pub short_traders_has_profit: bool,
    pub total_staked_amount_lamports: String,
    pub total_staked_amount_usd: String,
    pub total_staked_amount_usd_formatted: String,
}

/// `GET /top-traders`. USD values are raw (6 decimals).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTraders {
    pub market_mint: String,
    pub total_volume_usd: String,
    /// UNIX seconds.
    pub start_timestamp: u64,
    /// UNIX seconds.
    pub end_timestamp: u64,
    pub top_traders_by_pnl: Vec<TraderTotals>,
    pub top_traders_by_volume: Vec<TraderTotals>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderTotals {
    pub owner: String,
    pub total_volume_usd: String,
    pub total_pnl_usd: String,
}

/// `GET /trader-stats`. USD values are raw (6 decimals).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderStats {
    pub total_volume_usd: String,
    pub total_pnl_usd: String,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

// ---------------------------------------------------------------------------------------------
// Wallet reads
// ---------------------------------------------------------------------------------------------

/// `{"count": n, "dataList": [...]}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataList<T> {
    pub count: u64,
    pub data_list: Vec<T>,
}

/// `GET /positions` item. USD values are raw (6 decimals); percentages are formatted.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub asset: Asset,
    pub asset_mint: String,
    pub collateral_token: Token,
    pub collateral_mint: String,
    pub position_pubkey: String,
    pub side: Side,
    /// Formatted, e.g. `"5.17"`; `"0"` on closed positions.
    pub leverage: String,
    pub size_usd: String,
    pub size_token_amount: String,
    pub collateral_usd: String,
    pub value_usd: String,
    pub entry_price_usd: String,
    pub mark_price_usd: String,
    pub liquidation_price_usd: String,
    pub open_fees_usd: String,
    pub close_fees_usd: String,
    pub borrow_fees_usd: String,
    pub total_fees_usd: String,
    pub pnl_before_fees_usd: String,
    pub pnl_before_fees_pct: String,
    pub pnl_after_fees_usd: String,
    pub pnl_after_fees_pct: String,
    /// UNIX seconds.
    pub created_time: u64,
    /// UNIX seconds.
    pub updated_time: u64,
    pub tpsl_requests: Vec<PositionTpsl>,
}

/// An active TP/SL request on a [`Position`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionTpsl {
    pub position_request_pubkey: String,
    pub request_type: TpslRequestType,
    pub desired_mint: String,
    pub desired_token: Token,
    pub collateral_usd_delta: String,
    pub entire_position: bool,
    pub size_usd: String,
    pub size_percentage: String,
    #[serde(deserialize_with = "nullable")]
    pub trigger_price_usd: Option<String>,
    /// UNIX seconds.
    pub open_time: String,
}

/// `GET /orders/limit` item. USD values and prices are raw (6 decimals).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitOrder {
    pub collateral_mint: String,
    pub collateral_custody: String,
    pub collateral_usd: String,
    pub collateral_usd_at_trigger_price: String,
    pub collateral_token_amount: String,
    /// Custody account of the market.
    pub custody: String,
    pub executed: bool,
    pub input_mint: String,
    pub liquidation_price_usd: String,
    pub market_mint: String,
    pub max_size_usd_delta: String,
    pub min_size_usd_delta: String,
    /// UNIX seconds.
    pub open_time: String,
    pub position_pubkey: String,
    pub position_request_pubkey: String,
    pub side: Side,
    pub size_usd_delta: String,
    #[serde(deserialize_with = "nullable")]
    pub trigger_price: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub trigger_to_liquidation_percent: Option<String>,
}

/// `GET /trades` item. USD values here are formatted decimals.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub action: TradeAction,
    pub collateral_usd_delta: String,
    /// UNIX seconds.
    pub created_time: u64,
    pub base_fee: String,
    pub borrow_fee: String,
    pub liquidation_fee: String,
    pub price_impact_fee: String,
    /// Total fees including any liquidation penalty.
    pub fee: String,
    #[serde(default)]
    pub integrator_fee_token_amount: Option<String>,
    #[serde(default)]
    pub integrator_fee_token_mint: Option<String>,
    pub mint: String,
    pub order_type: TradeOrderType,
    pub owner: String,
    /// Before fees; null on increases.
    #[serde(default)]
    pub pnl: Option<String>,
    #[serde(default)]
    pub pnl_percentage: Option<String>,
    pub position_name: PositionName,
    pub position_pubkey: String,
    pub price: String,
    pub side: Side,
    pub size: String,
    #[serde(default)]
    pub transaction_fee_lamports: Option<String>,
    #[serde(default)]
    pub priority_fee_lamports: Option<String>,
    #[serde(default)]
    pub swap_fee_bps: Option<String>,
    #[serde(default)]
    pub swap_fee_usd: Option<String>,
    #[serde(default)]
    pub swap_fee_token_amount: Option<String>,
    #[serde(default)]
    pub swap_fee_token_mint: Option<String>,
    pub tx_hash: String,
    /// UNIX seconds.
    pub updated_time: u64,
}

/// `GET /trades` filters. `wallet_address` is required unless `txid` is set.
#[derive(Debug, Clone, Default)]
pub struct TradesQuery<'a> {
    pub wallet_address: Option<&'a str>,
    pub txid: Option<&'a str>,
    pub action: Option<TradeAction>,
    /// Market mint.
    pub mint: Option<&'a str>,
    pub side: Option<Side>,
    /// UNIX seconds, inclusive.
    pub created_at_after: Option<u64>,
    /// UNIX seconds, exclusive.
    pub created_at_before: Option<u64>,
    /// First row, inclusive (API default 0).
    pub start: Option<u64>,
    /// Last row, exclusive (API default 20).
    pub end: Option<u64>,
}

/// `GET /positions/collateral-limits`. USD values raw (6 decimals), token amounts in the input
/// mint's decimals.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollateralLimits {
    pub max_collateral_deposit_token_amount: String,
    pub max_collateral_deposit_usd: String,
    pub min_collateral_deposit_token_amount: String,
    pub min_collateral_deposit_usd: String,
    pub max_collateral_withdrawal_token_amount: String,
    pub max_collateral_withdrawal_usd: String,
    pub min_collateral_withdrawal_token_amount: String,
    pub min_collateral_withdrawal_usd: String,
}

// ---------------------------------------------------------------------------------------------
// Transaction building: shared pieces
// ---------------------------------------------------------------------------------------------

/// Blockhash the returned transaction was built with. Confirm against these when submitting
/// through your own RPC.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockhashMetadata {
    pub blockhash: String,
    pub last_valid_block_height: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxMetadata {
    pub blockhash: String,
    pub last_valid_block_height: String,
    pub transaction_fee_lamports: String,
    pub account_rent_lamports: String,
}

/// One transaction of a batch (`POST /positions/close-all`, `GET /orders/expired`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedTx {
    pub serialized_tx_base64: String,
    pub position_request_pubkey: String,
}

/// `POST /positions/close-all` and `GET /orders/expired`: one unsigned transaction per position
/// (or expired order). Empty when there is nothing to close.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedTxBatch {
    pub serialized_txs: Vec<SerializedTx>,
    pub tx_metadata: BlockhashMetadata,
}

/// Quote of an increase (market or limit order). USD values and prices raw (6 decimals).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncreaseQuote {
    pub average_price_usd: String,
    pub collateral_usd_delta: String,
    pub leverage: String,
    pub liquidation_price_usd: String,
    pub open_fee_usd: String,
    pub outstanding_borrow_fee_usd: String,
    pub position_collateral_usd: String,
    pub position_size_usd: String,
    pub price_impact_fee_bps: String,
    pub price_impact_fee_usd: String,
    pub side: Side,
    pub size_usd_delta: String,
    /// In the market token's decimals.
    pub size_token_delta: String,
}

/// Quote of a decrease. USD values and prices raw (6 decimals).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecreaseQuote {
    pub close_fee_usd: String,
    pub collateral_usd_delta: String,
    pub leverage: String,
    pub liquidation_price_usd: String,
    pub outstanding_borrow_fee_usd: String,
    pub pnl_after_fees_usd: String,
    pub pnl_after_fees_percent: String,
    pub pnl_before_fees_usd: String,
    pub pnl_before_fees_percent: String,
    pub position_collateral_usd: String,
    pub position_size_usd: String,
    pub price_impact_fee_bps: String,
    pub price_impact_fee_usd: String,
    pub side: Side,
    pub size_usd_delta: String,
    pub total_fee_usd: String,
    /// In `transfer_token_mint`'s decimals.
    pub transfer_amount_token: String,
    pub transfer_amount_usd: String,
    pub transfer_token_mint: String,
}

// ---------------------------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------------------------

/// TP/SL attached to `POST /positions/increase` (at most 2).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncreaseTpsl {
    pub receive_token: Token,
    /// Raw, 6 decimals.
    pub trigger_price: String,
    pub request_type: TpslRequestType,
}

/// `POST /positions/increase`: open or add to a position at market, or deposit collateral.
/// Pass either `leverage` or `size_usd_delta`. Without `wallet_address` the API only quotes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncreasePositionRequest {
    pub asset: Asset,
    pub input_token: Token,
    /// Integer amount in the input token's decimals.
    pub input_token_amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<String>,
    pub max_slippage_bps: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_fee_micro_lamports: Option<String>,
    pub side: Side,
    /// Raw, 6 decimals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_usd_delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tpsl: Option<Vec<IncreaseTpsl>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
}

/// TP/SL created alongside an increase.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncreaseTpslResult {
    pub has_profit: bool,
    pub pnl_usd: String,
    pub pnl_percent: String,
    /// Position request account of the TP/SL.
    pub pubkey: String,
    pub request_type: TpslRequestType,
}

/// `POST /positions/increase` reply. `position_pubkey`, `serialized_tx_base64` and
/// `tx_metadata` are null for quote-only requests (no `walletAddress`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncreasePositionResponse {
    #[serde(deserialize_with = "nullable")]
    pub position_pubkey: Option<String>,
    pub quote: IncreaseQuote,
    #[serde(deserialize_with = "nullable")]
    pub serialized_tx_base64: Option<String>,
    pub tpsl: Vec<IncreaseTpslResult>,
    #[serde(deserialize_with = "nullable")]
    pub tx_metadata: Option<TxMetadata>,
}

/// `POST /positions/decrease`: reduce, close, or withdraw collateral.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecreasePositionRequest {
    pub position_pubkey: String,
    pub receive_token: Token,
    /// Raw, 6 decimals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collateral_usd_delta: Option<String>,
    /// Raw, 6 decimals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_usd_delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entire_position: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_slippage_bps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_fee_micro_lamports: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecreasePositionResponse {
    pub position_pubkey: String,
    pub quote: DecreaseQuote,
    pub serialized_tx_base64: String,
    pub tx_metadata: TxMetadata,
}

// ---------------------------------------------------------------------------------------------
// Limit orders
// ---------------------------------------------------------------------------------------------

/// `POST /orders/limit`. Pass either `leverage` or `size_usd_delta`. The transaction is
/// returned only with `wallet_address` and `include_serialized_tx: Some(true)`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLimitOrderRequest {
    pub asset: Asset,
    /// Must be the market token or USDC.
    pub input_token: Token,
    /// Integer amount in the input token's decimals.
    pub input_token_amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<String>,
    pub side: Side,
    /// Raw, 6 decimals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_usd_delta: Option<String>,
    /// Raw, 6 decimals.
    pub trigger_price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_serialized_tx: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLimitOrderResponse {
    #[serde(deserialize_with = "nullable")]
    pub position_pubkey: Option<String>,
    pub quote: IncreaseQuote,
    #[serde(deserialize_with = "nullable")]
    pub serialized_tx_base64: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub tx_metadata: Option<TxMetadata>,
}

/// `PATCH /orders/limit`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLimitOrderRequest {
    pub position_request_pubkey: String,
    /// Raw, 6 decimals.
    pub trigger_price: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLimitOrderResponse {
    #[serde(deserialize_with = "nullable")]
    pub position_pubkey: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub position_request_pubkey: Option<String>,
    pub quote: IncreaseQuote,
    #[serde(deserialize_with = "nullable")]
    pub serialized_tx_base64: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub tx_metadata: Option<TxMetadata>,
}

/// `DELETE /orders/limit` reply.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelLimitOrderResponse {
    #[serde(deserialize_with = "nullable")]
    pub serialized_tx_base64: Option<String>,
    pub tx_metadata: TxMetadata,
    #[serde(deserialize_with = "nullable")]
    pub position_pubkey: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub position_request_pubkey: Option<String>,
    #[serde(default)]
    pub require_keeper_signature: Option<bool>,
}

// ---------------------------------------------------------------------------------------------
// TP / SL
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TpslSpec {
    pub receive_token: Token,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entire_position: Option<bool>,
    /// Raw, 6 decimals. Required when `entire_position` is not `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_usd_delta: Option<String>,
    /// Raw, 6 decimals.
    pub trigger_price: String,
    pub request_type: TpslRequestType,
}

/// `POST /tpsl`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTpslRequest {
    pub wallet_address: String,
    pub position_pubkey: String,
    pub tpsl: Vec<TpslSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TpslTransactionTypeRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TpslEstimate {
    pub estimated_pnl_usd: String,
    pub estimated_pnl_percent: String,
    pub has_profit: bool,
    pub request_type: TpslRequestType,
    pub position_request_pubkey: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTpslResponse {
    pub tpsl_pubkeys: Vec<String>,
    /// When true, submit through `POST /transaction/execute` so the keeper co-signs.
    pub require_keeper_signature: bool,
    pub serialized_tx_base64: String,
    pub tpsl_requests: Vec<TpslEstimate>,
    pub transaction_type: TransactionType,
    pub tx_metadata: TxMetadata,
}

/// `PATCH /tpsl`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTpslRequest {
    pub position_request_pubkey: String,
    /// Raw, 6 decimals.
    pub trigger_price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TpslTransactionTypeRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTpslResponse {
    pub require_keeper_signature: bool,
    pub serialized_tx_base64: String,
    pub tpsl_requests: Vec<TpslEstimate>,
    pub transaction_type: TransactionType,
    pub tx_metadata: TxMetadata,
}

/// `DELETE /tpsl` reply.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTpslResponse {
    pub require_keeper_signature: bool,
    pub serialized_tx_base64: String,
    pub transaction_type: TransactionType,
    pub tx_metadata: TxMetadata,
}

// ---------------------------------------------------------------------------------------------
// Submission
// ---------------------------------------------------------------------------------------------

/// `POST /transaction/execute`: submit a signed transaction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRequest {
    pub action: ExecuteAction,
    /// The signed wire transaction, base64.
    pub serialized_tx_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResponse {
    pub action: String,
    pub txid: String,
}
