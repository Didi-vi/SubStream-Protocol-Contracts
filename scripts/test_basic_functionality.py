#!/usr/bin/env python3
"""
Basic functionality test for Revenue Claim Cron Job

This script tests the core logic without requiring external dependencies.
"""

import os
import sys
import json
from dataclasses import dataclass
from typing import List, Dict, Optional

@dataclass
class Config:
    """Configuration for the revenue claimer"""
    creator_address: str
    threshold: int = 100_000_000  # 100 USDC (6 decimals)
    check_interval: int = 300  # 5 minutes
    network_url: str = "https://horizon-testnet.stellar.org"
    contract_address: str = "CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L"
    private_key: Optional[str] = None
    usdc_address: str = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"

class MockRevenueClaimer:
    """Mock implementation for testing"""
    
    def __init__(self, config: Config):
        self.config = config
        
    def check_subscription_revenue(self, subscriber: str, creator: str) -> Optional[Dict]:
        """Mock subscription revenue checking"""
        # Simulate different scenarios
        mock_data = {
            "GTEST1": 120_000_000,  # 120 USDC - above threshold
            "GTEST2": 80_000_000,   # 80 USDC - below threshold
            "GTEST3": 200_000_000,  # 200 USDC - above threshold
        }
        
        amount = mock_data.get(subscriber, 0)
        
        if amount > 0:
            return {
                'subscriber': subscriber,
                'creator': creator,
                'token': self.config.usdc_address,
                'balance': amount + 30_000_000,
                'last_collected': 1640995200,
                'amount_to_collect': amount
            }
        return None
        
    def get_pending_subscriptions(self) -> List[Dict]:
        """Get subscriptions with revenue above threshold"""
        pending = []
        
        # Mock subscribers
        subscribers = ["GTEST1", "GTEST2", "GTEST3", "GTEST4"]
        
        for subscriber in subscribers:
            subscription = self.check_subscription_revenue(subscriber, self.config.creator_address)
            if subscription and subscription['amount_to_collect'] >= self.config.threshold:
                pending.append(subscription)
                
        return pending
        
    def claim_revenue(self, subscription: Dict) -> bool:
        """Mock revenue claiming"""
        print(f"Mock claiming {subscription['amount_to_collect'] / 1_000_000} USDC from {subscription['subscriber']}")
        return True

def test_config():
    """Test configuration creation"""
    print("Testing configuration...")
    
    config = Config(
        creator_address="GTEST",
        threshold=50_000_000
    )
    
    assert config.creator_address == "GTEST"
    assert config.threshold == 50_000_000
    assert config.check_interval == 300  # Default value
    print("✓ Configuration test passed")

def test_threshold_logic():
    """Test threshold checking logic"""
    print("Testing threshold logic...")
    
    config = Config(creator_address="GTEST", threshold=100_000_000)
    claimer = MockRevenueClaimer(config)
    
    # Test different amounts
    test_cases = [
        ("GTEST1", 120_000_000, True),   # Above threshold
        ("GTEST2", 80_000_000, False),   # Below threshold
        ("GTEST3", 200_000_000, True),   # Above threshold
    ]
    
    for subscriber, amount, should_claim in test_cases:
        subscription = claimer.check_subscription_revenue(subscriber, "GTEST")
        if should_claim:
            assert subscription is not None
            assert subscription['amount_to_collect'] >= config.threshold
        else:
            assert subscription is None or subscription['amount_to_collect'] < config.threshold
            
    print("✓ Threshold logic test passed")

def test_pending_subscriptions():
    """Test getting pending subscriptions"""
    print("Testing pending subscriptions...")
    
    config = Config(creator_address="GTEST", threshold=100_000_000)
    claimer = MockRevenueClaimer(config)
    
    pending = claimer.get_pending_subscriptions()
    
    # Should have 2 subscriptions above threshold (GTEST1: 120, GTEST3: 200)
    assert len(pending) == 2
    
    # Verify amounts are above threshold
    for subscription in pending:
        assert subscription['amount_to_collect'] >= config.threshold
        
    print(f"✓ Found {len(pending)} pending subscriptions above threshold")
    print("✓ Pending subscriptions test passed")

def test_claiming_process():
    """Test the claiming process"""
    print("Testing claiming process...")
    
    config = Config(creator_address="GTEST", threshold=100_000_000)
    claimer = MockRevenueClaimer(config)
    
    pending = claimer.get_pending_subscriptions()
    
    # Claim each pending subscription
    for subscription in pending:
        success = claimer.claim_revenue(subscription)
        assert success == True
        
    print("✓ Claiming process test passed")

def test_usdc_conversion():
    """Test USDC amount conversion"""
    print("Testing USDC conversion...")
    
    # Test various conversions
    test_cases = [
        (100_000_000, 100.0),    # 100 USDC
        (150_000_000, 150.0),    # 150 USDC
        (1_000_000, 1.0),        # 1 USDC
        (500_000, 0.5),          # 0.5 USDC
    ]
    
    for stroops, expected_usdc in test_cases:
        actual_usdc = stroops / 1_000_000
        assert actual_usdc == expected_usdc
        
    print("✓ USDC conversion test passed")

def test_environment_config():
    """Test loading configuration from environment"""
    print("Testing environment configuration...")
    
    # Set test environment variables
    os.environ['CREATOR_ADDRESS'] = 'GTEST_ENV_CREATOR'
    os.environ['REVENUE_THRESHOLD'] = '200'
    os.environ['CHECK_INTERVAL'] = '600'
    
    # Mock environment loading
    config = Config(
        creator_address=os.getenv('CREATOR_ADDRESS', ''),
        threshold=int(os.getenv('REVENUE_THRESHOLD', '100')) * 1_000_000,
        check_interval=int(os.getenv('CHECK_INTERVAL', '300'))
    )
    
    assert config.creator_address == 'GTEST_ENV_CREATOR'
    assert config.threshold == 200_000_000  # 200 USDC in stroops
    assert config.check_interval == 600
    
    # Clean up
    del os.environ['CREATOR_ADDRESS']
    del os.environ['REVENUE_THRESHOLD']
    del os.environ['CHECK_INTERVAL']
    
    print("✓ Environment configuration test passed")

def run_all_tests():
    """Run all tests"""
    print("=" * 50)
    print("Running Revenue Claimer Basic Tests")
    print("=" * 50)
    
    tests = [
        test_config,
        test_threshold_logic,
        test_pending_subscriptions,
        test_claiming_process,
        test_usdc_conversion,
        test_environment_config,
    ]
    
    passed = 0
    failed = 0
    
    for test in tests:
        try:
            test()
            passed += 1
        except Exception as e:
            print(f"✗ {test.__name__} failed: {e}")
            failed += 1
    
    print("=" * 50)
    print(f"Test Results: {passed} passed, {failed} failed")
    print("=" * 50)
    
    return failed == 0

if __name__ == '__main__':
    success = run_all_tests()
    sys.exit(0 if success else 1)
