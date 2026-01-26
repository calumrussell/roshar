use roshar_types::{ExchangeInfo, TickerData};
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::BinanceRestClient;

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

    /// Get exchange info from metadata manager
    pub async fn get_exchange_info(&self) -> Result<ExchangeInfo, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.metadata_query_tx
            .send(MetadataQuery::GetExchangeInfo { reply: reply_tx })
            .await
            .map_err(|e| format!("Failed to query exchange info: {}", e))?;
        reply_rx
            .await
            .map_err(|e| format!("Failed to receive exchange info: {}", e))?
            .ok_or_else(|| "Exchange info not yet available".to_string())
    }

    /// Get ticker data from metadata manager
    pub async fn get_ticker_data(&self) -> Result<HashMap<String, TickerData>, String> {
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
    GetExchangeInfo {
        reply: tokio::sync::oneshot::Sender<Option<ExchangeInfo>>,
    },
    GetTickerData {
        reply: tokio::sync::oneshot::Sender<HashMap<String, TickerData>>,
    },
}

/// Manages exchange metadata with periodic background updates
pub struct ExchangeMetadataManager {
    exchange_info: Option<ExchangeInfo>,
    ticker_data: HashMap<String, TickerData>,
    update_interval_secs: u64,
    query_rx: tokio::sync::mpsc::Receiver<MetadataQuery>,
    client: BinanceRestClient,
}

impl ExchangeMetadataManager {
    /// Spawn a new metadata manager and return the handle and JoinHandle.
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
        let client = BinanceRestClient::new(requests_per_second);

        Self {
            exchange_info: None,
            ticker_data: HashMap::new(),
            update_interval_secs,
            query_rx,
            client,
        }
    }

    /// Run the metadata manager loop that periodically updates exchange metadata
    /// and handles queries. Caller should spawn this with tokio::spawn() to run in background.
    pub async fn run(mut self) {
        // Initial fetch
        if let Err(e) = self.update_exchange_info().await {
            log::error!("Initial Binance exchange info fetch failed: {}", e);
        }
        if let Err(e) = self.update_ticker_data().await {
            log::error!("Initial Binance ticker data fetch failed: {}", e);
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
                    if let Err(e) = self.update_exchange_info().await {
                        log::error!("Failed to update Binance exchange info: {}", e);
                    }
                    if let Err(e) = self.update_ticker_data().await {
                        log::error!("Failed to update Binance ticker data: {}", e);
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
            MetadataQuery::GetExchangeInfo { reply } => {
                let _ = reply.send(self.exchange_info.clone());
            }
            MetadataQuery::GetTickerData { reply } => {
                let _ = reply.send(self.ticker_data.clone());
            }
        }
    }

    async fn update_exchange_info(&mut self) -> Result<(), String> {
        let exchange_info = self
            .client
            .get_exchange_info()
            .await
            .map_err(|e| format!("Failed to fetch Binance exchange info: {}", e))?;

        let count = exchange_info.symbols.len();
        self.exchange_info = Some(exchange_info);
        log::debug!("Updated Binance exchange info: {} symbols", count);

        Ok(())
    }

    async fn update_ticker_data(&mut self) -> Result<(), String> {
        let ticker_list = self
            .client
            .get_24hr_ticker(None)
            .await
            .map_err(|e| format!("Failed to fetch Binance ticker data: {}", e))?;

        // Convert list to HashMap keyed by symbol
        let ticker_data: HashMap<String, TickerData> = ticker_list
            .into_iter()
            .map(|t| (t.symbol.clone(), t))
            .collect();

        let count = ticker_data.len();
        self.ticker_data = ticker_data;
        log::debug!("Updated Binance ticker data: {} symbols", count);

        Ok(())
    }
}
