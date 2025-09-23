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


pub struct SushiswapClient {
    provider: Arc<Provider<Http>>,
    router_contract: Contract<Provider<Http>>,
    weth_address: Address,
    usdc_address: Address,
}

impl SushiswapClient {
    pub async fn new(
        rpc_url: &str,
        router_address: &str,
        weth_address: &str,
        usdc_address: &str,
    ) -> Result<Self> {
        let provider = Arc::new(
            Provider::<Http>::try_from(rpc_url)
                .context("Failed to create HTTP provider")?
        );
        
        let router_addr = Address::from_str(router_address)
            .context("Invalid router address")?;
        
        let router_contract = Contract::from_json(
            provider.clone(),
            router_addr,
            SUSHISWAP_ROUTER_ABI.as_bytes(),
        ).context("Failed to create router contract")?;
        
        Ok(Self {
            provider,
            router_contract,
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
impl DexClient for SushiswapClient {
    async fn get_price(&self, pair: &TokenPair) -> Result<Decimal> {
        let usdc_decimals = self.get_token_decimals(self.usdc_address).await?;
        let weth_decimals = self.get_token_decimals(self.weth_address).await?;
        
        let amount_in = U256::from(1000) * U256::exp10(usdc_decimals as usize); // 1000 USDC
        
        // Create the path: USDC -> WETH
        let path = vec![self.usdc_address, self.weth_address];
        
        let amounts_out: Vec<U256> = self.router_contract
            .method::<_, Vec<U256>>("getAmountsOut", (amount_in, path))?
            .call()
            .await
            .context("Failed to get SushiSwap quote")?;
        
        if amounts_out.len() != 2 {
            return Err(anyhow!("Unexpected getAmountsOut response length"));
        }
        
        // Convert back to human readable price
        let weth_out = amounts_out[1].as_u128() as f64 / 10_f64.powi(weth_decimals as i32);
        let usdc_in = 1000.0; // We quoted for 1000 USDC
        
        let price = Decimal::try_from(usdc_in / weth_out)
            .context("Failed to convert price to Decimal")?;
        
        log::debug!("SushiSwap price for {}: {} USDC per WETH", pair.symbol, price);
        Ok(price)
    }
}