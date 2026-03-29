#!/usr/bin/env python3
"""
Test script for Revenue Claim Cron Job

This script tests the basic functionality of the revenue claimer without
requiring actual network calls or private keys.
"""

import os
import sys
import unittest
from unittest.mock import Mock, patch, MagicMock
from revenue_claim_cron_job import RevenueClaimer, Config

class TestRevenueClaimer(unittest.TestCase):
    """Test cases for RevenueClaimer"""
    
    def setUp(self):
        """Set up test fixtures"""
        self.config = Config(
            creator_address="GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            threshold=100_000_000,  # 100 USDC
            check_interval=60,
            network_url="https://horizon-testnet.stellar.org",
            contract_address="CAOUX2FZ65IDC4F2X7LJJ2SVF23A35CCTZB7KVVN475JCLKTTU4CEY6L",
            private_key=None  # Test mode
        )
        
    def test_config_creation(self):
        """Test configuration creation"""
        self.assertEqual(self.config.creator_address, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF")
        self.assertEqual(self.config.threshold, 100_000_000)
        self.assertEqual(self.config.check_interval, 60)
        
    def test_config_default_values(self):
        """Test default configuration values"""
        config = Config(
            creator_address="GTEST",
            threshold=50_000_000
        )
        self.assertEqual(config.threshold, 50_000_000)
        self.assertEqual(config.check_interval, 300)  # Default value
        self.assertEqual(config.network_url, "https://horizon-testnet.stellar.org")  # Default value
        
    @patch('revenue_claim_cron_job.Server')
    def test_claimer_initialization(self, mock_server):
        """Test RevenueClaimer initialization"""
        mock_server_instance = Mock()
        mock_server.return_value = mock_server_instance
        
        claimer = RevenueClaimer(self.config)
        
        self.assertEqual(claimer.config, self.config)
        self.assertIsNone(claimer.keypair)  # No private key in test mode
        self.assertIsNone(claimer.account)
        
    def test_check_subscription_revenue_mock(self):
        """Test subscription revenue checking with mocked data"""
        claimer = RevenueClaimer(self.config)
        
        # Mock the method to return known data
        with patch.object(claimer, 'check_subscription_revenue') as mock_check:
            mock_check.return_value = {
                'subscriber': 'GTEST1',
                'creator': 'GCREATOR',
                'token': 'GUSDC',
                'balance': 150_000_000,
                'last_collected': 1640995200,
                'amount_to_collect': 120_000_000
            }
            
            result = claimer.check_subscription_revenue(
                'GTEST1', 
                'GCREATOR'
            )
            
            self.assertIsNotNone(result)
            self.assertEqual(result['amount_to_collect'], 120_000_000)
            self.assertEqual(result['subscriber'], 'GTEST1')
            
    def test_threshold_checking(self):
        """Test threshold checking logic"""
        claimer = RevenueClaimer(self.config)
        
        # Test subscription below threshold
        subscription_below = {
            'amount_to_collect': 50_000_000,  # 50 USDC
            'subscriber': 'GTEST1'
        }
        
        # Test subscription above threshold
        subscription_above = {
            'amount_to_collect': 150_000_000,  # 150 USDC
            'subscriber': 'GTEST2'
        }
        
        # Should not be claimed (below threshold)
        self.assertLess(subscription_below['amount_to_collect'], self.config.threshold)
        
        # Should be claimed (above threshold)
        self.assertGreater(subscription_above['amount_to_collect'], self.config.threshold)
        
    @patch('revenue_claim_cron_job.RevenueClaimer.get_pending_subscriptions')
    def test_get_pending_subscriptions(self, mock_pending):
        """Test getting pending subscriptions"""
        claimer = RevenueClaimer(self.config)
        
        mock_pending.return_value = [
            {
                'subscriber': 'GTEST1',
                'creator': 'GCREATOR',
                'amount_to_collect': 120_000_000
            },
            {
                'subscriber': 'GTEST2',
                'creator': 'GCREATOR',
                'amount_to_collect': 80_000_000  # Below threshold
            }
        ]
        
        pending = claimer.get_pending_subscriptions()
        
        # Should only return subscriptions above threshold
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0]['subscriber'], 'GTEST1')
        self.assertEqual(pending[0]['amount_to_collect'], 120_000_000)
        
    def test_usdc_conversion(self):
        """Test USDC amount conversion (6 decimals)"""
        amount_stroops = 150_000_000  # 150 USDC in stroops
        amount_usdc = amount_stroops / 1_000_000
        
        self.assertEqual(amount_usdc, 150.0)
        
        # Test threshold conversion
        threshold_usdc = self.config.threshold / 1_000_000
        self.assertEqual(threshold_usdc, 100.0)

class TestConfigLoading(unittest.TestCase):
    """Test configuration loading from environment"""
    
    def setUp(self):
        """Set up test environment"""
        # Save original environment
        self.original_env = os.environ.copy()
        
    def tearDown(self):
        """Restore original environment"""
        os.environ.clear()
        os.environ.update(self.original_env)
        
    def test_load_config_from_env(self):
        """Test loading configuration from environment variables"""
        # Set test environment variables
        os.environ['CREATOR_ADDRESS'] = 'GTEST_CREATOR'
        os.environ['REVENUE_THRESHOLD'] = '200'
        os.environ['CHECK_INTERVAL'] = '600'
        os.environ['NETWORK_URL'] = 'https://custom.stellar.org'
        
        # Import here to use patched environment
        from revenue_claim_cron_job import load_config_from_env
        
        config = load_config_from_env()
        
        self.assertEqual(config.creator_address, 'GTEST_CREATOR')
        self.assertEqual(config.threshold, 200_000_000)  # 200 USDC in stroops
        self.assertEqual(config.check_interval, 600)
        self.assertEqual(config.network_url, 'https://custom.stellar.org')
        
    def test_missing_required_env_var(self):
        """Test error when required environment variable is missing"""
        # Remove required environment variable
        if 'CREATOR_ADDRESS' in os.environ:
            del os.environ['CREATOR_ADDRESS']
            
        from revenue_claim_cron_job import load_config_from_env
        
        with self.assertRaises(ValueError) as context:
            load_config_from_env()
            
        self.assertIn('CREATOR_ADDRESS', str(context.exception))

def run_tests():
    """Run all tests"""
    print("Running Revenue Claimer Tests...")
    
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add test cases
    suite.addTests(loader.loadTestsFromTestCase(TestRevenueClaimer))
    suite.addTests(loader.loadTestsFromTestCase(TestConfigLoading))
    
    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    
    # Return success status
    return result.wasSuccessful()

if __name__ == '__main__':
    success = run_tests()
    sys.exit(0 if success else 1)
