#!/usr/bin/env python3
"""
Revenue Claim Cron Job for SubStream Protocol

This script automatically claims earned streaming funds for creators once they reach a certain threshold.
It monitors the SubStream contract and calls the collect function when revenue exceeds the configured threshold.

Usage:
    python revenue_claim_cron_job.py --creator <CREATOR_ADDRESS> --threshold <USDC_AMOUNT>

Environment Variables:
    CREATOR_ADDRESS: The creator's Stellar address
    REVENUE_THRESHOLD: Revenue threshold in USDC (default: 100)
    CHECK_INTERVAL: Check interval in seconds (default: 300)
    NETWORK_URL: Stellar network URL (default: testnet)
    CONTRACT_ADDRESS: SubStream contract address
    PRIVATE_KEY: Creator's private key for signing transactions
"""

import os
import sys
import time
import json
import logging
import argparse
from typing import List, Dict, Optional
from dataclasses import dataclass
from stellar_sdk import Server, Keypair, TransactionBuilder, Network, Account
from stellar_sdk.contract import ContractClient
from stellar_sdk.exceptions import NotFoundError, HorizonError

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

@dataclass
class Config:
    """Configuration for the revenue claim cron job"""
    creator_address: str
    threshold: int = 100  # USDC
    check_interval: int = 300  # 5 minutes
    network_url: str = "https://horizon-testnet.stellar.org"
    contract_address: str = "CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L"
    private_key: Optional[str] = None
    usdc_address: str = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"  # USDC testnet

class RevenueClaimer:
    """Handles revenue claiming for SubStream creators"""
    
    def __init__(self, config: Config):
        self.config = config
        self.server = Server(config.network_url)
        self.network_passphrase = Network.TESTNET_NETWORK_PASSPHRASE
        
        if config.private_key:
            self.keypair = Keypair.from_secret(config.private_key)
            self.account = Account(self.keypair.public_key, 1)  # Will be updated with actual sequence
        else:
            self.keypair = None
            self.account = None
        
        self.contract_client = ContractClient(
            contract_id=config.contract_address,
            client=self.server
        )

    def get_account_sequence(self) -> int:
        """Get the current account sequence number"""
        if not self.keypair:
            raise ValueError("No keypair configured")
        
        account = self.server.load_account(self.keypair.public_key)
        return account.sequence

    def get_pending_subscriptions(self) -> List[Dict]:
        """
        Get all subscriptions where the creator is a recipient and has pending revenue.
        
        This is a simplified implementation. In production, you would need:
        1. A way to enumerate all subscriptions for a creator
        2. Efficient querying of subscription states
        3. Proper handling of pagination
        """
        try:
            # For now, we'll use events to find recent subscriptions
            # In a real implementation, you'd have indexed storage
            pending_subscriptions = []
            
            # Get recent collect events to see which subscriptions have activity
            events = self.server.events(
                for_contract=self.config.contract_address,
                limit=100
            )
            
            # Extract subscriber-creator pairs from events
            subscriber_creator_pairs = set()
            for event in events._embedded.records:
                if event.type == "contract":
                    topic = event.topic
                    # Parse event topics to identify subscriptions
                    # This is simplified - you'd need proper event parsing
                    if len(topic) >= 2:
                        subscriber = str(topic[0])
                        creator = str(topic[1])
                        if creator == self.config.creator_address:
                            subscriber_creator_pairs.add((subscriber, creator))
            
            # Check each subscription for collectable revenue
            for subscriber, creator in subscriber_creator_pairs:
                try:
                    subscription_info = self.check_subscription_revenue(subscriber, creator)
                    if subscription_info and subscription_info['amount_to_collect'] >= self.config.threshold * 1_000_000:
                        pending_subscriptions.append(subscription_info)
                except Exception as e:
                    logger.warning(f"Error checking subscription {subscriber}: {e}")
            
            return pending_subscriptions
            
        except Exception as e:
            logger.error(f"Error getting pending subscriptions: {e}")
            return []

    def check_subscription_revenue(self, subscriber: str, creator: str) -> Optional[Dict]:
        """
        Check a specific subscription for collectable revenue.
        
        Returns subscription info if there's collectable revenue, None otherwise.
        """
        try:
            # In a real implementation, you would call the contract to get subscription details
            # For now, we'll simulate this with a mock implementation
            
            # Mock data - replace with actual contract calls
            # You would need to implement a way to query subscription state from the contract
            mock_subscription = {
                'subscriber': subscriber,
                'creator': creator,
                'token': self.config.usdc_address,
                'balance': 150_000_000,  # 150 USDC
                'last_collected': int(time.time()) - 86400,  # 1 day ago
                'amount_to_collect': 120_000_000,  # 120 USDC available to collect
            }
            
            return mock_subscription
            
        except Exception as e:
            logger.error(f"Error checking subscription revenue: {e}")
            return None

    def claim_revenue(self, subscription: Dict) -> bool:
        """
        Claim revenue from a specific subscription by calling the contract's collect function.
        
        Returns True if successful, False otherwise.
        """
        if not self.keypair:
            logger.error("No private key configured for transaction signing")
            return False
        
        try:
            logger.info(f"Claiming revenue for subscriber: {subscription['subscriber']}")
            logger.info(f"Amount: {subscription['amount_to_collect'] / 1_000_000} USDC")
            
            # Get current account sequence
            account = self.server.load_account(self.keypair.public_key)
            
            # Build transaction to call collect function
            transaction = (
                TransactionBuilder(
                    source_account=account,
                    network_passphrase=self.network_passphrase,
                    base_fee=100
                )
                .append_contract_call_op(
                    contract_id=self.config.contract_address,
                    function_name="collect",
                    parameters=[
                        subscription['subscriber'],
                        subscription['creator']
                    ]
                )
                .set_timeout(30)
                .build()
            )
            
            # Sign transaction
            transaction.sign(self.keypair)
            
            # Submit transaction
            response = self.server.submit_transaction(transaction)
            
            if response['successful']:
                logger.info(f"Successfully claimed revenue from subscriber: {subscription['subscriber']}")
                logger.info(f"Transaction hash: {response['hash']}")
                return True
            else:
                logger.error(f"Transaction failed: {response['result_xdr']}")
                return False
                
        except Exception as e:
            logger.error(f"Error claiming revenue: {e}")
            return False

    def run_cron_job(self):
        """Main cron job loop"""
        logger.info("Starting revenue claim cron job")
        logger.info(f"Creator: {self.config.creator_address}")
        logger.info(f"Threshold: {self.config.threshold} USDC")
        logger.info(f"Check interval: {self.config.check_interval} seconds")
        logger.info(f"Network: {self.config.network_url}")
        
        if not self.keypair:
            logger.warning("No private key configured - will only monitor, not claim")
        
        while True:
            try:
                logger.info("Checking for pending revenue...")
                
                pending_subscriptions = self.get_pending_subscriptions()
                
                if not pending_subscriptions:
                    logger.info("No pending revenue above threshold")
                else:
                    logger.info(f"Found {len(pending_subscriptions)} subscriptions with revenue above threshold")
                    
                    for subscription in pending_subscriptions:
                        if self.keypair:
                            success = self.claim_revenue(subscription)
                            if success:
                                logger.info(f"Successfully claimed from {subscription['subscriber']}")
                            else:
                                logger.error(f"Failed to claim from {subscription['subscriber']}")
                        else:
                            logger.info(f"Would claim {subscription['amount_to_collect'] / 1_000_000} USDC from {subscription['subscriber']} (monitoring mode)")
                
            except KeyboardInterrupt:
                logger.info("Received interrupt signal, stopping...")
                break
            except Exception as e:
                logger.error(f"Error in cron job loop: {e}")
            
            # Wait for next check
            time.sleep(self.config.check_interval)

