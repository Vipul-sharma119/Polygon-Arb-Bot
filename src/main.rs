use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

mod config;
mod database;
mod dex;
mod models;

use config::Config;
use database::Database;
use dex::{uniswap::UniswapV3Client, sushiswap::SushiswapClient, DexClient};
use models::{ArbitrageOpportunity, TokenPair};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let config = Config::from_env()?;
    let db = Database::new(&config.database_url).await?;
    
    // Initialize database tables
    db.init().await?;

    let bot = ArbitrageBot::new(config, db).await?;
    bot.run().await
}

pub struct ArbitrageBot {
    config: Config,
    db: Database,
    uniswap_client: UniswapV3Client,
    sushiswap_client: SushiswapClient,
}

impl ArbitrageBot {
    pub async fn new(config: Config, db: Database) -> Result<Self> {
        let uniswap_client = UniswapV3Client::new(&config.polygon_rpc_url);
        let sushiswap_client = SushiswapClient::new(&config.polygon_rpc_url);

        Ok(Self {
            config,
            db,
            uniswap_client,
            sushiswap_client,
        })
    }

    pub async fn run(&self) -> Result<()> {
        log::info!("Starting Polygon Arbitrage Bot");

        let token_pair = TokenPair {
            token0: self.config.weth_address.clone(),
            token1: self.config.usdc_address.clone(),
            symbol: "WETH/USDC".to_string(),
        };

        loop {
            if let Err(e) = self.check_arbitrage_opportunity(&token_pair).await {
                log::error!("Error checking arbitrage opportunity: {}", e);
            }

            sleep(Duration::from_secs(self.config.check_interval_seconds)).await;
        }
    }

    async fn check_arbitrage_opportunity(&self, pair: &TokenPair) -> Result<()> {
        // Get prices from both DEXes
        let uniswap_price = self.uniswap_client.get_price(pair).await?;
        let sushiswap_price = self.sushiswap_client.get_price(pair).await?;

        log::debug!(
            "Prices - Uniswap: {}, Sushiswap: {}",
            uniswap_price,
            sushiswap_price
        );

        // Calculate price difference and potential profit
        let price_diff = if uniswap_price > sushiswap_price {
            (uniswap_price - sushiswap_price) / sushiswap_price
        } else {
            (sushiswap_price - uniswap_price) / uniswap_price
        };

        // Check if price difference exceeds minimum threshold
        if price_diff >= self.config.min_profit_threshold {
            let opportunity = self.calculate_arbitrage_profit(
                pair,
                uniswap_price,
                sushiswap_price,
                price_diff,
            ).await?;

            log::info!(
                "Arbitrage opportunity found! Profit: {} USDC ({}%)",
                opportunity.estimated_profit,
                (price_diff * Decimal::from(100))
            );

            // Save to database
            self.db.save_opportunity(&opportunity).await?;
        }

        Ok(())
    }

    async fn calculate_arbitrage_profit(
        &self,
        pair: &TokenPair,
        uniswap_price: Decimal,
        sushiswap_price: Decimal,
        price_diff_pct: Decimal,
    ) -> Result<ArbitrageOpportunity> {
        let trade_amount = self.config.trade_amount;
        
        let (buy_dex, sell_dex, buy_price, sell_price) = if uniswap_price > sushiswap_price {
            ("Sushiswap", "Uniswap", sushiswap_price, uniswap_price)
        } else {
            ("Uniswap", "Sushiswap", uniswap_price, sushiswap_price)
        };

        // Calculate tokens received when buying
        let tokens_bought = trade_amount / buy_price;
        
        // Calculate USDC received when selling
        let usdc_received = tokens_bought * sell_price;
        
        // Estimate gas costs (simplified)
        let estimated_gas_cost = self.config.estimated_gas_cost;
        
        // Calculate net profit
        let gross_profit = usdc_received - trade_amount;
        let net_profit = gross_profit - estimated_gas_cost;

        Ok(ArbitrageOpportunity {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            token_pair: pair.symbol.clone(),
            buy_dex: buy_dex.to_string(),
            sell_dex: sell_dex.to_string(),
            buy_price,
            sell_price,
            price_difference_pct: price_diff_pct,
            trade_amount,
            estimated_profit: net_profit,
            gas_cost: estimated_gas_cost,
        })
    }
}
