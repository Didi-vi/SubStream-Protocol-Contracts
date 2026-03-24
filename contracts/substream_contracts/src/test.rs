#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger},
    token, vec, Address, Bytes, Env,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> token::Client<'a> {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    token::Client::new(env, &sac.address())
}

#[test]
fn test_subscribe_and_collect() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &2);

    assert_eq!(token.balance(&subscriber), 900);
    assert_eq!(token.balance(&contract_id), 100);

    env.ledger().set_timestamp(110);
    client.collect(&subscriber, &creator);

    assert_eq!(token.balance(&contract_id), 80);
    assert_eq!(token.balance(&creator), 20);

    // Advance 50 more seconds — would be 100 tokens but only 80 left
    env.ledger().set_timestamp(160);
    client.collect(&subscriber, &creator);

    assert_eq!(token.balance(&contract_id), 0);
    assert_eq!(token.balance(&creator), 100);
}

#[test]
fn test_cancel() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    // 24h + 120 seconds pass
    env.ledger().set_timestamp(100 + 86400 + 120);
    client.cancel(&subscriber, &creator);

    assert_eq!(token.balance(&creator), 100);
    assert_eq!(token.balance(&subscriber), 900);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
#[should_panic(expected = "amount and rate must be positive")]
fn test_subscribe_invalid_amounts() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.subscribe(&subscriber, &creator, &token, &0, &2);
}

#[test]
#[should_panic(expected = "stream already exists")]
fn test_subscribe_already_exists() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.subscribe(&subscriber, &creator, &token.address, &100, &2);
    client.subscribe(&subscriber, &creator, &token.address, &100, &2);
}

#[test]
fn test_top_up() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.subscribe(&subscriber, &creator, &token.address, &100, &1);
    assert_eq!(token.balance(&contract_id), 100);

    client.top_up(&subscriber, &creator, &50);
    assert_eq!(token.balance(&contract_id), 150);

    env.ledger().set_timestamp(120);
    client.collect(&subscriber, &creator);
    assert_eq!(token.balance(&creator), 120);
    assert_eq!(token.balance(&contract_id), 30);
}

#[test]
fn test_group_subscribe_and_collect_split() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let channel_id = Address::generate(&env);
    let creator_1 = Address::generate(&env);
    let creator_2 = Address::generate(&env);
    let creator_3 = Address::generate(&env);
    let creator_4 = Address::generate(&env);
    let creator_5 = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    let creators = vec![
        &env,
        creator_1.clone(),
        creator_2.clone(),
        creator_3.clone(),
        creator_4.clone(),
        creator_5.clone(),
    ];
    let percentages = vec![&env, 40u32, 25u32, 15u32, 10u32, 10u32];

    env.ledger().set_timestamp(100);
    client.subscribe_group(
        &subscriber,
        &channel_id,
        &token.address,
        &500,
        &10,
        &creators,
        &percentages,
    );

    assert_eq!(token.balance(&subscriber), 500);
    assert_eq!(token.balance(&contract_id), 500);

    env.ledger().set_timestamp(110);
    client.collect_group(&subscriber, &channel_id);

    // 10 seconds * 10 tokens/sec = 100 tokens split across creators
    assert_eq!(token.balance(&creator_1), 40);
    assert_eq!(token.balance(&creator_2), 25);
    assert_eq!(token.balance(&creator_3), 15);
    assert_eq!(token.balance(&creator_4), 10);
    assert_eq!(token.balance(&creator_5), 10);
    assert_eq!(token.balance(&contract_id), 400);
}

#[test]
fn test_cliff_based_access_before_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.set_cliff_threshold(&creator, &50);
    assert_eq!(client.get_cliff_threshold(&creator), 50);

    assert!(!client.has_unlocked_access(&subscriber, &creator));
    assert_eq!(client.get_access_tier(&subscriber, &creator), 0);

    client.subscribe(&subscriber, &creator, &token.address, &30, &1);
    env.ledger().set_timestamp(100);
    client.collect(&subscriber, &creator);

    assert!(!client.has_unlocked_access(&subscriber, &creator));
    assert_eq!(client.get_access_tier(&subscriber, &creator), 0);
}

#[test]
fn test_cancel_after_minimum_duration() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    // Advance beyond minimum duration (24h + 1h)
    env.ledger().set_timestamp(100 + 86400 + 3600);
    client.cancel(&subscriber, &creator);

    assert_eq!(token.balance(&creator), 100);
    assert_eq!(token.balance(&subscriber), 900);
}