def load_config_from_env() -> Config:
    """Load configuration from environment variables"""
    creator_address = os.getenv('CREATOR_ADDRESS')
    if not creator_address:
        raise ValueError("CREATOR_ADDRESS environment variable must be set")
    
    threshold = int(os.getenv('REVENUE_THRESHOLD', '100'))
    check_interval = int(os.getenv('CHECK_INTERVAL', '300'))
    network_url = os.getenv('NETWORK_URL', 'https://horizon-testnet.stellar.org')
    contract_address = os.getenv('CONTRACT_ADDRESS', 'CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L')
    private_key = os.getenv('PRIVATE_KEY')
    
    return Config(
        creator_address=creator_address,
        threshold=threshold,
        check_interval=check_interval,
        network_url=network_url,
        contract_address=contract_address,
        private_key=private_key
    )

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description='Revenue Claim Cron Job for SubStream Protocol')
    parser.add_argument('--creator', help='Creator address (overrides CREATOR_ADDRESS env var)')
    parser.add_argument('--threshold', type=int, help='Revenue threshold in USDC (overrides REVENUE_THRESHOLD env var)')
    parser.add_argument('--interval', type=int, help='Check interval in seconds (overrides CHECK_INTERVAL env var)')
    parser.add_argument('--network', help='Network URL (overrides NETWORK_URL env var)')
    parser.add_argument('--private-key', help='Private key for signing transactions (overrides PRIVATE_KEY env var)')
    parser.add_argument('--monitor-only', action='store_true', help='Only monitor, dont actually claim revenue')
    
    args = parser.parse_args()
    
    try:
        # Load configuration
        config = load_config_from_env()
        
        # Override with command line arguments
        if args.creator:
            config.creator_address = args.creator
        if args.threshold:
            config.threshold = args.threshold
        if args.interval:
            config.check_interval = args.interval
        if args.network:
            config.network_url = args.network
        if args.private_key:
            config.private_key = args.private_key
        if args.monitor_only:
            config.private_key = None
        
        # Create and run the claimer
        claimer = RevenueClaimer(config)
        claimer.run_cron_job()
        
    except KeyboardInterrupt:
        logger.info("Received interrupt signal, exiting...")
        sys.exit(0)
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        sys.exit(1)

if __name__ == '__main__':
    main()
