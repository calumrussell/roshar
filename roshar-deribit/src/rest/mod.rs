pub mod market;
pub use market::{MarketApi, DERIBIT_REST_URL, DERIBIT_TESTNET_REST_URL};

#[cfg(test)]
mod tests;
