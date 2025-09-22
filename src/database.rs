use anyhow::Result;
use sqlx::{PgPool, Row};

use crate::models::ArbitrageOpportunity;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS arbitrage_opportunities (
                id UUID PRIMARY KEY,
                timestamp TIMESTAMPTZ NOT NULL,
                token_pair VARCHAR NOT NULL,
                buy_dex VARCHAR NOT NULL,
                sell_dex VARCHAR NOT NULL,
                buy_price DECIMAL NOT NULL,
                sell_price DECIMAL NOT NULL,
                price_difference_pct DECIMAL NOT NULL,
                trade_amount DECIMAL NOT NULL,
                estimated_profit DECIMAL NOT NULL,
                gas_cost DECIMAL NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_arbitrage_timestamp 
            ON arbitrage_opportunities (timestamp);

            CREATE INDEX IF NOT EXISTS idx_arbitrage_token_pair 
            ON arbitrage_opportunities (token_pair);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_opportunities (
                id, timestamp, token_pair, buy_dex, sell_dex,
                buy_price, sell_price, price_difference_pct,
                trade_amount, estimated_profit, gas_cost
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(&opportunity.id)
        .bind(&opportunity.timestamp)
        .bind(&opportunity.token_pair)
        .bind(&opportunity.buy_dex)
        .bind(&opportunity.sell_dex)
        .bind(&opportunity.buy_price)
        .bind(&opportunity.sell_price)
        .bind(&opportunity.price_difference_pct)
        .bind(&opportunity.trade_amount)
        .bind(&opportunity.estimated_profit)
        .bind(&opportunity.gas_cost)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_opportunities(&self, limit: i32) -> Result<Vec<ArbitrageOpportunity>> {
        let opportunities = sqlx::query_as::<_, ArbitrageOpportunity>(
            "SELECT * FROM arbitrage_opportunities ORDER BY timestamp DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(opportunities)
    }
}