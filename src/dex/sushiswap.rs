use anyhow::{Context, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::TokenPair;
use super::DexClient;

pub struct SushiswapClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl SushiswapClient {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl DexClient for SushiswapClient {
    async fn get_price(&self, pair: &TokenPair) -> Result<Decimal> {
        // This is a simplified implementation
        // In production, you'd call SushiSwap's router contract
        // or use their API/subgraph
        
        // For now, simulate with mock data + some randomness
        let base_price = Decimal::from(2000); // ~2000 USDC per WETH
        let variation = fastrand::f64() * 0.02 - 0.01; // ±1% variation
        let price = base_price * (Decimal::from(1) + Decimal::try_from(variation).unwrap_or_default());
        
        log::debug!("SushiSwap price for {}: {}", pair.symbol, price);
        Ok(price)
    }
}