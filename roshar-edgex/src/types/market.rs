use serde::Deserialize;

/// `GET /api/v2/public/meta/getServerTime`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexServerTime {
    pub time_millis: String,
}

/// `GET /api/v2/public/meta/getMetaData`. `multiChain` (deposit/withdraw chains) and `campaign`
/// are not parsed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexMetaData {
    pub global: EdgexGlobal,
    pub coin_list: Vec<EdgexCoin>,
    pub contract_list: Vec<EdgexContract>,
}

/// `getMetaData().global`. `nativeChainId` and `contractAddress` form the L2 signing domain.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexGlobal {
    pub app_env: String,
    pub app_name: String,
    pub app_only_sign_on: String,
    pub booster_ratio: String,
    /// Hex chain id, e.g. `"0xd0f"`.
    pub chain_id: String,
    pub chase_price_bound_ratio: String,
    pub collateral_coin_id: String,
    pub contract_address: String,
    pub demo_self_claim_amount: String,
    pub demo_self_claim_equity_threshold: String,
    pub fast_withdraw_account_id: String,
    pub fast_withdraw_account_l2_key: String,
    pub fast_withdraw_max_amount: String,
    pub fast_withdraw_registry_address: String,
    pub fee_account_id: String,
    pub funding_validity_period: i64,
    pub isolated_margin_sign_permission: String,
    pub isolated_margin_signer: String,
    pub maintenance_reason: String,
    pub max_funding_rate: i64,
    pub min_transfer_size: String,
    /// Decimal chain id, e.g. `"3343"`.
    pub native_chain_id: String,
    pub perp_vault_address: String,
    pub pool_account_id: String,
    pub pool_account_l2_key: String,
    pub price_validity_period: i64,
    pub spot_vault_address: String,
    pub transfer_coin_id: String,
    pub transfer_limit_enable: bool,
    pub transfer_token_address: String,
    pub transfer_token_decimals: i64,
    pub twap_max_runtime: String,
    pub twap_min_runtime: String,
}

/// `getMetaData().coinList[]`. `assetId` and `resolution` are `null` for coins that are not
/// collateral.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexCoin {
    pub asset_id: Option<String>,
    pub coin_id: String,
    pub coin_name: String,
    pub icon_url: String,
    /// `0x`-hex or decimal, see `signer::parse_resolution`.
    pub resolution: Option<String>,
    pub show_step_size: String,
    pub step_size: String,
}

/// `getMetaData().contractList[]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexContract {
    pub base_coin_id: String,
    pub contract_id: String,
    pub contract_name: String,
    pub default_leverage: String,
    pub default_maker_fee_rate: String,
    pub default_taker_fee_rate: String,
    pub display_digit_merge: String,
    pub display_hot_icon: bool,
    pub display_max_leverage: String,
    pub display_min_leverage: String,
    pub display_new_icon: bool,
    pub enable_booster_credit: bool,
    pub enable_display: bool,
    pub enable_open_position: bool,
    pub enable_trade: bool,
    pub funding_impact_margin_notional: String,
    pub funding_interest_rate: String,
    pub funding_max_rate: String,
    pub funding_min_rate: String,
    pub funding_quote_impact_margin_notional: String,
    pub funding_rate_interval_min: String,
    pub is_fx: bool,
    pub is_stock: bool,
    pub liquidate_fee_rate: String,
    pub match_server_name: String,
    pub max_market_position_size: String,
    pub max_order_buy_price_ratio: String,
    pub max_order_size: String,
    pub max_position_size: String,
    pub min_order_sell_price_ratio: String,
    pub min_order_size: String,
    pub open_size_reduce_ratio: String,
    pub oracle_price_quorum: String,
    pub oracle_price_signed_asset_ids: Vec<String>,
    pub oracle_price_signers: Vec<String>,
    pub quote_coin_id: String,
    /// Synthetic resolution, e.g. `"10000000000"`.
    pub resolution: String,
    pub risk_tier_list: Vec<EdgexRiskTier>,
    pub step_size: String,
    pub synthetic_asset_id: String,
    pub tick_size: String,
    pub tpsl_spread_protection_threshold: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexRiskTier {
    pub maintenance_margin_rate: String,
    pub max_leverage: String,
    pub position_value_upper_bound: String,
    pub risk: String,
    pub tier: i64,
    pub upper_bound: String,
}

/// One price level: `{"price","size"}`. A `size` of `"0"` in a `CHANGED` update removes the level.
#[derive(Debug, Clone, Deserialize)]
pub struct EdgexBookLevel {
    pub price: String,
    pub size: String,
}

/// `GET /api/v2/public/quote/getDepth` item and the `depth.{contractId}.{level}` payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexDepth {
    pub start_version: String,
    pub end_version: String,
    pub level: u32,
    pub contract_id: String,
    pub contract_name: String,
    pub asks: Vec<EdgexBookLevel>,
    pub bids: Vec<EdgexBookLevel>,
    /// `SNAPSHOT` or `CHANGED`.
    pub depth_type: String,
}

/// `GET /api/v2/public/quote/getKline` item and the `kline.*` payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexKline {
    pub kline_id: String,
    pub contract_id: String,
    pub contract_name: String,
    pub kline_type: String,
    pub kline_time: String,
    pub price_type: String,
    pub trades: String,
    pub size: String,
    pub value: String,
    pub high: String,
    pub low: String,
    pub open: String,
    pub close: String,
    pub maker_buy_size: String,
    pub maker_buy_value: String,
}

/// `GET /api/v2/public/quote/getMultiContractKline` item.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexContractKlines {
    pub contract_id: String,
    pub kline_list: Vec<EdgexKline>,
}

/// `GET /api/v2/public/quote/getTicker` item (24 h window).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexTicker {
    pub contract_id: String,
    pub contract_name: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub trades: String,
    pub size: String,
    pub value: String,
    pub high: String,
    pub low: String,
    pub open: String,
    pub close: String,
    pub high_time: String,
    pub low_time: String,
    pub start_time: String,
    pub end_time: String,
    pub last_price: String,
    pub index_price: String,
    pub oracle_price: String,
    pub mark_price: String,
    pub open_interest: String,
    pub funding_rate: String,
    pub funding_time: String,
    pub next_funding_time: String,
}

/// `GET /api/v2/public/quote/getMarketStatus`. `contractId` is a JSON number here.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexMarketStatus {
    pub contract_id: i64,
    pub status: bool,
    pub force: bool,
    pub mark_price: String,
    pub limit_low_price: String,
    pub limit_high_price: String,
}

/// `GET /api/v2/public/funding/getLatestFundingRate` / `getFundingRatePage` item. The websocket
/// variant is `EdgexWsFundingRate`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexFundingRate {
    pub contract_id: String,
    pub funding_time: String,
    pub funding_timestamp: String,
    pub oracle_price: String,
    pub index_price: String,
    pub mark_price: String,
    pub funding_rate: String,
    pub is_settlement: bool,
    /// Empty on settlement records.
    pub forecast_funding_rate: String,
    pub previous_funding_rate: String,
    pub previous_funding_timestamp: String,
    pub premium_index: String,
    pub avg_premium_index: String,
    pub premium_index_timestamp: String,
    pub impact_margin_notional: String,
    pub impact_ask_price: String,
    pub impact_bid_price: String,
    pub interest_rate: String,
    pub predicted_funding_rate: String,
    pub funding_rate_interval_min: String,
    pub funding_index: String,
}
