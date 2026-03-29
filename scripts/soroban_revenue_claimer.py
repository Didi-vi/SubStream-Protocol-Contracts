#!/usr/bin/env python3
"""
Soroban Revenue Claimer for SubStream Protocol

This script uses the Soroban RPC to interact with the SubStream smart contract
and automatically claims revenue for creators when it exceeds the configured threshold.

Requirements:
- soroban-rpc
- stellar-sdk

Installation:
pip install stellar-sdk soroban-rpc requests python-dotenv

Usage:
export CREATOR_ADDRESS="your_creator_address"
export PRIVATE_KEY="your_private_key"
python soroban_revenue_claimer.py
"""

import os
import sys
import time
import json
import logging
import asyncio
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass
from dotenv import load_dotenv

# Load environment variables
load_dotenv()

import requests
from stellar_sdk import Keypair, TransactionBuilder, Network
from stellar_sdk import xdr as stellar_xdr
from stellar_sdk.soroban_rpc import SorobanRPC
from stellar_sdk.soroban import SorobanServer
from stellar_sdk.exceptions import PrepareTransactionException, RpcError

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

@dataclass
class Config:
    """Configuration for the revenue claimer"""
    creator_address: str
    private_key: str
    threshold: int = 100_000_000  # 100 USDC (6 decimals)
    check_interval: int = 300  # 5 minutes
    network_url: str = "https://soroban-testnet.stellar.org"
    horizon_url: str = "https://horizon-testnet.stellar.org"
    contract_address: str = "CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L"
    usdc_address: str = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
    network_passphrase: str = Network.TESTNET_NETWORK_PASSPHRASE

