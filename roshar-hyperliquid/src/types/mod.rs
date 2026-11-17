pub mod exchange;
mod info;
mod market;
mod ws;

/// For `Option` fields the exchange always sends but may set to `null`: the key stays required
/// (serde's implicit `None` for a missing `Option` key does not apply with `deserialize_with`).
pub(crate) fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(d)
}

pub use info::{
    normalize_outcome_order_coin_to_token, normalize_outcome_token_to_order_coin, Asset, AssetInfo,
    AssetPosition, CrossMarginSummary, EvmContract, HistoricalFundingRate, InfoApiRequest,
    Leverage, MarginTable, MarginTier, MarketData, MetaAndAssetCtxs, Position, SpotAsset,
    SpotBalance, SpotClearinghouseState, SpotMarketData, SpotMeta, SpotMetaAndAssetCtxs, SpotToken,
    Universe, UserPerpetualsState,
};
pub use market::{
    OutcomeDeployer, OutcomeMeta, OutcomeSideSpec, OutcomeSpec, OutcomeTemplate,
    OutcomeTemplateRole, PerpAnnotation, PerpConciseAnnotation, PerpDex, PerpDexLimits,
    PerpDexStatus, PredictedFunding, PredictedFundings, QuestionSpec, SettledOutcome, TokenDetails,
    TokenGenesis,
};
pub use ws::{
    decode_fast_asset_ctxs, FastAssetCtx, HyperliquidBbo, HyperliquidBboMessage, HyperliquidBook,
    HyperliquidBookLevel, HyperliquidBookMessage, HyperliquidCandleData, HyperliquidCandleMessage,
    HyperliquidOrderUpdatesMessage, HyperliquidTrade, HyperliquidTradesMessage,
    HyperliquidUserFill, HyperliquidUserFillsData, HyperliquidUserFillsMessage, HyperliquidWsEvent,
    HyperliquidWssMessage, HyperliquidWssSubscription, WsActiveAssetCtx, WsActiveSpotAssetCtx,
    WsAllDexsAssetCtxs, WsAllMids, WsBasicOrder, WsLevel, WsOrder, WsOutcomeMetaUpdate,
};
