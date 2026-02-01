//! Main CLI application for running data fetching jobs.

use clap::{Parser, Subcommand};

mod jobs;

/// Roshar data jobs CLI
#[derive(Parser)]
#[command(name = "roshar-jobs")]
#[command(about = "CLI for running data fetching and processing jobs")]
struct Cli {
    #[command(subcommand)]
    job: JobType,
}

/// Available job types
#[derive(Subcommand)]
enum JobType {
    /// Fetch historical trades from Deribit API
    DeribitTrades {
        /// Currency: BTC or ETH
        #[arg(long)]
        currency: String,

        /// Start date in YYYYMMDD or YYYY-MM-DD format
        #[arg(long)]
        start: String,

        /// End date in YYYYMMDD or YYYY-MM-DD format (defaults to today)
        #[arg(long)]
        end: Option<String>,
    },
}

/// Configuration for jobs
#[derive(Clone)]
pub struct Config {
    /// ClickHouse URL
    pub clickhouse_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            clickhouse_url: std::env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    let config = Config::default();

    match cli.job {
        JobType::DeribitTrades { currency, start, end } => {
            jobs::deribit_trades::run(config, currency, start, end).await?;
        }
    }

    Ok(())
}
