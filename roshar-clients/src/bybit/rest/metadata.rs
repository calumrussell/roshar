use std::collections::HashMap;
use tokio::sync::mpsc;

use super::{ByBitInstrumentInfo, ByBitTickerData, MarketApi};

/// Handle for querying exchange metadata
/// Wraps the oneshot channel communication with the metadata manager
#[derive(Clone)]
pub struct ExchangeMetadataHandle {
    metadata_query_tx: mpsc::Sender<MetadataQuery>,
}

impl ExchangeMetadataHandle {
    fn new(metadata_query_tx: mpsc::Sender<MetadataQuery>) -> Self {
        Self { metadata_query_tx }
    }

    /// Get instruments info from metadata manager
    pub async fn get_instruments_info(&self) -> Result<HashMap<String, ByBitInstrumentInfo>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetInstrumentsInfo { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query instruments info: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive instruments info: {}", e))
    }

    /// Get ticker data from metadata manager
    pub async fn get_ticker_data(&self) -> Result<HashMap<String, ByBitTickerData>, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetTickerData { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query ticker data: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive ticker data: {}", e))
    }
}

enum MetadataQuery {
    GetInstrumentsInfo {
        reply: tokio::sync::oneshot::Sender<HashMap<String, ByBitInstrumentInfo>>,
    },
    GetTickerData {
        reply: tokio::sync::oneshot::Sender<HashMap<String, ByBitTickerData>>,
    },
}

/// Manages ByBit exchange metadata with periodic background updates
pub struct ExchangeMetadataManager {
    instruments_info: HashMap<String, ByBitInstrumentInfo>,
    ticker_data: HashMap<String, ByBitTickerData>,
    update_interval_secs: u64,
    query_rx: tokio::sync::mpsc::Receiver<MetadataQuery>,
    market_api: MarketApi,
}

impl ExchangeMetadataManager {
    /// Spawn a new metadata manager and return the handle and join handle.
    /// Creates a MarketApi instance internally with the specified rate limit.
    pub fn spawn(
        update_interval_secs: u64,
        requests_per_second: u32,
    ) -> (ExchangeMetadataHandle, tokio::task::JoinHandle<()>) {
        let (query_tx, query_rx) = tokio::sync::mpsc::channel(100);
        let manager = Self::new(update_interval_secs, query_rx, requests_per_second);
        let handle = tokio::spawn(async move {
            manager.run().await;
        });
        let metadata_handle = ExchangeMetadataHandle::new(query_tx);
        (metadata_handle, handle)
    }

    fn new(
        update_interval_secs: u64,
        query_rx: tokio::sync::mpsc::Receiver<MetadataQuery>,
        requests_per_second: u32,
    ) -> Self {
        let market_api = MarketApi::new(requests_per_second);

        Self {
            instruments_info: HashMap::new(),
            ticker_data: HashMap::new(),
            update_interval_secs,
            query_rx,
            market_api,
        }
    }

    /// Run the metadata manager loop that periodically updates ByBit metadata
    /// and handles queries. Caller should spawn this with tokio::spawn() to run in background.
    pub async fn run(mut self) {
        // Initial fetch
        if let Err(e) = self.update_instruments_info().await {
            log::error!("Initial ByBit instruments info fetch failed: {}", e);
        }
        if let Err(e) = self.update_ticker_data().await {
            log::error!("Initial ByBit ticker data fetch failed: {}", e);
        }

        // Periodic updates
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(self.update_interval_secs));

        loop {
            tokio::select! {
                // Handle queries
                Some(query) = self.query_rx.recv() => {
                    self.handle_query(query);
                }

                // Periodic metadata update
                _ = interval.tick() => {
                    if let Err(e) = self.update_instruments_info().await {
                        log::error!("Failed to update ByBit instruments info: {}", e);
                    }
                    if let Err(e) = self.update_ticker_data().await {
                        log::error!("Failed to update ByBit ticker data: {}", e);
                    }
                }

                else => {
                    log::info!("ExchangeMetadataManager channels closed, shutting down");
                    break;
                }
            }
        }
    }

    fn handle_query(&self, query: MetadataQuery) {
        match query {
            MetadataQuery::GetInstrumentsInfo { reply } => {
                let _ = reply.send(self.instruments_info.clone());
            }
            MetadataQuery::GetTickerData { reply } => {
                let _ = reply.send(self.ticker_data.clone());
            }
        }
    }

    async fn update_instruments_info(&mut self) -> Result<(), String> {
        let instruments_info = self
            .market_api
            .get_instruments_info()
            .await
            .map_err(|e| format!("Failed to fetch ByBit instruments info: {}", e))?;

        let count = instruments_info.len();
        self.instruments_info = instruments_info;
        log::debug!("Updated ByBit instruments info: {} instruments", count);

        Ok(())
    }

    async fn update_ticker_data(&mut self) -> Result<(), String> {
        let ticker_data = self
            .market_api
            .get_tickers()
            .await
            .map_err(|e| format!("Failed to fetch ByBit ticker data: {}", e))?;

        let count = ticker_data.len();
        self.ticker_data = ticker_data;
        log::debug!("Updated ByBit ticker data: {} tickers", count);

        Ok(())
    }
}
