use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

/// Price validation and sanity checking for arbitrage opportunities
pub struct PriceValidator {
    /// Minimum reasonable price for WETH in USDC
    min_price: Decimal,
    
    /// Maximum reasonable price for WETH in USDC
    max_price: Decimal,
    
    /// Maximum allowed price change percentage between checks
    max_price_change_pct: Decimal,
    
    /// Store of last valid prices for each DEX
    last_prices: HashMap<String, PriceSnapshot>,
    
    /// Maximum age of price data before considering it stale
    max_price_age: Duration,
}

#[derive(Debug, Clone)]
struct PriceSnapshot {
    price: Decimal,
    timestamp: DateTime<Utc>,
    consecutive_errors: u32,
}

impl PriceValidator {
    /// Create a new price validator with sensible defaults
    pub fn new() -> Self {
        Self {
            // Reasonable bounds for WETH/USDC (adjust based on market conditions)
            min_price: Decimal::from(500),   // Min 500 USDC per WETH
            max_price: Decimal::from(10000), // Max 10000 USDC per WETH
            max_price_change_pct: Decimal::try_from(0.15).unwrap(), // 15% max change
            last_prices: HashMap::new(),
            max_price_age: Duration::minutes(5), // 5 minutes max age
        }
    }
    
    /// Create a validator with custom parameters
    pub fn with_bounds(
        min_price: Decimal,
        max_price: Decimal,
        max_price_change_pct: Decimal,
        max_price_age_minutes: i64,
    ) -> Self {
        Self {
            min_price,
            max_price,
            max_price_change_pct,
            last_prices: HashMap::new(),
            max_price_age: Duration::minutes(max_price_age_minutes),
        }
    }
    
    /// Validate a price from a specific DEX
    pub fn validate_price(&mut self, dex_name: &str, price: Decimal) -> Result<ValidationResult> {
        let now = Utc::now();
        
        // Check 1: Absolute bounds
        if !self.check_absolute_bounds(price) {
            self.record_error(dex_name);
            return Ok(ValidationResult::Invalid(format!(
                "Price {} outside reasonable bounds ({}-{})",
                price, self.min_price, self.max_price
            )));
        }
        
        // Check 2: Relative change (if we have historical data)
        if let Some(validation_error) = self.check_price_change(dex_name, price) {
            self.record_error(dex_name);
            return Ok(ValidationResult::Invalid(validation_error));
        }
        
        // Check 3: Price staleness
        if let Some(last_snapshot) = self.last_prices.get(dex_name) {
            if now.signed_duration_since(last_snapshot.timestamp) > self.max_price_age {
                log::warn!("Stale price data for {} (age: {:?})", 
                    dex_name, 
                    now.signed_duration_since(last_snapshot.timestamp)
                );
            }
        }
        
        // Check 4: Circuit breaker for consecutive errors
        if let Some(snapshot) = self.last_prices.get(dex_name) {
            if snapshot.consecutive_errors > 5 {
                return Ok(ValidationResult::CircuitBreakerTripped(format!(
                    "Too many consecutive errors for {} ({})",
                    dex_name, snapshot.consecutive_errors
                )));
            }
        }
        
        // All checks passed - record the valid price
        self.record_valid_price(dex_name, price, now);
        
        Ok(ValidationResult::Valid)
    }
    
    /// Check if price is within absolute bounds
    fn check_absolute_bounds(&self, price: Decimal) -> bool {
        price >= self.min_price && price <= self.max_price
    }
    
    /// Check if price change is reasonable compared to last price
    fn check_price_change(&self, dex_name: &str, price: Decimal) -> Option<String> {
        if let Some(last_snapshot) = self.last_prices.get(dex_name) {
            let price_change = (price - last_snapshot.price).abs() / last_snapshot.price;
            
            if price_change > self.max_price_change_pct {
                return Some(format!(
                    "Large price change detected for {} ({:.2}%): {} -> {}",
                    dex_name,
                    price_change * Decimal::from(100),
                    last_snapshot.price,
                    price
                ));
            }
        }
        None
    }
    
    /// Record a valid price
    fn record_valid_price(&mut self, dex_name: &str, price: Decimal, timestamp: DateTime<Utc>) {
        self.last_prices.insert(dex_name.to_string(), PriceSnapshot {
            price,
            timestamp,
            consecutive_errors: 0,
        });
    }
    
