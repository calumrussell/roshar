mod exchange;
mod info;
pub mod metadata;

pub use exchange::{ExchangeApi, HyperliquidOrderType, ModifyOrderParams, ScheduleCancel};
pub use info::InfoApi;
pub use metadata::{ExchangeMetadataHandle, ExchangeMetadataManager, SpotAssetInfo};

// Re-export SDK types that users need
pub use hyperliquid_rust_sdk::{
    ClientLimit, ClientModifyRequest, ClientOrder, ClientOrderRequest, ExchangeClient,
    ExchangeDataStatus, ExchangeResponseStatus,
};