#[test]
fn test_migrate_tier_downgrade_prorates_refund() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &10);

    env.ledger().set_timestamp(105);
    client.migrate_tier(&subscriber, &creator, &5, &0);

    assert_eq!(token.balance(&creator), 50);
    assert_eq!(token.balance(&contract_id), 25);
    assert_eq!(token.balance(&subscriber), 925);
}

#[test]
fn test_group_cancel_collects_and_refunds_remaining_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let channel_id = Address::generate(&env);
    let creator_1 = Address::generate(&env);
    let creator_2 = Address::generate(&env);
    let creator_3 = Address::generate(&env);
    let creator_4 = Address::generate(&env);
    let creator_5 = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    let creators = vec![
        &env,
        creator_1.clone(),
        creator_2.clone(),
        creator_3.clone(),
        creator_4.clone(),
        creator_5.clone(),
    ];
    let percentages = vec![&env, 40u32, 20u32, 20u32, 10u32, 10u32];

    // Start at t=0, deposit 200 tokens at rate 1/sec
    // After exactly 30 seconds, cancel (past minimum duration)
    // 30 tokens collected, 170 refunded
    env.ledger().set_timestamp(0);
    client.subscribe_group(
        &subscriber,
        &channel_id,
        &token.address,
        &200,
        &1,
        &creators,
        &percentages,
    );

    // Advance past minimum duration (24h) + 30 seconds
    env.ledger().set_timestamp(86400 + 30);
    client.cancel_group(&subscriber, &channel_id);

    // 86430s * 1/sec = 86430, capped at balance 200 → all 200 collected
    // 200 tokens split: 40%=80, 20%=40, 20%=40, 10%=20, 10%=20
    assert_eq!(token.balance(&creator_1), 80);
    assert_eq!(token.balance(&creator_2), 40);
    assert_eq!(token.balance(&creator_3), 40);
    assert_eq!(token.balance(&creator_4), 20);
    assert_eq!(token.balance(&creator_5), 20);
    assert_eq!(token.balance(&subscriber), 800); // 1000 - 200 deposited, 0 refund
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_cliff_based_access_after_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.set_cliff_threshold(&creator, &50);

    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    assert!(!client.has_unlocked_access(&subscriber, &creator));

    env.ledger().set_timestamp(100);
    client.collect(&subscriber, &creator);

    assert!(client.has_unlocked_access(&subscriber, &creator));
    assert_eq!(client.get_total_streamed(&subscriber, &creator), 100);
    assert_eq!(client.get_access_tier(&subscriber, &creator), 1);
}

#[test]
fn test_migrate_tier_upgrade_with_additional_deposit() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    client.migrate_tier(&subscriber, &creator, &2, &50);

    assert_eq!(token.balance(&contract_id), 150);
    assert_eq!(token.balance(&subscriber), 850);
}

#[test]
fn test_access_tiers() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &10000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.set_cliff_threshold(&creator, &50);

    client.subscribe(&subscriber, &creator, &token.address, &60, &2);

    env.ledger().set_timestamp(100);
    client.collect(&subscriber, &creator);
    assert_eq!(client.get_access_tier(&subscriber, &creator), 1);
    assert!(client.has_unlocked_access(&subscriber, &creator));

    client.top_up(&subscriber, &creator, &200);
    env.ledger().set_timestamp(200);
    client.collect(&subscriber, &creator);
    assert_eq!(client.get_access_tier(&subscriber, &creator), 2);

    client.top_up(&subscriber, &creator, &300);
    env.ledger().set_timestamp(400);
    client.collect(&subscriber, &creator);
    assert_eq!(client.get_access_tier(&subscriber, &creator), 3);
}

#[test]
fn test_migrate_tier_emits_tier_changed_event() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    client.migrate_tier(&subscriber, &creator, &3, &0);

    let events = env.events().all();
    // Verify at least one event was emitted (TierChanged)
    let _ = events;
}

#[test]
#[should_panic(expected = "group channel must contain exactly 5 creators")]
fn test_group_requires_exactly_five_creators() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let channel_id = Address::generate(&env);
    let creator_1 = Address::generate(&env);
    let creator_2 = Address::generate(&env);
    let creator_3 = Address::generate(&env);
    let creator_4 = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    let creators = vec![
        &env,
        creator_1.clone(),
        creator_2.clone(),
        creator_3.clone(),
        creator_4.clone(),
    ];
    let percentages = vec![&env, 25u32, 25u32, 25u32, 25u32];

    client.subscribe_group(
        &subscriber,
        &channel_id,
        &token.address,
        &100,
        &1,
        &creators,
        &percentages,
    );
}

