#![no_std]
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, vec, Address, Bytes, Env, Vec};

// Minimum flow duration: 24 hours in seconds (24 * 60 * 60 = 86400)
const MINIMUM_FLOW_DURATION: u64 = 86400;
const FREE_TRIAL_DURATION: u64 = 7 * 24 * 60 * 60;
const GRACE_PERIOD: u64 = 24 * 60 * 60; // 24 hours in seconds
const GENESIS_NFT_ADDRESS: &str = "CAS3J7GYCCX7RRBHAHXDUY3OOWFMTIDDNVGCH6YOY7W7Y7G656H2HHMA";
const DISCOUNT_BPS: i128 = 2000; // 20% discount

fn is_genesis_member(env: &Env, user: &Address) -> bool {
    let nft_address = Address::from_string(&soroban_sdk::String::from_str(env, GENESIS_NFT_ADDRESS));
    let client = TokenClient::new(env, &nft_address);
    client.balance(user) > 0
}

fn apply_discount(rate: i128) -> i128 {
    rate * (10000 - DISCOUNT_BPS) / 10000
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Subscription(Address, Address),   // (beneficiary, stream_id)
    TotalStreamed(Address, Address), // (beneficiary, creator) - cumulative tokens streamed
    CliffThreshold(Address),         // creator -> threshold amount for access
    CreatorSubscribers(Address),     // creator -> Vec<beneficiary>
    CreatorMetadata(Address),        // creator -> IPFS CID bytes
    ChannelPaused(Address),          // creator -> bool
    GiftsReceived(Address),          // beneficiary -> Vec<stream_id>
    CreatorSplit(Address),           // creator -> Vec<(Address, u32)>
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tier {
    pub rate_per_second: i128,
    pub trial_duration: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub token: Address,
    pub tier: Tier,
    pub balance: i128,
    pub last_collected: u64,
    pub start_time: u64,
    pub last_funds_exhausted: u64,
    pub creators: Vec<Address>,
    pub percentages: Vec<u32>,
    pub payer: Address,
    pub beneficiary: Address,
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

#[contractevent]
pub struct Subscribed {
    #[topic]
    pub subscriber: Address,
    #[topic]
    pub creator: Address,
    pub rate_per_second: i128,
}

#[contractevent]
pub struct Unsubscribed {
    #[topic]
    pub subscriber: Address,
    #[topic]
    pub creator: Address,
}

#[contractevent]
pub struct TipReceived {
    #[topic]
    pub user: Address,
    #[topic]
    pub creator: Address,
    #[topic]
    pub token: Address,
    pub amount: i128,
}

#[contract]
pub struct SubStreamContract;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitPartition {
    pub partner: Address,
    pub percentage: u32,
}

fn subscription_key(subscriber: &Address, stream_id: &Address) -> DataKey {
    DataKey::Subscription(subscriber.clone(), stream_id.clone())
}

/// Validates that the creators/percentages arrays are consistent and sum to 100.

fn stream_exists(env: &Env, key: &DataKey) -> bool {
    env.storage().persistent().has(key) || env.storage().temporary().has(key)
}

fn get_stream(env: &Env, key: &DataKey) -> Stream {
    if env.storage().persistent().has(key) {
        env.storage().persistent().get(key).unwrap()
    } else if env.storage().temporary().has(key) {
        env.storage().temporary().get(key).unwrap()
    } else {
        panic!("stream not found")
    }
}

fn set_stream(env: &Env, key: &DataKey, stream: &Stream) {
    if stream.balance > 0 {
        env.storage().persistent().set(key, stream);
        env.storage().temporary().remove(key);
    } else {
        env.storage().temporary().set(key, stream);
        env.storage().persistent().remove(key);
    }
}

fn remove_stream(env: &Env, key: &DataKey) {
    env.storage().persistent().remove(key);
    env.storage().temporary().remove(key);
}

fn validate_distribution(
    creators: &Vec<Address>,
    percentages: &Vec<u32>,
    expected_creator_count: u32,
) {
    if creators.len() != expected_creator_count {
        if expected_creator_count == 5 {
            panic!("group channel must contain exactly 5 creators");
        }
        panic!("invalid creator count");
    }
    if percentages.len() != creators.len() {
        panic!("creators and percentages length mismatch");
    }
    let mut total: u32 = 0;
    let len = creators.len();
    for i in 0..len {
        let percentage = percentages.get(i).unwrap();
        if percentage == 0 {
            panic!("percentages must be positive");
        }
        total = total.checked_add(percentage).expect("overflow");

        let creator_i = creators.get(i).unwrap();
        for j in (i + 1)..len {
            if creator_i == creators.get(j).unwrap() {
                panic!("creators must be unique");
            }
        }
    }

    if total != 100 {
        panic!("percentages must sum to 100");
    }
}

fn subscription_exists(env: &Env, key: &DataKey) -> bool {
    env.storage().persistent().has(key) || env.storage().temporary().has(key)
}

fn get_subscription(env: &Env, key: &DataKey) -> Subscription {
    if env.storage().persistent().has(key) {
        env.storage().persistent().get(key).unwrap()
    } else if env.storage().temporary().has(key) {
        env.storage().temporary().get(key).unwrap()
    } else {
        panic!("subscription not found")
    }
}

fn set_subscription(env: &Env, key: &DataKey, sub: &Subscription) {
    if sub.balance > 0 {
        env.storage().persistent().set(key, sub);
        env.storage().temporary().remove(key);
    } else {
        env.storage().temporary().set(key, sub);
        env.storage().persistent().remove(key);
    }
}

fn remove_subscription(env: &Env, key: &DataKey) {
    env.storage().persistent().remove(key);
    env.storage().temporary().remove(key);
}

// ---------------------------------------------------------------------------
// Public contract interface
// ---------------------------------------------------------------------------

#[contractimpl]
impl SubStreamContract {
    /// Direct subscription where the signer is the payer and beneficiary.
    pub fn subscribe(
        env: Env,
        subscriber: Address,
        creator: Address,
        token: Address,
        amount: i128,
        rate_per_second: i128,
    ) {
        Self::subscribe_gift(&env, subscriber.clone(), subscriber, creator, token, amount, rate_per_second);
    }

    /// Sponsored subscription: `payer` pays for `beneficiary`'s access.
    pub fn subscribe_gift(
        env: &Env,
        payer: Address,
        beneficiary: Address,
        creator: Address,
        token: Address,
        amount: i128,
        rate_per_second: i128,
    ) {
        subscribe_core(
            env,
            &payer,
            &beneficiary,
            &creator,
            &token,
            amount,
            rate_per_second,
            vec![env, creator.clone()],
            vec![env, 100u32],
        );
        subscriber.require_auth();

        if amount <= 0 || rate_per_second <= 0 {
            panic!("amount and rate must be positive");
        }

        let key = stream_key(&subscriber, &creator);
        if stream_exists(&env, &key) {
            panic!("stream already exists");
        }

        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(&subscriber, &env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();
        let stream = Stream {
            token: token.clone(),
            tier: Tier {
                rate_per_second,
                trial_duration: FREE_TRIAL_DURATION,
            },
            balance: amount,
            last_collected: now,
            start_time: now,
            last_funds_exhausted: 0,
            creators: vec![&env, creator.clone()],
            percentages: vec![&env, 100u32],
        };

        env.storage().persistent().set(&key, &stream);

        add_subscriber_to_creator(&env, &creator, &subscriber);

        Subscribed {
            subscriber: subscriber.clone(),
            creator: creator.clone(),
            rate_per_second,
        }.publish(&env);
    }

    pub fn collect(env: Env, subscriber: Address, creator: Address) {
        // distribute_and_collect handles trial duration, pauses, and multi-creator payouts.
        // For single creators, it also updates the total_streamed (access tier) count.
        distribute_and_collect(&env, &subscriber, &creator, Some(&creator));
    }

    pub fn cancel(env: Env, subscriber: Address, creator: Address) {
        cancel_internal(&env, &subscriber, &creator);
    }

    pub fn top_up(env: Env, subscriber: Address, stream_id: Address, amount: i128) {
        top_up_internal(&env, &subscriber, &stream_id, amount);
    }

    /// View: returns true only if the user has active funds remaining (not expired)
    pub fn is_subscribed(env: Env, subscriber: Address, creator: Address) -> bool {
        let key = subscription_key(&subscriber, &creator);
        if !env.storage().persistent().has(&key) && !env.storage().temporary().has(&key) {
            return false;
        }
        let sub = get_subscription(&env, &key);
        if sub.tier.rate_per_second <= 0 {
            return false;
        }

        let trial_end = sub.start_time.saturating_add(sub.tier.trial_duration);
        let charge_start = if sub.last_collected > trial_end {
            sub.last_collected
        } else {
            trial_end
        };

        let now = env.ledger().timestamp();
        if now <= charge_start {
            return true;
        }

        let elapsed = (now - charge_start) as i128;
        let potential_charge = elapsed
            .checked_mul(sub.tier.rate_per_second)
            .unwrap_or(0);
        
        if sub.balance > potential_charge {
            return true;
        }

        // Grace period check
        if sub.last_funds_exhausted > 0 {
            let grace_period_end = sub.last_funds_exhausted.saturating_add(GRACE_PERIOD);
            if now <= grace_period_end {
                return true;
            }
        }
        
        false
    }

    // Group channel wrappers
    pub fn subscribe_group(
        env: Env,
        subscriber: Address,
        channel_id: Address,
        token: Address,
        amount: i128,
        rate_per_second: i128,
        creators: Vec<Address>,
        percentages: Vec<u32>,
    ) {
        Self::subscribe_group_gift(&env, subscriber.clone(), subscriber, channel_id, token, amount, rate_per_second, creators, percentages);
    }

    /// Sponsored group subscription: `payer` pays for `beneficiary`'s access.
    pub fn subscribe_group_gift(
        env: &Env,
        payer: Address,
        beneficiary: Address,
        channel_id: Address,
        token: Address,
        amount: i128,
        rate_per_second: i128,
        creators: Vec<Address>,
        percentages: Vec<u32>,
    ) {
        validate_distribution(&creators, &percentages, 5);
        subscribe_core(
            env,
            &payer,
            &beneficiary,
            &channel_id,
            &token,
            amount,
            rate_per_second,
            creators,
            percentages,
        );
    }

    pub fn collect_group(env: Env, subscriber: Address, channel_id: Address) {
        distribute_and_collect(&env, &subscriber, &channel_id, None);
    }

    pub fn cancel_group(env: Env, subscriber: Address, channel_id: Address) {
        cancel_group_internal(&env, &subscriber, &channel_id);
    }

    pub fn top_up_group(env: Env, subscriber: Address, channel_id: Address, amount: i128) {
        top_up_internal(&env, &subscriber, &channel_id, amount);
    }

    /// Retrieve all channel IDs a user is currently gifted for.
    pub fn get_gifts_received(env: Env, beneficiary: Address) -> Vec<Address> {
        let gift_key = DataKey::GiftsReceived(beneficiary.clone());
        env.storage().persistent().get(&gift_key).unwrap_or(vec![&env])
    }

    /// Creator-level pause: stops charging all incoming streams for this creator.
    pub fn pause_channel(env: Env, creator: Address) {
        creator.require_auth();

        if is_creator_paused(&env, &creator) {
            return;
        }

        let key = DataKey::CreatorSubscribers(creator.clone());
        let subs: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![&env]);

        // Settle all streams up to pause timestamp, then freeze charging.
        for subscriber in subs.iter() {
            let s_key = subscription_key(&subscriber, &creator);
            if subscription_exists(&env, &s_key) {
                distribute_and_collect(&env, &subscriber, &creator, Some(&creator));
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::ChannelPaused(creator), &true);
    }

    pub fn unpause_channel(env: Env, creator: Address) {
        creator.require_auth();

        if !is_creator_paused(&env, &creator) {
            return;
        }

        let key = DataKey::CreatorSubscribers(creator.clone());
        let subs: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![&env]);
        let now = env.ledger().timestamp();

        // Resume billing from now so paused window is never charged.
        for beneficiary in subs.iter() {
            let s_key = subscription_key(&beneficiary, &creator);
            if subscription_exists(&env, &s_key) {
                let mut sub = get_subscription(&env, &s_key);
                sub.last_collected = now;
                set_subscription(&env, &s_key, &sub);
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::ChannelPaused(creator), &false);
    }

    /// Update revenue sharing for a creator. Only affects NEWly collected tokens.
    /// Supports up to 5 partners. Total percentages must equal 100.
    pub fn set_creator_split(env: Env, creator: Address, partitions: Vec<SplitPartition>) {
        creator.require_auth();
        
        if partitions.len() > 5 {
            panic!("max 5 split partners");
        }
        
        let mut total: u32 = 0;
        for p in partitions.iter() {
            total += p.percentage;
        }
        
        if total != 100 {
            panic!("percentages must sum to 100");
        }
        
        env.storage().persistent().set(&DataKey::CreatorSplit(creator), &partitions);
    }

    pub fn is_channel_paused(env: Env, creator: Address) -> bool {
        is_creator_paused(&env, &creator)
    }

    /// Transition a Subscription to a new tier (rate) in a single atomic transaction.
    /// Calculates pro-rated earnings at the old rate first to prevent the "double-start" bug.
    /// Optionally accepts an `additional_amount` to top-up the Subscription balance during migration.
    pub fn migrate_tier(
        env: Env,
        subscriber: Address,
        stream_id: Address,
        mut new_rate_per_second: i128,
        additional_amount: i128,
    ) {
        subscriber.require_auth();

        let key = subscription_key(&subscriber, &stream_id);
        if !subscription_exists(&env, &key) {
            panic!("Subscription not found");
        }

        // Apply NFT discount if applicable
        if is_genesis_member(&env, &subscriber) {
            new_rate_per_second = apply_discount(new_rate_per_second);
        }

        let mut sub = get_subscription(&env, &key);
        let old_rate = sub.tier.rate_per_second;

        // 1. Pro-rate earnings at the OLD rate up to EXACTLY NOW
        let creator_to_update = if sub.creators.len() == 1 {
            Some(sub.creators.get(0).unwrap())
        } else {
            None
        };
        distribute_and_collect(&env, &subscriber, &stream_id, creator_to_update.as_ref());

        // Refresh Subscription state after collection
        sub = get_subscription(&env, &key);

        // 2. Apply the NEW rate starting from this second
        sub.tier.rate_per_second = new_rate_per_second;

        // 3. Atomically add more funds if requested
        if additional_amount > 0 {
            let token_client = TokenClient::new(&env, &sub.token);
            token_client.transfer(&subscriber, &env.current_contract_address(), &additional_amount);
            sub.balance += additional_amount;
        }

        set_subscription(&env, &key, &sub);

        // Notify indexing services of the tier change
        env.events().publish(
            (symbol_short!("TierChg"), subscriber.clone(), stream_id.clone()),
            (old_rate, new_rate_per_second),
        );
        TierChanged {
            subscriber: subscriber.clone(),
            creator: stream_id.clone(),
            old_rate,
            new_rate: new_rate_per_second,
        }.publish(&env);
    }

    /// Collect from all active streams for a creator in a single call.
    /// `max_count` caps the batch size to avoid hitting ledger instruction limits.
    /// Returns the total amount collected across all processed streams.
    pub fn withdraw_all(env: Env, creator: Address, max_count: u32) -> i128 {
        let subs_key = DataKey::CreatorSubscribers(creator.clone());
        let subs: Vec<Address> = env.storage().persistent().get(&subs_key).unwrap_or(vec![&env]);

        let mut total: i128 = 0;
        let limit = max_count.min(subs.len());

        for i in 0..limit {
            let subscriber = subs.get(i).unwrap();
            let s_key = subscription_key(&subscriber, &creator);
            if subscription_exists(&env, &s_key) {
                total += distribute_and_collect(&env, &subscriber, &creator, Some(&creator));
            }
        }

        total
    }

    /// Read-only helper: calculates the total "earned but not yet withdrawn"
    /// balance for `creator` across all their active subscriber streams.
    ///
    /// This function performs no transfers or state mutations. It uses the
    /// current ledger timestamp to compute real-time accrued amounts, making
    /// it suitable for frontend dashboards to display "Current Unclaimed Balance"
    /// without requiring multiple RPC calls or expensive client-side iteration.
    ///
    /// Calculation per stream:
    ///   1. Skip streams that are paused (channel-level pause).
    ///   2. Respect the free trial window Ã¢â‚¬â€ no earnings accrue during the trial.
    ///   3. Accrue `rate_per_second * elapsed_billable_seconds`, capped at
    ///      each stream's remaining balance.
    ///   4. Apply the creator's share percentage for group channels.
    ///
    /// Returns the sum of all unclaimed amounts denominated in the stream's
    /// native token units (stroops for XLM-based tokens).
    pub fn calculate_total_earned(env: Env, creator: Address) -> i128 {
        let subs_key = DataKey::CreatorSubscribers(creator.clone());
        let subs: Vec<Address> = env
            .storage()
            .persistent()
            .get(&subs_key)
            .unwrap_or(vec![&env]);

        let now = env.ledger().timestamp();
        let channel_paused = is_creator_paused(&env, &creator);

        let mut total_earned: i128 = 0;

        for subscriber in subs.iter() {
            // We need to check both the direct Subscription (id=creator) and 
            // any group streams where this creator is a participant.
            // Since we index subscribers under the creator, we check:
            // 1. Is there a direct Subscription for this subscriber?
            let direct_key = subscription_key(&subscriber, &creator);
            if subscription_exists(&env, &direct_key) {
                total_earned += calculate_stream_earned(&env, &direct_key, &creator, now, channel_paused);
            }

            // Note: Support for multiple group streams per subscriber for the same creator 
            // would require a more advanced index (e.g. CreatorSubscribers storing stream_ids).
            // For now, this handles the primary use case.
        }

        total_earned
    }

 

    pub fn set_cliff_threshold(env: Env, creator: Address, threshold: i128) {
        creator.require_auth();

        if threshold < 0 {
            panic!("threshold must be non-negative");
        }

        env.storage()
            .persistent()
            .set(&DataKey::CliffThreshold(creator), &threshold);
    }

    pub fn get_cliff_threshold(env: Env, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::CliffThreshold(creator))
            .unwrap_or(0)
    }

    /// Store an IPFS CID pointing to the creator's profile, links, and tier descriptions.
    /// Only the creator themselves can update their own metadata.
    pub fn set_creator_metadata(env: Env, creator: Address, cid: Bytes) {
        creator.require_auth();
        let key = DataKey::CreatorMetadata(creator.clone());
        env.storage().persistent().set(&key, &cid);
    }

    /// Retrieve the IPFS CID for a creator. Returns None if not set.
    pub fn get_creator_metadata(env: Env, creator: Address) -> Option<Bytes> {
        let key = DataKey::CreatorMetadata(creator.clone());
        env.storage().persistent().get(&key)
    }

    pub fn get_total_streamed(env: Env, subscriber: Address, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalStreamed(subscriber, creator))
            .unwrap_or(0)
    }

    pub fn has_unlocked_access(env: Env, subscriber: Address, creator: Address) -> bool {
        let threshold: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::CliffThreshold(creator.clone()))
            .unwrap_or(0);

        if threshold == 0 {
            return true;
        }

        // During the free trial window, access is always unlocked.
        if Self::is_subscribed(env.clone(), subscriber.clone(), creator.clone()) {
            return true;
        }

        let total_streamed: i128 = env.storage().persistent().get(&DataKey::TotalStreamed(subscriber, creator)).unwrap_or(0);
        total_streamed >= threshold
    }

    pub fn get_access_tier(env: Env, subscriber: Address, creator: Address) -> u32 {
        let threshold_key = DataKey::CliffThreshold(creator.clone());
        let threshold: i128 = env.storage().persistent().get(&threshold_key).unwrap_or(0);
        if threshold == 0 {
            return 2;
        }
        let streamed_key = DataKey::TotalStreamed(subscriber.clone(), creator.clone());
        let total_streamed: i128 = env.storage().persistent().get(&streamed_key).unwrap_or(0);

        if total_streamed >= 250 {
            3
        } else if total_streamed >= 150 {
            2
        } else if total_streamed >= 50 {
            1
        } else {
            0
        }
    }

    pub fn tip(env: Env, user: Address, creator: Address, token: Address, amount: i128) {
        user.require_auth();
        
        if amount <= 0 {
            panic!("amount must be positive");
        }
        
        if user == creator {
            panic!("cannot tip yourself");
        }
        
        // Direct transfer from user to creator
        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(&user, &creator, &amount);
        
        // Emit TipReceived event: topics = (event_name, user, creator, token), data = amount
        env.events().publish(
            (symbol_short!("TipRcvd"), user.clone(), creator.clone(), token.clone()),
        // Emit TipReceived event
        TipReceived {
            user: user.clone(),
            creator: creator.clone(),
            token: token.clone(),
            amount,
        }.publish(&env);
    }
}

fn calculate_stream_earned(env: &Env, key: &DataKey, creator: &Address, now: u64, channel_paused: bool) -> i128 {
    if channel_paused { return 0; }
    let sub = get_subscription(env, key);
    if sub.balance <= 0 || sub.tier.rate_per_second <= 0 { return 0; }

    let trial_end = sub.start_time.saturating_add(sub.tier.trial_duration);
    let charge_start = if sub.last_collected > trial_end { sub.last_collected } else { trial_end };
    if now <= charge_start { return 0; }

    let elapsed = (now - charge_start) as i128;
    let mut gross_earned = elapsed.checked_mul(sub.tier.rate_per_second).unwrap_or(i128::MAX);
    if gross_earned > sub.balance { gross_earned = sub.balance; }

    if sub.creators.len() > 1 {
        if let Some(idx) = sub.creators.iter().position(|c| c == *creator) {
            let percentage = sub.percentages.get(idx as u32).unwrap() as i128;
            return (gross_earned * percentage) / 100;
        }
        return 0;
    }
    gross_earned
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn is_creator_paused(env: &Env, creator: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ChannelPaused(creator.clone()))
        .unwrap_or(false)
}

fn add_subscriber_to_creator(env: &Env, creator: &Address, subscriber: &Address) {
    let key = DataKey::CreatorSubscribers(creator.clone());
    let mut subs: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![env]);

    for s in subs.iter() {
        if s == *subscriber {
            return;
        }
    }

    subs.push_back(subscriber.clone());
    env.storage().persistent().set(&key, &subs);
}

