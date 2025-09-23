use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::time::Duration;
use tokio::time::sleep;

mod config;
mod database;
mod dex;
mod models;
mod price_validator; // Add the new module

use config::Config;
use database::Database;
use dex::{uniswap::UniswapV3Client, sushiswap::SushiswapClient, DexClient};
use models::{ArbitrageOpportunity, TokenPair};
use price_validator::{PriceValidator, ValidationResult}; 

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let config = Config::from_env()?;
    let db = Database::new(&config.database_url).await?;
    
    
    db.init().await?;

    let mut bot = ArbitrageBot::new(config, db).await?;
    bot.run().await
}

pub struct ArbitrageBot {
    config: Config,
    db: Database,
    uniswap_client: UniswapV3Client,
    sushiswap_client: SushiswapClient,
    price_validator: PriceValidator, // Use the separate module
}

impl ArbitrageBot {
    pub async fn new(config: Config, db: Database) -> Result<Self> {
        let uniswap_client = UniswapV3Client::new(
            &config.polygon_rpc_url,
            &config.uniswap_v3_quoter_address,
            &config.weth_address,
            &config.usdc_address,
        ).await.context("Failed to create Uniswap client")?;
        
        let sushiswap_client = SushiswapClient::new(
            &config.polygon_rpc_url,
            &config.sushiswap_router_address,
            &config.weth_address,
            &config.usdc_address,
        ).await.context("Failed to create SushiSwap client")?;
        
        // Create price validator with custom bounds based on config
        let price_validator = PriceValidator::with_bounds(
            Decimal::from(500),   // Min price
            Decimal::from(10000), // Max price
            Decimal::try_from(0.15).unwrap(), // 15% max change
            5, // 5 minutes max age
        );

        Ok(Self {
            config,
            db,
            uniswap_client,
            sushiswap_client,
            price_validator,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        log::info!("Starting Production Polygon Arbitrage Bot");

        let token_pair = TokenPair {
            token0: self.config.weth_address.clone(),
            token1: self.config.usdc_address.clone(),
            symbol: "WETH/USDC".to_string(),
        };

        loop {
            if let Err(e) = self.check_arbitrage_opportunity(&token_pair).await {
                log::error!("Error checking arbitrage opportunity: {}", e);
                
                // Print validation stats on errors
                let stats = self.price_validator.get_stats();
                log::info!("Validation stats: {:?}", stats);
                
                // Exponential backoff on errors
                sleep(Duration::from_secs(60)).await;
            } else {
                sleep(Duration::from_secs(self.config.check_interval_seconds)).await;
            }
        }
    }

    async fn check_arbitrage_opportunity(&mut self, pair: &TokenPair) -> Result<()> {
        log::debug!("Checking arbitrage opportunity for {}", pair.symbol);

        // Get prices from both DEXes with timeout
        let timeout_duration = Duration::from_secs(30);

        let (uniswap_result, sushiswap_result) = tokio::join!(
            tokio::time::timeout(timeout_duration, self.uniswap_client.get_price(pair)),
            tokio::time::timeout(timeout_duration, self.sushiswap_client.get_price(pair))
        );
 
        // Handle Uniswap price
        let uniswap_price = match uniswap_result {
            Ok(Ok(price)) => price,
            Ok(Err(e)) => {
                log::error!("Failed to get Uniswap price: {}", e);
                return Ok(());
            },
            Err(_) => {
                log::error!("Uniswap price fetch timeout");
                return Ok(());
            }
        };

        // Handle SushiSwap price
        let sushiswap_price = match sushiswap_result {
            Ok(Ok(price)) => price,
            Ok(Err(e)) => {
                log::error!("Failed to get SushiSwap price: {}", e);
                return Ok(());
            },
            Err(_) => {
                log::error!("SushiSwap price fetch timeout");
                return Ok(());
            }
        };

        // Validate prices using the separate validator
        let uniswap_validation = self.price_validator.validate_price("Uniswap", uniswap_price)?;
        let sushiswap_validation = self.price_validator.validate_price("SushiSwap", sushiswap_price)?;

        // Check if both prices are valid
        if !uniswap_validation.is_valid() {
            log::warn!("Invalid Uniswap price: {}", 
                uniswap_validation.error_message().unwrap_or("Unknown error"));
            return Ok(());
        }

        if !sushiswap_validation.is_valid() {
            log::warn!("Invalid SushiSwap price: {}", 
                sushiswap_validation.error_message().unwrap_or("Unknown error"));
            return Ok(());
        }

        log::info!(
            "Valid prices - Uniswap: {} USDC, SushiSwap: {} USDC",
            uniswap_price,
            sushiswap_price
        );

        // Calculate price difference and potential profit
        let price_diff = if uniswap_price > sushiswap_price {
            (uniswap_price - sushiswap_price) / sushiswap_price
        } else {
            (sushiswap_price - uniswap_price) / uniswap_price
        };

        log::debug!("Price difference: {:.4}%", price_diff * Decimal::from(100));

        // Check if price difference exceeds minimum threshold
        if price_diff >= self.config.min_profit_threshold {
            let opportunity = self.calculate_arbitrage_profit(
                pair,
                uniswap_price,
                sushiswap_price,
                price_diff,
            ).await?;

            // Additional profitability check after gas costs
            if opportunity.estimated_profit > Decimal::ZERO {
                log::info!(
                    "🚀 Profitable arbitrage opportunity found! Profit: {} USDC ({:.2}%)",
                    opportunity.estimated_profit,
                    (price_diff * Decimal::from(100))
                );

                // Save to database
                self.db.save_opportunity(&opportunity).await
                    .context("Failed to save opportunity to database")?;

                // Here you would implement the actual trading logic
                // self.execute_arbitrage(&opportunity).await?;
            } else {
                log::debug!("Opportunity found but not profitable after gas costs");
            }
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
            ("SushiSwap", "Uniswap", sushiswap_price, uniswap_price)
        } else {
            ("Uniswap", "SushiSwap", uniswap_price, sushiswap_price)
        };

        // Calculate tokens received when buying (accounting for slippage)
        let slippage_factor = Decimal::from(1) - 
            Decimal::from(self.config.max_slippage_bps) / Decimal::from(10000);

        let tokens_bought = (trade_amount / buy_price) * slippage_factor;

        // Calculate USDC received when selling (accounting for slippage)
        let usdc_received = (tokens_bought * sell_price) * slippage_factor;

        // Estimate gas costs based on current network conditions
        let estimated_gas_cost = self.estimate_gas_cost().await?;

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

    async fn estimate_gas_cost(&self) -> Result<Decimal> {
        // This is a simplified gas estimation
        // In production, you'd want to:
        // 1. Get current gas price from the network
        // 2. Estimate gas usage for your specific transactions
        // 3. Convert to USDC equivalent

        // For now, use the configured estimate
        Ok(self.config.estimated_gas_cost)
    }
}