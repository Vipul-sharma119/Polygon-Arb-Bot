use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::TokenPair;

pub mod uniswap;
pub mod sushiswap;

#[async_trait]
pub trait DexClient {
    async fn get_price(&self, pair: &TokenPair) -> Result<Decimal>;
}