fn remove_subscriber_from_creator(env: &Env, creator: &Address, subscriber: &Address) {
    let key = DataKey::CreatorSubscribers(creator.clone());
    let subs: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![env]);

    let mut updated = vec![env];
    for s in subs.iter() {
        if s != *subscriber {
            updated.push_back(s);
        }
    }

    env.storage().persistent().set(&key, &updated);
}


fn update_total_streamed(env: &Env, subscriber: &Address, creator: &Address, amount: i128) {
    let key = DataKey::TotalStreamed(subscriber.clone(), creator.clone());
    let current_total: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&key, &(current_total + amount));
}

// ---------------------------------------------------------------------------
// Internal implementations
// ---------------------------------------------------------------------------

fn subscribe_core(
    env: &Env,
    payer: &Address,
    beneficiary: &Address,
    stream_id: &Address,
    token: &Address,
    amount: i128,
    mut rate_per_second: i128,
    creators: Vec<Address>,
    percentages: Vec<u32>,
) {
    payer.require_auth();

    if amount <= 0 || rate_per_second <= 0 {
        panic!("amount and rate must be positive");
    }

    let key = subscription_key(beneficiary, stream_id);
    if subscription_exists(env, &key) {
        panic!("subscription already exists for this beneficiary");
    }

    // Apply NFT discount if applicable
    if is_genesis_member(env, beneficiary) {
        rate_per_second = apply_discount(rate_per_second);
    }

    let token_client = TokenClient::new(env, token);
    token_client.transfer(payer, &env.current_contract_address(), &amount);

    let now = env.ledger().timestamp();
    let sub = Subscription {
        token: token.clone(),
        tier: Tier {
            rate_per_second,
            trial_duration: FREE_TRIAL_DURATION,
        },
        balance: amount,
        last_collected: now,
        start_time: now,
        creators: creators.clone(),
        percentages: percentages.clone(),
        payer: payer.clone(),
        beneficiary: beneficiary.clone(),
        last_funds_exhausted: 0,
    };
    set_subscription(env, &key, &sub);

    // Map gifted access to beneficiary
    if payer != beneficiary {
        let gift_key = DataKey::GiftsReceived(beneficiary.clone());
        let mut gifts: Vec<Address> = env.storage().persistent().get(&gift_key).unwrap_or(vec![env]);
        gifts.push_back(stream_id.clone());
        env.storage().persistent().set(&gift_key, &gifts);
    }

    // Track subscriber in each participant's subscriber map.
    for creator in creators.iter() {
        add_subscriber_to_creator(env, &creator, beneficiary);
    }

    Subscribed {
        subscriber: beneficiary.clone(),
        creator: stream_id.clone(),
        rate_per_second,
    }.publish(&env);
}

