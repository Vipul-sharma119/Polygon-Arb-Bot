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


pub struct UniswapV3Client {
    provider: Arc<Provider<Http>>,
    quoter_contract: Contract<Provider<Http>>,
    weth_address: Address,
    usdc_address: Address,
}

impl UniswapV3Client {
    pub async fn new(
        rpc_url: &str,
        quoter_address: &str,
        weth_address: &str,
        usdc_address: &str,
    ) -> Result<Self> {
        let provider = Arc::new(
            Provider::<Http>::try_from(rpc_url)
                .context("Failed to create HTTP provider")?
        );
        
        let quoter_addr = Address::from_str(quoter_address)
            .context("Invalid quoter address")?;
        
        let quoter_contract = Contract::from_json(
            provider.clone(),
            quoter_addr,
            UNISWAP_V3_QUOTER_ABI.as_bytes(),
        ).context("Failed to create quoter contract")?;
        
        Ok(Self {
            provider,
            quoter_contract,
            weth_address: Address::from_str(weth_address)?,
            usdc_address: Address::from_str(usdc_address)?,
        })
    }
    
    async fn get_token_decimals(&self, token_address: Address) -> Result<u8> {
        let token_contract = Contract::from_json(
            self.provider.clone(),
            token_address,
            ERC20_ABI.as_bytes(),
        )?;
        
        let decimals: u8 = token_contract
            .method::<_, u8>("decimals", ())?
            .call()
            .await
            .context("Failed to get token decimals")?;
        
        Ok(decimals)
    }
}

#[async_trait]
impl DexClient for UniswapV3Client {
    async fn get_price(&self, pair: &TokenPair) -> Result<Decimal> {
        // Convert trade amount to token units (assuming USDC input)
        let usdc_decimals = self.get_token_decimals(self.usdc_address).await?;
        let weth_decimals = self.get_token_decimals(self.weth_address).await?;
        
        let amount_in = U256::from(1000) * U256::exp10(usdc_decimals as usize); // 1000 USDC
        
        // Uniswap V3 fee tiers: 500 (0.05%), 3000 (0.3%), 10000 (1%)
        // Try the most common 0.3% fee tier first
        let fee_tier = 3000u32;
        
        let quote_result: U256 = self.quoter_contract
            .method::<_, U256>(
                "quoteExactInputSingle",
                (
                    self.usdc_address,
                    self.weth_address,
                    fee_tier,
                    amount_in,
                    U256::zero(), // No price limit
                ),
            )?
            .call()
            .await
            .context("Failed to get Uniswap quote")?;
        
        // Convert back to human readable price
        let weth_out = quote_result.as_u128() as f64 / 10_f64.powi(weth_decimals as i32);
        let usdc_in = 1000.0; // We quoted for 1000 USDC
        
        let price = Decimal::try_from(usdc_in / weth_out)
            .context("Failed to convert price to Decimal")?;
        
        log::debug!("Uniswap V3 price for {}: {} USDC per WETH", pair.symbol, price);
        Ok(price)
    }
}
