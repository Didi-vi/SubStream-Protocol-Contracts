#![no_std]
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, vec, Address, Bytes, Env, Vec};

// --- Constants ---
const MINIMUM_FLOW_DURATION: u64 = 86400;
const FREE_TRIAL_DURATION: u64 = 7 * 24 * 60 * 60;
const GRACE_PERIOD: u64 = 24 * 60 * 60; 
const GENESIS_NFT_ADDRESS: &str = "CAS3J7GYCCX7RRBHAHXDUY3OOWFMTIDDNVGCH6YOY7W7Y7G656H2HHMA";
const DISCOUNT_BPS: i128 = 2000; 
const SIX_MONTHS: u64 = 180 * 24 * 60 * 60;

// --- Helper: Charge Calculation ---
fn calculate_discounted_charge(start_time: u64, charge_start: u64, now: u64, base_rate: i128) -> i128 {
    if now <= charge_start {
        return 0;
    }

    let mut total_charge: i128 = 0;
    let mut current_t = charge_start;

    while current_t < now {
        let elapsed_since_start = current_t.saturating_sub(start_time);
        let periods = elapsed_since_start / SIX_MONTHS;
        let percent_discount = periods * 5;
        let discount = if percent_discount > 100 { 100 } else { percent_discount };

        let current_rate = base_rate * (100 - discount as i128) / 100;

        let next_boundary = start_time + (periods + 1) * SIX_MONTHS;
        let end_t = if now < next_boundary { now } else { next_boundary };

        let duration = (end_t - current_t) as i128;
        total_charge = total_charge.saturating_add(duration.saturating_mul(current_rate));

        current_t = end_t;
    }
    total_charge
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Subscription(Address, Address),
    TotalStreamed(Address, Address),
    CliffThreshold(Address),
    CreatorSubscribers(Address),
    CreatorMetadata(Address),
    ChannelPaused(Address),
    GiftsReceived(Address),
    CreatorSplit(Address),
    ContractAdmin,
    VerifiedCreator(Address),
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitPartition {
    pub partner: Address,
    pub percentage: u32,
}

// --- Events ---
#[contractevent]
pub struct TierChanged {
    #[topic] pub subscriber: Address,
    #[topic] pub creator: Address,
    pub old_rate: i128,
    pub new_rate: i128,
}

#[contractevent]
pub struct Subscribed {
    #[topic] pub subscriber: Address,
    #[topic] pub creator: Address,
    pub rate_per_second: i128,
}

#[contractevent]
pub struct Unsubscribed {
    #[topic] pub subscriber: Address,
    #[topic] pub creator: Address,
}

#[contractevent]
pub struct TipReceived {
    #[topic] pub user: Address,
    #[topic] pub creator: Address,
    #[topic] pub token: Address,
    pub amount: i128,
}

#[contractevent]
pub struct CreatorVerified {
    #[topic] pub creator: Address,
    #[topic] pub verified_by: Address,
}

#[contract]
pub struct SubStreamContract;

#[contractimpl]
impl SubStreamContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::ContractAdmin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::ContractAdmin, &admin);
    }

    pub fn verify_creator(env: Env, admin: Address, creator: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().persistent().get(&DataKey::ContractAdmin).expect("not initialized");
        if admin != stored_admin { panic!("admin only"); }

        env.storage().persistent().set(&DataKey::VerifiedCreator(creator.clone()), &true);
        CreatorVerified { creator, verified_by: admin }.publish(&env);
    }

    pub fn is_creator_verified(env: Env, creator: Address) -> bool {
        env.storage().persistent().get(&DataKey::VerifiedCreator(creator)).unwrap_or(false)
    }

    pub fn subscribe(env: Env, subscriber: Address, creator: Address, token: Address, amount: i128, rate_per_second: i128) {
        Self::subscribe_gift(&env, subscriber.clone(), subscriber, creator, token, amount, rate_per_second);
    }

    pub fn subscribe_gift(env: &Env, payer: Address, beneficiary: Address, creator: Address, token: Address, amount: i128, rate_per_second: i128) {
        subscribe_core(env, &payer, &beneficiary, &creator, &token, amount, rate_per_second, vec![env, creator.clone()], vec![env, 100u32]);
    }

    pub fn is_subscribed(env: Env, subscriber: Address, creator: Address) -> bool {
        let key = subscription_key(&subscriber, &creator);
        if !subscription_exists(&env, &key) { return false; }
        
        let sub = get_subscription(&env, &key);
        if sub.tier.rate_per_second <= 0 { return false; }

        let trial_end = sub.start_time.saturating_add(sub.tier.trial_duration);
        let charge_start = if sub.last_collected > trial_end { sub.last_collected } else { trial_end };
        let now = env.ledger().timestamp();

        if now <= charge_start { return true; }

        // Use the discounted charge logic for consistent "is active" checks
        let potential_charge = calculate_discounted_charge(sub.start_time, charge_start, now, sub.tier.rate_per_second);

        if sub.balance > potential_charge { return true; }

        // Grace period check
        if sub.last_funds_exhausted > 0 {
            let grace_period_end = sub.last_funds_exhausted.saturating_add(GRACE_PERIOD);
            if now <= grace_period_end { return true; }
        }
        false
    }

    pub fn collect(env: Env, subscriber: Address, creator: Address) {
        distribute_and_collect(&env, &subscriber, &creator, Some(&creator));
    }

    pub fn top_up(env: Env, subscriber: Address, stream_id: Address, amount: i128) {
        top_up_internal(&env, &subscriber, &stream_id, amount);
    }

    pub fn cancel(env: Env, subscriber: Address, creator: Address) {
        cancel_internal(&env, &subscriber, &creator);
    }

    pub fn tip(env: Env, user: Address, creator: Address, token: Address, amount: i128) {
        user.require_auth();
        if amount <= 0 || user == creator { panic!("invalid tip"); }
        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(&user, &creator, &amount);
        TipReceived { user, creator, token, amount }.publish(&env);
    }
}