fn distribute_and_collect(
    env: &Env,
    beneficiary: &Address,
    stream_id: &Address,
    total_streamed_creator: Option<&Address>,
) -> i128 {
    let key = subscription_key(beneficiary, stream_id);
    let mut sub = get_subscription(env, &key);
    let now = env.ledger().timestamp();

    if now <= sub.last_collected {
        return 0;
    }

    if let Some(creator) = total_streamed_creator {
        if is_creator_paused(env, creator) {
            sub.last_collected = now;
            set_subscription(env, &key, &sub);
            return 0;
        }
    }

    let trial_end = sub.start_time.saturating_add(sub.tier.trial_duration);
    let charge_start = if sub.last_collected > trial_end { sub.last_collected } else { trial_end };
    if now <= charge_start { return 0; }

    let elapsed = (now - charge_start) as i128;
    let mut amount_to_collect = elapsed.checked_mul(sub.tier.rate_per_second).unwrap_or(0);
    
    // Debt management and grace period
    if amount_to_collect > sub.balance {
        if sub.last_funds_exhausted == 0 {
            sub.last_funds_exhausted = now;
        } else {
            let grace_period_end = sub.last_funds_exhausted.saturating_add(GRACE_PERIOD);
            if now > grace_period_end {
                amount_to_collect = sub.balance.max(0);
            }
        }
    } else {
        sub.last_funds_exhausted = 0;
    }

    if amount_to_collect <= 0 { return 0; }

    let available_balance = sub.balance.max(0);
    let amount_to_transfer = amount_to_collect.min(available_balance);

    if amount_to_transfer > 0 {
        let token_client = TokenClient::new(env, &sub.token);
        let mut remaining = amount_to_transfer;
        let mut distribution: Vec<(Address, u32)> = vec![env];
        
        if sub.creators.len() == 1 {
            let creator = sub.creators.get(0).unwrap();
            let split_key = DataKey::CreatorSplit(creator.clone());
            if env.storage().persistent().has(&split_key) {
                let partitions: Vec<SplitPartition> = env.storage().persistent().get(&split_key).unwrap();
                for p in partitions.iter() {
                    distribution.push_back((p.partner, p.percentage));
                }
            } else {
                distribution.push_back((creator, 100));
            }
        } else {
            for i in 0..sub.creators.len() {
                distribution.push_back((sub.creators.get(i).unwrap(), sub.percentages.get(i).unwrap()));
            }
        }

        let dist_len = distribution.len();
        for i in 0..dist_len {
            let (partner, percentage) = distribution.get(i).unwrap();
            let payout = if (i + 1) == dist_len {
                remaining
            } else {
                let p = (amount_to_transfer * percentage as i128) / 100;
                remaining -= p;
                p
            };

            if payout > 0 {
                token_client.transfer(&env.current_contract_address(), &partner, &payout);
            }
        }
    }

    sub.balance -= amount_to_collect;
    sub.last_collected = now;
    set_subscription(env, &key, &sub);

    for creator in sub.creators.iter() {
        update_total_streamed(env, beneficiary, &creator, amount_to_collect);
    }

    amount_to_collect
}

