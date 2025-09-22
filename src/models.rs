use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub token0: String,
    pub token1: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArbitrageOpportunity {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub token_pair: String,
    pub buy_dex: String,
    pub sell_dex: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub price_difference_pct: Decimal,
    pub trade_amount: Decimal,
    pub estimated_profit: Decimal,
    pub gas_cost: Decimal,
}