class SorobanRevenueClaimer:
    """Revenue claimer using Soroban RPC"""
    
    def __init__(self, config: Config):
        self.config = config
        self.keypair = Keypair.from_secret(config.private_key)
        self.rpc = SorobanRPC(config.network_url)
        self.server = SorobanServer(config.network_url)
        self.horizon_server = StellarHorizon(config.horizon_url)

    async def get_account_sequence(self) -> int:
        """Get the current account sequence number"""
        try:
            account = await self.horizon_server.load_account(self.keypair.public_key)
            return int(account['sequence'])
        except Exception as e:
            logger.error(f"Error loading account: {e}")
            raise

    async def simulate_collect(self, subscriber: str, creator: str) -> Optional[int]:
        """
        Simulate a collect transaction to see how much revenue would be claimed.
        
        Returns the amount that would be claimed, or None if simulation fails.
        """
        try:
            # Get current account info
            account = await self.horizon_server.load_account(self.keypair.public_key)
            account_obj = stellar_xdr.LedgerKeyAccount(
                account_id=stellar_xdr.AccountId(
                    stellar_xdr.PublicKey(
                        stellar_xdr.PublicKeyType.PUBLIC_KEY_TYPE_ED25519,
                        bytes.fromhex(self.keypair.public_key)[32:]
                    )
                )
            )
            
            # Build transaction
            transaction = TransactionBuilder(
                source_account=account,
                network_passphrase=self.config.network_passphrase,
                base_fee=100
            ).set_timeout(30).build()
            
            # Add contract call operation
            contract_call = stellar_xdr.Operation(
                source_account=None,
                body=stellar_xdr.OperationBody(
                    invoke_host_function=stellar_xdr.InvokeHostFunctionOp(
                        host_function=stellar_xdr.HostFunction(
                            type=stellar_xdr.HostFunctionType.HOST_FUNCTION_TYPE_INVOKE_CONTRACT,
                            invoke_contract=stellar_xdr.InvokeContractArgs(
                                contract_address=stellar_xdr.ScAddress(
                                    type=stellar_xdr.ScAddressType.SCONTRACT,
                                    contract_id=bytes.fromhex(self.config.contract_address)[32:]
                                ),
                                function_name=stellar_xdr.Scsymbol(b"collect"),
                                args=[
                                    stellar_xdr.ScVal(
                                        type=stellar_xdr.ScValType.SCV_ADDRESS,
                                        address=stellar_xdr.ScAddress(
                                            type=stellar_xdr.ScAddressType.SCPUBLIC_KEY,
                                            public_key=stellar_xdr.PublicKey(
                                                stellar_xdr.PublicKeyType.PUBLIC_KEY_TYPE_ED25519,
                                                bytes.fromhex(subscriber)[32:]
                                            )
                                        )
                                    ),
                                    stellar_xdr.ScVal(
                                        type=stellar_xdr.ScValType.SCV_ADDRESS,
                                        address=stellar_xdr.ScAddress(
                                            type=stellar_xdr.ScAddressType.SCPUBLIC_KEY,
                                            public_key=stellar_xdr.PublicKey(
                                                stellar_xdr.PublicKeyType.PUBLIC_KEY_TYPE_ED25519,
                                                bytes.fromhex(creator)[32:]
                                            )
                                        )
                                    )
                                ]
                            )
                        ),
                        auth=[]
                    )
                )
            )
            
            transaction.operations.append(contract_call)
            
            # Simulate transaction
            simulation = await self.rpc.simulate_transaction(transaction)
            
            if simulation.error:
                logger.warning(f"Simulation error: {simulation.error}")
                return None
            
            # Parse simulation results to get the amount that would be transferred
            if simulation.results and len(simulation.results) > 0:
                result = simulation.results[0]
                # The result should contain information about token transfers
                # This is simplified - you'd need to parse the actual XDR result
                return self.parse_simulation_result(result)
            
            return None
            
        except Exception as e:
            logger.error(f"Error simulating collect: {e}")
            return None

    def parse_simulation_result(self, result) -> Optional[int]:
        """
        Parse simulation result to extract the amount that would be claimed.
        
        This is a simplified implementation. In practice, you'd need to:
        1. Parse the XDR result properly
        2. Look for token transfer events
        3. Extract the transfer amounts
        """
        # For now, return a mock value
        # In a real implementation, you'd parse the XDR result
        return 120_000_000  # Mock: 120 USDC

    async def get_creator_subscriptions(self) -> List[Tuple[str, str]]:
        """
        Get all subscriptions where the creator is a recipient.
        
        Returns a list of (subscriber, creator) tuples.
        """
        try:
            # This is a simplified implementation
            # In practice, you'd need a way to query contract storage or events
            
            # For now, we'll use a mock list of subscribers
            # In a real implementation, you would:
            # 1. Query contract events for subscriptions
            # 2. Use contract storage to enumerate subscriptions
            # 3. Maintain an external index of subscriptions
            
            mock_subscriptions = [
                ("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF", self.config.creator_address),
                ("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWH2", self.config.creator_address),
            ]
            
            return mock_subscriptions
            
        except Exception as e:
            logger.error(f"Error getting creator subscriptions: {e}")
            return []

    async def check_pending_revenue(self) -> List[Dict]:
        """Check all subscriptions for revenue above threshold"""
        pending = []
        
        try:
            subscriptions = await self.get_creator_subscriptions()
            
            for subscriber, creator in subscriptions:
                try:
                    amount = await self.simulate_collect(subscriber, creator)
                    
                    if amount and amount >= self.config.threshold:
                        pending.append({
                            'subscriber': subscriber,
                            'creator': creator,
                            'amount': amount,
                            'amount_usdc': amount / 1_000_000
                        })
                        
                except Exception as e:
                    logger.warning(f"Error checking subscription {subscriber}: {e}")
            
            return pending
            
        except Exception as e:
            logger.error(f"Error checking pending revenue: {e}")
            return []

    async def claim_revenue(self, subscription: Dict) -> bool:
        """Claim revenue from a specific subscription"""
        try:
            logger.info(f"Claiming {subscription['amount_usdc']} USDC from subscriber: {subscription['subscriber']}")
            
            # Get current account info
            account = await self.horizon_server.load_account(self.keypair.public_key)
            
            # Build transaction
            transaction = TransactionBuilder(
                source_account=account,
                network_passphrase=self.config.network_passphrase,
                base_fee=100
            ).set_timeout(30).build()
            
            # Add contract call operation
            contract_call = stellar_xdr.Operation(
                source_account=None,
                body=stellar_xdr.OperationBody(
                    invoke_host_function=stellar_xdr.InvokeHostFunctionOp(
                        host_function=stellar_xdr.HostFunction(
                            type=stellar_xdr.HostFunctionType.HOST_FUNCTION_TYPE_INVOKE_CONTRACT,
                            invoke_contract=stellar_xdr.InvokeContractArgs(
                                contract_address=stellar_xdr.ScAddress(
                                    type=stellar_xdr.ScAddressType.SCONTRACT,
                                    contract_id=bytes.fromhex(self.config.contract_address)[32:]
                                ),
                                function_name=stellar_xdr.Scsymbol(b"collect"),
                                args=[
                                    stellar_xdr.ScVal(
                                        type=stellar_xdr.ScValType.SCV_ADDRESS,
                                        address=stellar_xdr.ScAddress(
                                            type=stellar_xdr.ScAddressType.SCPUBLIC_KEY,
                                            public_key=stellar_xdr.PublicKey(
                                                stellar_xdr.PublicKeyType.PUBLIC_KEY_TYPE_ED25519,
                                                bytes.fromhex(subscription['subscriber'])[32:]
                                            )
                                        )
                                    ),
                                    stellar_xdr.ScVal(
                                        type=stellar_xdr.ScValType.SCV_ADDRESS,
                                        address=stellar_xdr.ScAddress(
                                            type=stellar_xdr.ScAddressType.SCPUBLIC_KEY,
                                            public_key=stellar_xdr.PublicKey(
                                                stellar_xdr.PublicKeyType.PUBLIC_KEY_TYPE_ED25519,
                                                bytes.fromhex(subscription['creator'])[32:]
                                            )
                                        )
                                    )
                                ]
                            )
                        ),
                        auth=[]
                    )
                )
            )
            
            transaction.operations.append(contract_call)
            
            # Prepare transaction
            prepare_response = await self.rpc.prepare_transaction(transaction)
            
            if prepare_response.error:
                logger.error(f"Transaction preparation failed: {prepare_response.error}")
                return False
            
            # Sign transaction
            transaction.sign(self.keypair)
            
            # Send transaction
            send_response = await self.rpc.send_transaction(transaction)
            
            if send_response.error:
                logger.error(f"Transaction send failed: {send_response.error}")
                return False
            
            # Wait for transaction confirmation
            tx_hash = send_response.hash
            logger.info(f"Transaction sent: {tx_hash}")
            
            # Poll for transaction status
            for _ in range(30):  # Wait up to 30 seconds
                await asyncio.sleep(1)
                
                try:
                    status = await self.rpc.get_transaction(tx_hash)
                    
                    if status.status == "SUCCESS":
                        logger.info(f"Transaction confirmed: {tx_hash}")
                        return True
                    elif status.status == "FAILED":
                        logger.error(f"Transaction failed: {tx_hash}")
                        return False
                        
                except RpcError as e:
                    if "not found" not in str(e):
                        logger.warning(f"Error checking transaction status: {e}")
            
            logger.error(f"Transaction timeout: {tx_hash}")
            return False
            
        except Exception as e:
            logger.error(f"Error claiming revenue: {e}")
            return False

    async def run(self):
        """Main run loop"""
        logger.info("Starting Soroban Revenue Claimer")
        logger.info(f"Creator: {self.config.creator_address}")
        logger.info(f"Threshold: {self.config.threshold / 1_000_000} USDC")
        logger.info(f"Check interval: {self.config.check_interval} seconds")
        
        while True:
            try:
                logger.info("Checking for pending revenue...")
                
                pending = await self.check_pending_revenue()
                
                if not pending:
                    logger.info("No pending revenue above threshold")
                else:
                    logger.info(f"Found {len(pending)} subscriptions with revenue above threshold")
                    
                    for subscription in pending:
                        success = await self.claim_revenue(subscription)
                        
                        if success:
                            logger.info(f"Successfully claimed {subscription['amount_usdc']} USDC")
                        else:
                            logger.error(f"Failed to claim from {subscription['subscriber']}")
                
            except KeyboardInterrupt:
                logger.info("Received interrupt signal, stopping...")
                break
            except Exception as e:
                logger.error(f"Error in main loop: {e}")
            
            # Wait for next check
            await asyncio.sleep(self.config.check_interval)