fn cancel_internal(env: &Env, beneficiary: &Address, stream_id: &Address) {
    let key = subscription_key(beneficiary, stream_id);
    if !subscription_exists(env, &key) {
        panic!("subscription not found");
    }

    let mut sub = get_subscription(env, &key);
    sub.payer.require_auth();

    let now = env.ledger().timestamp();
    if now < sub.start_time + MINIMUM_FLOW_DURATION {
        panic!("cannot cancel: minimum duration not met");
    }

    distribute_and_collect(env, beneficiary, stream_id, None);

    sub = get_subscription(env, &key);
    if sub.balance > 0 {
        let token_client = TokenClient::new(env, &sub.token);
        token_client.transfer(&env.current_contract_address(), &sub.payer, &sub.balance);
    }
    
    for creator in sub.creators.iter() {
        remove_subscriber_from_creator(env, &creator, beneficiary);
    }
    remove_subscription(env, &key);

    Unsubscribed {
        subscriber: beneficiary.clone(),
        creator: stream_id.clone(),
    }.publish(&env);
}

fn top_up_internal(env: &Env, beneficiary: &Address, stream_id: &Address, amount: i128) {
    if amount <= 0 {
        panic!("amount must be positive");
    }
    let key = subscription_key(beneficiary, stream_id);
    if !subscription_exists(env, &key) {
        panic!("subscription not found");
    }

    let mut sub = get_subscription(env, &key);
    sub.payer.require_auth();

    let token_client = TokenClient::new(env, &sub.token);
    token_client.transfer(&sub.payer, &env.current_contract_address(), &amount);
    
    sub.balance += amount;
    if sub.balance > 0 {
        sub.last_funds_exhausted = 0;
    }
    set_subscription(env, &key, &sub);
    
    distribute_and_collect(env, beneficiary, stream_id, None);
}

fn cancel_group_internal(env: &Env, subscriber: &Address, stream_id: &Address) {
    cancel_internal(env, subscriber, stream_id);
}
