use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

mod jobs;

#[derive(Parser)]
#[command(name = "one-time-jobs")]
#[command(about = "One-time data backfill jobs for Roshar", long_about = None)]
struct Cli {
    /// ClickHouse URL (e.g., http://localhost:8123)
    #[arg(long, env = "CLICKHOUSE_URL", default_value = "http://localhost:8123")]
    clickhouse_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Backfill Deribit DVOL (volatility index) data
    DeribitDvol {
        /// Currency to fetch (BTC or ETH)
        #[arg(short, long)]
        currency: String,

        /// Start date (YYYY-MM-DD)
        #[arg(short, long)]
        start_date: String,

        /// End date (YYYY-MM-DD)
        #[arg(short, long)]
        end_date: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::DeribitDvol {
            currency,
            start_date,
            end_date,
        } => {
            let currency: jobs::deribit_dvol::Currency = currency.parse()?;
            let start_date = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid start date format: {}. Use YYYY-MM-DD", e))?;
            let end_date = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid end date format: {}. Use YYYY-MM-DD", e))?;

            jobs::deribit_dvol::run(currency, start_date, end_date, &cli.clickhouse_url).await?;
        }
    }

    Ok(())
}
