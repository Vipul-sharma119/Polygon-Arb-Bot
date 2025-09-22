use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub polygon_rpc_url: String,
    pub weth_address: String,
    pub usdc_address: String,
    pub min_profit_threshold: Decimal,
    pub trade_amount: Decimal,
    pub estimated_gas_cost: Decimal,
    pub check_interval_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            polygon_rpc_url: env::var("POLYGON_RPC_URL")
                .context("POLYGON_RPC_URL must be set")?,
            weth_address: env::var("WETH_ADDRESS")
                .unwrap_or_else(|_| "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
            usdc_address: env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
            min_profit_threshold: env::var("MIN_PROFIT_THRESHOLD")
                .unwrap_or_else(|_| "0.005".to_string()) // 0.5%
                .parse()
                .context("Invalid MIN_PROFIT_THRESHOLD")?,
            trade_amount: env::var("TRADE_AMOUNT")
                .unwrap_or_else(|_| "1000".to_string()) // 1000 USDC
                .parse()
                .context("Invalid TRADE_AMOUNT")?,
            estimated_gas_cost: env::var("ESTIMATED_GAS_COST")
                .unwrap_or_else(|_| "5".to_string()) // 5 USDC
                .parse()
                .context("Invalid ESTIMATED_GAS_COST")?,
            check_interval_seconds: env::var("CHECK_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("Invalid CHECK_INTERVAL_SECONDS")?,
        })
    }
}