class StellarHorizon:
    """Simple wrapper for Horizon API"""
    
    def __init__(self, horizon_url: str):
        self.horizon_url = horizon_url
    
    async def load_account(self, address: str) -> Dict:
        """Load account information from Horizon"""
        url = f"{self.horizon_url}/accounts/{address}"
        
        async with requests.AsyncClient() as client:
            response = await client.get(url)
            response.raise_for_status()
            return response.json()

def load_config() -> Config:
    """Load configuration from environment variables"""
    creator_address = os.getenv('CREATOR_ADDRESS')
    if not creator_address:
        raise ValueError("CREATOR_ADDRESS environment variable must be set")
    
    private_key = os.getenv('PRIVATE_KEY')
    if not private_key:
        raise ValueError("PRIVATE_KEY environment variable must be set")
    
    threshold = int(os.getenv('REVENUE_THRESHOLD', '100000000'))
    check_interval = int(os.getenv('CHECK_INTERVAL', '300'))
    network_url = os.getenv('NETWORK_URL', 'https://soroban-testnet.stellar.org')
    horizon_url = os.getenv('HORIZON_URL', 'https://horizon-testnet.stellar.org')
    contract_address = os.getenv('CONTRACT_ADDRESS', 'CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L')
    usdc_address = os.getenv('USDC_ADDRESS', 'GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5')
    
    return Config(
        creator_address=creator_address,
        private_key=private_key,
        threshold=threshold,
        check_interval=check_interval,
        network_url=network_url,
        horizon_url=horizon_url,
        contract_address=contract_address,
        usdc_address=usdc_address
    )

async def main():
    """Main entry point"""
    try:
        config = load_config()
        claimer = SorobanRevenueClaimer(config)
        await claimer.run()
        
    except KeyboardInterrupt:
        logger.info("Received interrupt signal, exiting...")
        sys.exit(0)
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        sys.exit(1)

if __name__ == '__main__':
    asyncio.run(main())
