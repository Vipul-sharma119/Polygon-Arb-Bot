use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use rust_decimal::Decimal;
use ethers::{
    providers::{Provider, Http, Middleware},
    types::{Address, U256},
    contract::Contract,
};
use std::str::FromStr;
use std::sync::Arc;
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub polygon_rpc_url: String,
    
    // Token addresses
    pub weth_address: String,
    pub usdc_address: String,
    
    // DEX Router addresses
    pub uniswap_v3_quoter_address: String,
    pub sushiswap_router_address: String,
    
    // Trading parameters
    pub min_profit_threshold: Decimal,
    pub trade_amount: Decimal,
    pub estimated_gas_cost: Decimal,
    pub check_interval_seconds: u64,
    
    // Slippage and safety
    pub max_slippage_bps: u16, // basis points (100 = 1%)
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            polygon_rpc_url: std::env::var("POLYGON_RPC_URL")
                .context("POLYGON_RPC_URL must be set")?,
            
            // Polygon mainnet addresses
            weth_address: std::env::var("WETH_ADDRESS")
                .unwrap_or_else(|_| "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
            usdc_address: std::env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
            
            // DEX contract addresses on Polygon
            uniswap_v3_quoter_address: std::env::var("UNISWAP_V3_QUOTER")
                .unwrap_or_else(|_| "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string()),
            sushiswap_router_address: std::env::var("SUSHISWAP_ROUTER")
                .unwrap_or_else(|_| "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string()),
            
            min_profit_threshold: std::env::var("MIN_PROFIT_THRESHOLD")
                .unwrap_or_else(|_| "0.005".to_string())
                .parse()
                .context("Invalid MIN_PROFIT_THRESHOLD")?,
            trade_amount: std::env::var("TRADE_AMOUNT")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .context("Invalid TRADE_AMOUNT")?,
            estimated_gas_cost: std::env::var("ESTIMATED_GAS_COST")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("Invalid ESTIMATED_GAS_COST")?,
            check_interval_seconds: std::env::var("CHECK_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("Invalid CHECK_INTERVAL_SECONDS")?,
            max_slippage_bps: std::env::var("MAX_SLIPPAGE_BPS")
                .unwrap_or_else(|_| "100".to_string()) // 1%
                .parse()
                .context("Invalid MAX_SLIPPAGE_BPS")?,
        })
    }
}