    /// Record an error for a DEX
    fn record_error(&mut self, dex_name: &str) {
        if let Some(snapshot) = self.last_prices.get_mut(dex_name) {
            snapshot.consecutive_errors += 1;
        } else {
            // First time seeing this DEX and it's an error
            self.last_prices.insert(dex_name.to_string(), PriceSnapshot {
                price: Decimal::ZERO,
                timestamp: Utc::now(),
                consecutive_errors: 1,
            });
        }
    }
    
    /// Get the last valid price for a DEX
    pub fn get_last_price(&self, dex_name: &str) -> Option<(Decimal, DateTime<Utc>)> {
        self.last_prices
            .get(dex_name)
            .map(|snapshot| (snapshot.price, snapshot.timestamp))
    }
    
    /// Check if a DEX has too many consecutive errors
    pub fn is_circuit_breaker_tripped(&self, dex_name: &str) -> bool {
        self.last_prices
            .get(dex_name)
            .map(|snapshot| snapshot.consecutive_errors > 5)
            .unwrap_or(false)
    }
    
    /// Reset error count for a DEX (call this when connection is restored)
    pub fn reset_error_count(&mut self, dex_name: &str) {
        if let Some(snapshot) = self.last_prices.get_mut(dex_name) {
            snapshot.consecutive_errors = 0;
        }
    }
    
    /// Get validation statistics
    pub fn get_stats(&self) -> ValidationStats {
        let mut stats = ValidationStats {
            total_dexes: self.last_prices.len(),
            active_dexes: 0,
            circuit_breaker_tripped: 0,
            stale_prices: 0,
        };
        
        let now = Utc::now();
        
        for (_, snapshot) in &self.last_prices {
            if snapshot.consecutive_errors == 0 {
                stats.active_dexes += 1;
            }
            
            if snapshot.consecutive_errors > 5 {
                stats.circuit_breaker_tripped += 1;
            }
            
            if now.signed_duration_since(snapshot.timestamp) > self.max_price_age {
                stats.stale_prices += 1;
            }
        }
        
        stats
    }
}

/// Result of price validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid(String),
    CircuitBreakerTripped(String),
}

/// Statistics about validation state
#[derive(Debug, Clone)]
pub struct ValidationStats {
    pub total_dexes: usize,
    pub active_dexes: usize,
    pub circuit_breaker_tripped: usize,
    pub stale_prices: usize,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }
    
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationResult::Valid => None,
            ValidationResult::Invalid(msg) | ValidationResult::CircuitBreakerTripped(msg) => Some(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_price_bounds() {
        let mut validator = PriceValidator::new();
        
        // Valid price
        let result = validator.validate_price("test_dex", dec!(2000)).unwrap();
        assert!(result.is_valid());
        
        // Too low
        let result = validator.validate_price("test_dex", dec!(100)).unwrap();
        assert!(!result.is_valid());
        
        // Too high
        let result = validator.validate_price("test_dex", dec!(15000)).unwrap();
        assert!(!result.is_valid());
    }
    
    #[test]
    fn test_price_change_validation() {
        let mut validator = PriceValidator::new();
        
        // First price - should be valid
        let result = validator.validate_price("test_dex", dec!(2000)).unwrap();
        assert!(result.is_valid());
        
        // Small change - should be valid
        let result = validator.validate_price("test_dex", dec!(2100)).unwrap();
        assert!(result.is_valid());
        
        // Large change - should be invalid
        let result = validator.validate_price("test_dex", dec!(3500)).unwrap();
        assert!(!result.is_valid());
    }
    
    #[test]
    fn test_circuit_breaker() {
        let mut validator = PriceValidator::new();
        
        // Cause multiple errors
        for _ in 0..6 {
            let _ = validator.validate_price("test_dex", dec!(100)); // Invalid price
        }
        
        // Should trip circuit breaker
        assert!(validator.is_circuit_breaker_tripped("test_dex"));
        
        // Reset and try again
        validator.reset_error_count("test_dex");
        assert!(!validator.is_circuit_breaker_tripped("test_dex"));
    }
}