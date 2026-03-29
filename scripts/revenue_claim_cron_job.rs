#!/usr/bin/env rust-script

use std::env;
use std::time::Duration;
use soroban_sdk::{Address, Env, xdr::ScVal};
use soroban_sdk::token::Client as TokenClient;
use soroban_spec::read::{Wasm};
use stellar_rpc_client::{Client, ContractId};
use tokio::time::interval;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    contract_address: String,
    creator_address: String,
    threshold: i128,
    check_interval_seconds: u64,
    network_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            contract_address: "CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L".to_string(), // Testnet contract
            creator_address: "".to_string(), // Must be provided
            threshold: 100_000_000, // 100 USDC (6 decimals)
            check_interval_seconds: 300, // 5 minutes
            network_url: "https://soroban-testnet.stellar.org".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SubscriptionInfo {
    subscriber: Address,
    creator: Address,
    token: Address,
    balance: i128,
    last_collected: u64,
    amount_to_collect: i128,
}

struct RevenueClaimer {
    config: Config,
    client: Client,
    contract_id: ContractId,
}

impl RevenueClaimer {
    fn new(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new(&config.network_url)?;
        let contract_id = ContractId::from_str(&config.contract_address)?;
        
        Ok(Self {
            config,
            client,
            contract_id,
        })
    }

    async fn get_pending_revenue(&self, creator_address: &str) -> Result<Vec<SubscriptionInfo>, Box<dyn std::error::Error>> {
        let mut pending_subscriptions = Vec::new();
        
        // Get all subscribers for the creator (this would need to be implemented based on contract storage)
        // For now, we'll simulate this by checking known subscribers or using contract events
        
        // In a real implementation, you would:
        // 1. Query contract storage for all subscriptions where the creator is a recipient
        // 2. For each subscription, calculate the amount that can be collected
        // 3. Return only those above the threshold
        
        // This is a simplified version - in production you'd need proper indexing
        let creator = Address::from_str(creator_address)?;
        
        // Example: Check a few known subscribers (in practice, you'd have a more comprehensive method)
        let known_subscribers = vec![
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWH2",
        ];
        
        for subscriber_str in known_subscribers {
            let subscriber = Address::from_str(subscriber_str)?;
            
            if let Ok(subscription_info) = self.check_subscription_revenue(&subscriber, &creator).await {
                if subscription_info.amount_to_collect >= self.config.threshold {
                    pending_subscriptions.push(subscription_info);
                }
            }
        }
        
        Ok(pending_subscriptions)
    }

    async fn check_subscription_revenue(&self, subscriber: &Address, creator: &Address) -> Result<SubscriptionInfo, Box<dyn std::error::Error>> {
        // In a real implementation, you would call the contract to get subscription details
        // and calculate the collectable amount using the same logic as the contract
        
        // This is a mock implementation - replace with actual contract calls
        Ok(SubscriptionInfo {
            subscriber: subscriber.clone(),
            creator: creator.clone(),
            token: Address::from_str("GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5")?, // USDC testnet
            balance: 150_000_000, // 150 USDC
            last_collected: 1640995200, // Mock timestamp
            amount_to_collect: 120_000_000, // 120 USDC available to collect
        })
    }

    async fn claim_revenue(&self, subscription: &SubscriptionInfo) -> Result<(), Box<dyn std::error::Error>> {
        println!("Claiming revenue for subscriber: {:?}, amount: {} USDC", 
                subscription.subscriber, 
                subscription.amount_to_collect / 1_000_000);

        // In a real implementation, you would:
        // 1. Sign and submit a transaction calling the contract's collect function
        // 2. Wait for confirmation
        // 3. Handle any errors
        
        // Mock transaction submission
        println!("Submitting collect transaction for subscriber: {:?}", subscription.subscriber);
        
        // Example of how you'd call the contract:
        // let tx = self.client.prepare_transaction(
        //     &self.contract_id,
        //     "collect",
        //     vec![&subscription.subscriber, &subscription.creator],
        // )?;
        // let result = self.client.send_transaction(tx).await?;
        
        Ok(())
    }

    async fn run_cron_job(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting revenue claim cron job for creator: {}", self.config.creator_address);
        println!("Threshold: {} USDC", self.config.threshold / 1_000_000);
        println!("Check interval: {} seconds", self.config.check_interval_seconds);

        let mut interval = interval(Duration::from_secs(self.config.check_interval_seconds));

        loop {
            interval.tick().await;
            
            println!("Checking for pending revenue...");
            
            match self.get_pending_revenue(&self.config.creator_address).await {
                Ok(pending_subscriptions) => {
                    if pending_subscriptions.is_empty() {
                        println!("No pending revenue above threshold");
                    } else {
                        println!("Found {} subscriptions with revenue above threshold", pending_subscriptions.len());
                        
                        for subscription in pending_subscriptions {
                            match self.claim_revenue(&subscription).await {
                                Ok(_) => {
                                    println!("Successfully claimed revenue from subscriber: {:?}", subscription.subscriber);
                                }
                                Err(e) => {
                                    eprintln!("Failed to claim revenue from subscriber {:?}: {}", subscription.subscriber, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error checking pending revenue: {}", e);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables or use defaults
    let config = Config {
        creator_address: env::var("CREATOR_ADDRESS")
            .unwrap_or_else(|_| panic!("CREATOR_ADDRESS environment variable must be set")),
        threshold: env::var("REVENUE_THRESHOLD")
            .unwrap_or_else(|_| "100000000".to_string())
            .parse()
            .unwrap_or(100_000_000),
        check_interval_seconds: env::var("CHECK_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .unwrap_or(300),
        network_url: env::var("NETWORK_URL")
            .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
        ..Default::default()
    };

    println!("Revenue Claim Cron Job Starting...");
    println!("Creator: {}", config.creator_address);
    println!("Threshold: {} USDC", config.threshold / 1_000_000);
    println!("Network: {}", config.network_url);

    let claimer = RevenueClaimer::new(config)?;
    
    // Set up graceful shutdown
    tokio::signal::ctrl_c().await?;
    println!("Received shutdown signal, stopping cron job...");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.threshold, 100_000_000);
        assert_eq!(config.check_interval_seconds, 300);
    }

    #[test]
    fn test_config_from_env() {
        env::set_var("CREATOR_ADDRESS", "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");
        env::set_var("REVENUE_THRESHOLD", "200000000");
        env::set_var("CHECK_INTERVAL_SECONDS", "600");
        
        let config = Config {
            creator_address: env::var("CREATOR_ADDRESS").unwrap(),
            threshold: env::var("REVENUE_THRESHOLD").unwrap().parse().unwrap(),
            check_interval_seconds: env::var("CHECK_INTERVAL_SECONDS").unwrap().parse().unwrap(),
            ..Default::default()
        };
        
        assert_eq!(config.threshold, 200_000_000);
        assert_eq!(config.check_interval_seconds, 600);
        
        env::remove_var("CREATOR_ADDRESS");
        env::remove_var("REVENUE_THRESHOLD");
        env::remove_var("CHECK_INTERVAL_SECONDS");
    }
}