// --- Internal Logic & Helpers ---

fn subscription_key(subscriber: &Address, stream_id: &Address) -> DataKey {
    DataKey::Subscription(subscriber.clone(), stream_id.clone())
}

fn subscription_exists(env: &Env, key: &DataKey) -> bool {
    env.storage().persistent().has(key) || env.storage().temporary().has(key)
}

fn get_subscription(env: &Env, key: &DataKey) -> Subscription {
    if let Some(sub) = env.storage().persistent().get(key) { sub }
    else { env.storage().temporary().get(key).expect("not found") }
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

fn distribute_and_collect(env: &Env, beneficiary: &Address, stream_id: &Address, total_streamed_creator: Option<&Address>) -> i128 {
    let key = subscription_key(beneficiary, stream_id);
    let mut sub = get_subscription(env, &key);
    let now = env.ledger().timestamp();

    if now <= sub.last_collected { return 0; }
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

    let mut amount_to_collect = calculate_discounted_charge(sub.start_time, charge_start, now, sub.tier.rate_per_second);
    
    // Check if grace period is active or expired
    if sub.balance <= 0 && sub.last_funds_exhausted > 0 {
        let grace_period_end = sub.last_funds_exhausted.saturating_add(GRACE_PERIOD);
        if now > grace_period_end { return 0; }
    }

    if amount_to_collect > sub.balance {
        if sub.last_funds_exhausted == 0 { sub.last_funds_exhausted = now; }
        // During grace period, we cap payout at available balance to prevent contract draining
    }

    let available_balance = sub.balance.max(0);
    let amount_to_payout = amount_to_collect.min(available_balance);

    if amount_to_payout > 0 {
        let token_client = TokenClient::new(env, &sub.token);
        let creators_len = sub.creators.len();
        let mut remaining = amount_to_payout;

        for i in 0..creators_len {
            let creator = sub.creators.get(i).unwrap();
            let share = sub.percentages.get(i).unwrap() as i128;
            let payout = if i + 1 == creators_len { remaining } else { (amount_to_payout * share) / 100 };
            remaining -= payout;
            if payout > 0 {
                token_client.transfer(&env.current_contract_address(), &creator, &payout);
            }
        }
    }

    sub.balance -= amount_to_collect;
    sub.last_collected = now;
    set_subscription(env, &key, &sub);
    amount_to_collect
}

fn top_up_internal(env: &Env, beneficiary: &Address, stream_id: &Address, amount: i128) {
    let key = subscription_key(beneficiary, stream_id);
    let mut sub = get_subscription(env, &key);
    sub.payer.require_auth();

    let token_client = TokenClient::new(env, &sub.token);
    token_client.transfer(&sub.payer, &env.current_contract_address(), &amount);
    
    sub.balance += amount;
    if sub.balance > 0 { sub.last_funds_exhausted = 0; }
    set_subscription(env, &key, &sub);
    distribute_and_collect(env, beneficiary, stream_id, None);
}

fn cancel_internal(env: &Env, beneficiary: &Address, stream_id: &Address) {
    let key = subscription_key(beneficiary, stream_id);
    let mut sub = get_subscription(env, &key);
    sub.payer.require_auth();

    if env.ledger().timestamp() < sub.start_time + MINIMUM_FLOW_DURATION { panic!("too early"); }

    distribute_and_collect(env, beneficiary, stream_id, None);
    sub = get_subscription(env, &key); // Refresh after collect

    if sub.balance > 0 {
        let token_client = TokenClient::new(env, &sub.token);
        token_client.transfer(&env.current_contract_address(), &sub.payer, &sub.balance);
    }
    env.storage().persistent().remove(&key);
    env.storage().temporary().remove(&key);
}

fn subscribe_core(env: &Env, payer: &Address, beneficiary: &Address, stream_id: &Address, token: &Address, amount: i128, rate: i128, creators: Vec<Address>, percentages: Vec<u32>) {
    payer.require_auth();
    let key = subscription_key(beneficiary, stream_id);
    if subscription_exists(env, &key) { panic!("exists"); }

    let token_client = TokenClient::new(env, token);
    token_client.transfer(payer, &env.current_contract_address(), &amount);

    let now = env.ledger().timestamp();
    let sub = Subscription {
        token: token.clone(),
        tier: Tier { rate_per_second: rate, trial_duration: FREE_TRIAL_DURATION },
        balance: amount,
        last_collected: now,
        start_time: now,
        last_funds_exhausted: 0,
        creators,
        percentages,
        payer: payer.clone(),
        beneficiary: beneficiary.clone(),
    };
    set_subscription(env, &key, &sub);
    Subscribed { subscriber: beneficiary.clone(), creator: stream_id.clone(), rate_per_second: rate }.publish(env);
}

fn is_creator_paused(env: &Env, creator: &Address) -> bool {
    env.storage().persistent().get(&DataKey::ChannelPaused(creator.clone())).unwrap_or(false)
}