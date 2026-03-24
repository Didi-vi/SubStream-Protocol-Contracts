#![no_std]
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Stream(Address, Address), // (subscriber, creator)
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    pub token: Address,
    pub rate_per_second: i128,
    pub balance: i128,
    pub last_collected: u64,
}

#[contractevent]
pub struct TierChanged {
    #[topic]
    pub subscriber: Address,
    #[topic]
    pub creator: Address,
    pub old_rate: i128,
    pub new_rate: i128,
}

#[contract]
pub struct SubStreamContract;

#[contractimpl]
impl SubStreamContract {
    pub fn subscribe(
        env: Env,
        subscriber: Address,
        creator: Address,
        token: Address,
        amount: i128,
        rate_per_second: i128,
    ) {
        subscriber.require_auth();

        if amount <= 0 || rate_per_second <= 0 {
            panic!("amount and rate must be positive");
        }

        let key = DataKey::Stream(subscriber.clone(), creator.clone());
        if env.storage().persistent().has(&key) {
            panic!("stream already exists");
        }

        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(&subscriber, &env.current_contract_address(), &amount);

        let stream = Stream {
            token,
            rate_per_second,
            balance: amount,
            last_collected: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &stream);
    }

    pub fn collect(env: Env, subscriber: Address, creator: Address) {
        let key = DataKey::Stream(subscriber.clone(), creator.clone());
        if !env.storage().persistent().has(&key) {
            panic!("stream not found");
        }

        let mut stream: Stream = env.storage().persistent().get(&key).unwrap();
        let current_time = env.ledger().timestamp();

        if current_time <= stream.last_collected {
            return;
        }

        let time_elapsed = (current_time - stream.last_collected) as i128;
        let mut amount_to_collect = time_elapsed * stream.rate_per_second;

        if amount_to_collect > stream.balance {
            amount_to_collect = stream.balance;
        }

        if amount_to_collect > 0 {
            let token_client = TokenClient::new(&env, &stream.token);
            token_client.transfer(
                &env.current_contract_address(),
                &creator,
                &amount_to_collect,
            );

            stream.balance -= amount_to_collect;
            stream.last_collected = current_time;

            env.storage().persistent().set(&key, &stream);
        }
    }

    pub fn cancel(env: Env, subscriber: Address, creator: Address) {
        subscriber.require_auth();

        let key = DataKey::Stream(subscriber.clone(), creator.clone());
        if !env.storage().persistent().has(&key) {
            panic!("stream not found");
        }

        // First collect any pending amount
        Self::collect(env.clone(), subscriber.clone(), creator.clone());

        // Get updated stream
        let stream: Stream = env.storage().persistent().get(&key).unwrap();

        // Refund remaining balance to subscriber
        if stream.balance > 0 {
            let token_client = TokenClient::new(&env, &stream.token);
            token_client.transfer(
                &env.current_contract_address(),
                &subscriber,
                &stream.balance,
            );
        }

        // Remove the stream from storage
        env.storage().persistent().remove(&key);
    }

    pub fn top_up(env: Env, subscriber: Address, creator: Address, amount: i128) {
        subscriber.require_auth();
        if amount <= 0 {
            panic!("amount must be positive");
        }

        let key = DataKey::Stream(subscriber.clone(), creator.clone());
        if !env.storage().persistent().has(&key) {
            panic!("stream not found");
        }

        let mut stream: Stream = env.storage().persistent().get(&key).unwrap();
        let token_client = TokenClient::new(&env, &stream.token);
        token_client.transfer(&subscriber, &env.current_contract_address(), &amount);

        stream.balance += amount;
        env.storage().persistent().set(&key, &stream);
    }

    /// Change the stream rate (tier) in one transaction without removing the stream.
    /// Pending payouts at the previous rate are settled via `collect` first.
    /// On downgrade (lower rate), excess buffer is prorated and refunded to the subscriber.
    /// On upgrade, `additional_deposit` can add tokens in the same transaction (use 0 if none).
    pub fn migrate_tier(
        env: Env,
        subscriber: Address,
        creator: Address,
        new_rate_per_second: i128,
        additional_deposit: i128,
    ) {
        subscriber.require_auth();

        if new_rate_per_second <= 0 {
            panic!("new rate must be positive");
        }
        if additional_deposit < 0 {
            panic!("additional deposit must be non-negative");
        }

        let key = DataKey::Stream(subscriber.clone(), creator.clone());
        if !env.storage().persistent().has(&key) {
            panic!("stream not found");
        }

        let stream_before: Stream = env.storage().persistent().get(&key).unwrap();
        let old_rate = stream_before.rate_per_second;

        Self::collect(env.clone(), subscriber.clone(), creator.clone());

        let mut stream: Stream = env.storage().persistent().get(&key).unwrap();
        let mut balance = stream.balance;

        if new_rate_per_second < old_rate && balance > 0 {
            let tokens_to_keep = balance
                .checked_mul(new_rate_per_second)
                .expect("overflow")
                .checked_div(old_rate)
                .expect("old rate must be positive");
            let refund = balance.saturating_sub(tokens_to_keep);
            if refund > 0 {
                let token_client = TokenClient::new(&env, &stream.token);
                token_client.transfer(&env.current_contract_address(), &subscriber, &refund);
                balance = tokens_to_keep;
            }
        }

        stream.rate_per_second = new_rate_per_second;
        stream.balance = balance;

        if additional_deposit > 0 {
            let token_client = TokenClient::new(&env, &stream.token);
            token_client.transfer(
                &subscriber,
                &env.current_contract_address(),
                &additional_deposit,
            );
            stream.balance = stream
                .balance
                .checked_add(additional_deposit)
                .expect("overflow");
        }

        env.storage().persistent().set(&key, &stream);

        if old_rate != new_rate_per_second {
            TierChanged {
                subscriber: subscriber.clone(),
                creator: creator.clone(),
                old_rate,
                new_rate: new_rate_per_second,
            }
            .publish(&env);
        }
    }
}

mod test;
