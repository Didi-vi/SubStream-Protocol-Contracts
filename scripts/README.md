# Revenue Claim Cron Job

This directory contains scripts for automatically claiming earned streaming funds for creators once they reach a certain threshold.

## Overview

The Revenue Claim Cron Job monitors the SubStream Protocol contract and automatically calls the `collect` function when a creator's accumulated revenue exceeds the configured threshold (e.g., > 100 USDC).

## Files

### 1. `revenue_claim_cron_job.py` (Recommended)
A Python script that uses the Stellar SDK to interact with the SubStream contract. This is the most user-friendly option.

### 2. `soroban_revenue_claimer.py`
An advanced Python script that uses Soroban RPC directly for more precise contract interaction. Suitable for production use.

### 3. `revenue_claim_cron_job.rs`
A Rust implementation (requires additional dependencies and setup).

## Quick Start

### 1. Install Dependencies

```bash
# For Python scripts
pip install -r requirements.txt

# Or install manually
pip install stellar-sdk soroban-rpc requests python-dotenv
```

### 2. Configure Environment

Copy the example environment file:
```bash
cp .env.example .env
```

Edit `.env` with your configuration:
```bash
# Required: Your creator address
CREATOR_ADDRESS=GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX

# Optional: Revenue threshold in USDC (default: 100)
REVENUE_THRESHOLD=100

# Optional: Check interval in seconds (default: 300 = 5 minutes)
CHECK_INTERVAL=300

# Optional: Your private key for signing transactions
PRIVATE_KEY=YOUR_PRIVATE_KEY_HERE
```

### 3. Run the Script

#### Option A: Basic Python Script
```bash
export CREATOR_ADDRESS="your_creator_address"
export PRIVATE_KEY="your_private_key"
python revenue_claim_cron_job.py
```

#### Option B: Advanced Soroban Script
```bash
export CREATOR_ADDRESS="your_creator_address"
export PRIVATE_KEY="your_private_key"
python soroban_revenue_claimer.py
```

#### Option C: Command Line Arguments
```bash
python revenue_claim_cron_job.py \
  --creator GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \
  --threshold 100 \
  --interval 300 \
  --private-key YOUR_PRIVATE_KEY
```

## Configuration Options

| Environment Variable | Description | Default |
|----------------------|-------------|---------|
| `CREATOR_ADDRESS` | Your Stellar creator address | Required |
| `PRIVATE_KEY` | Your private key for signing transactions | Required |
| `REVENUE_THRESHOLD` | Revenue threshold in USDC | 100 |
| `CHECK_INTERVAL` | Check interval in seconds | 300 (5 minutes) |
| `NETWORK_URL` | Stellar network URL | Testnet |
| `CONTRACT_ADDRESS` | SubStream contract address | Testnet contract |
| `USDC_ADDRESS` | USDC token address | Testnet USDC |

## Monitoring Mode

To run in monitoring mode (without actually claiming revenue):

```bash
python revenue_claim_cron_job.py --monitor-only
```

This will check for pending revenue and log what would be claimed without submitting transactions.

## Production Deployment

### Using systemd

Create a systemd service file `/etc/systemd/system/substream-revenue-claimer.service`:

```ini
[Unit]
Description=SubStream Revenue Claimer
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/path/to/SubStream-Protocol-Contracts/scripts
Environment=CREATOR_ADDRESS=your_creator_address
Environment=PRIVATE_KEY=your_private_key
Environment=REVENUE_THRESHOLD=100
Environment=CHECK_INTERVAL=300
ExecStart=/usr/bin/python3 soroban_revenue_claimer.py
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
sudo systemctl enable substream-revenue-claimer
sudo systemctl start substream-revenue-claimer
```

### Using Docker

Create a `Dockerfile`:

```dockerfile
FROM python:3.11-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install -r requirements.txt

COPY . .
CMD ["python", "soroban_revenue_claimer.py"]
```

Build and run:
```bash
docker build -t substream-revenue-claimer .
docker run -d \
  --name revenue-claimer \
  --restart unless-stopped \
  -e CREATOR_ADDRESS="your_creator_address" \
  -e PRIVATE_KEY="your_private_key" \
  -e REVENUE_THRESHOLD="100" \
  -e CHECK_INTERVAL="300" \
  substream-revenue-claimer
```

## Security Considerations

1. **Private Key Security**: Never commit private keys to version control. Use environment variables or secure key management systems.

2. **Hardware Wallets**: For production use, consider using hardware wallets or secure key management services.

3. **Network Security**: Ensure your server is properly secured with firewalls and regular updates.

4. **Monitoring**: Set up monitoring and alerts for the cron job to ensure it's running properly.

## Troubleshooting

### Common Issues

1. **"Account not found"**: Ensure the creator address is correct and the account exists on the network.

2. **"Insufficient fee"**: Increase the base fee in the transaction builder.

3. **"Transaction failed"**: Check the transaction result XDR for specific error details.

4. **"Connection timeout"**: Check network connectivity and RPC endpoint availability.

### Debug Mode

Enable debug logging:
```bash
export RUST_LOG=debug  # For Rust script
export PYTHONPATH=.    # For Python scripts
python -v revenue_claim_cron_job.py
```

## How It Works

1. **Monitoring**: The script periodically checks all subscriptions where the creator is a recipient.

2. **Simulation**: For each subscription, it simulates the `collect` function to determine how much revenue can be claimed.

3. **Threshold Check**: It compares the claimable amount against the configured threshold.

4. **Transaction Submission**: If above threshold, it submits a transaction calling the `collect` function.

5. **Confirmation**: It waits for transaction confirmation and logs the result.

## Contract Integration

The script integrates with the SubStream Protocol contract's `collect` function:

```rust
pub fn collect(env: Env, subscriber: Address, creator: Address) {
    distribute_and_collect(&env, &subscriber, &creator, Some(&creator));
}
```

This function:
- Calculates the amount to collect based on streaming duration and rates
- Handles discounted pricing for long-term subscribers
- Transfers the collected amount to the creator
- Updates the subscription state

## Contributing

When contributing to the revenue claimer:

1. Test thoroughly on testnet before mainnet deployment
2. Ensure proper error handling and logging
3. Follow security best practices for key management
4. Add comprehensive tests for new features

## License

This code is part of the SubStream Protocol project and follows the same license terms.