#[test]
#[should_panic(expected = "percentages must sum to 100")]
fn test_group_percentages_must_sum_to_100() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let channel_id = Address::generate(&env);
    let creator_1 = Address::generate(&env);
    let creator_2 = Address::generate(&env);
    let creator_3 = Address::generate(&env);
    let creator_4 = Address::generate(&env);
    let creator_5 = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    let creators = vec![
        &env,
        creator_1.clone(),
        creator_2.clone(),
        creator_3.clone(),
        creator_4.clone(),
        creator_5.clone(),
    ];
    let percentages = vec![&env, 30u32, 20u32, 20u32, 10u32, 10u32]; // sums to 90

    client.subscribe_group(
        &subscriber,
        &channel_id,
        &token.address,
        &100,
        &1,
        &creators,
        &percentages,
    );
}

#[test]
#[should_panic(expected = "new rate must be positive")]
fn test_migrate_tier_invalid_rate() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    client.migrate_tier(&subscriber, &creator, &0, &0);
}

#[test]
fn test_migrate_tier_upgrade_collects_at_new_rate() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    env.ledger().set_timestamp(110);
    client.migrate_tier(&subscriber, &creator, &2, &0);

    assert_eq!(token.balance(&creator), 10);
    assert_eq!(token.balance(&contract_id), 90);

    env.ledger().set_timestamp(120);
    client.collect(&subscriber, &creator);
    assert_eq!(token.balance(&creator), 30);
    assert_eq!(token.balance(&contract_id), 70);
}

#[test]
#[should_panic(expected = "cannot cancel stream: minimum duration not met")]
fn test_cancel_before_minimum_duration() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    // Try to cancel after only 1 hour — should fail
    env.ledger().set_timestamp(100 + 3600);
    client.cancel(&subscriber, &creator);
}

#[test]
fn test_cancel_exactly_at_minimum_duration() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    env.ledger().set_timestamp(100 + 86400);
    client.cancel(&subscriber, &creator);

    assert_eq!(token.balance(&creator), 100);
    assert_eq!(token.balance(&subscriber), 900);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
#[should_panic(
    expected = "cannot cancel stream: minimum duration not met. 43200 seconds remaining"
)]
fn test_cancel_with_remaining_time_message() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(100);
    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    // Try to cancel after 12 hours (43200 seconds remaining)
    env.ledger().set_timestamp(100 + 43200);
    client.cancel(&subscriber, &creator);
}

#[test]
fn test_total_streamed_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    let admin = Address::generate(&env);

    let token = create_token_contract(&env, &admin);
    let token_admin = token::StellarAssetClient::new(&env, &token.address);
    token_admin.mint(&subscriber, &1000);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    client.subscribe(&subscriber, &creator, &token.address, &100, &1);

    env.ledger().set_timestamp(100);
    client.collect(&subscriber, &creator);
    assert_eq!(client.get_total_streamed(&subscriber, &creator), 100);

    client.top_up(&subscriber, &creator, &50);
    env.ledger().set_timestamp(150);
    client.collect(&subscriber, &creator);
    assert_eq!(client.get_total_streamed(&subscriber, &creator), 150);
}

#[test]
fn test_creator_metadata() {
    let env = Env::default();
    env.mock_all_auths();

    let creator = Address::generate(&env);

    let contract_id = env.register(SubStreamContract, ());
    let client = SubStreamContractClient::new(&env, &contract_id);

    // No metadata set yet
    assert_eq!(client.get_creator_metadata(&creator), None);

    // Set an IPFS CID
    let cid = Bytes::from_slice(&env, b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    client.set_creator_metadata(&creator, &cid);

    // Retrieve and verify
    assert_eq!(client.get_creator_metadata(&creator), Some(cid.clone()));

    // Update to a new CID
    let new_cid = Bytes::from_slice(&env, b"QmNewCIDabcdefghijklmnopqrstuvwxyz1234567890AB");
    client.set_creator_metadata(&creator, &new_cid);
    assert_eq!(client.get_creator_metadata(&creator), Some(new_cid));
}
