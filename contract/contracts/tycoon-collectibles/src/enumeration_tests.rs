#![cfg(test)]
//! SW-CT-ENM-001: Targeted coverage tests for tycoon-collectibles enumeration.rs
//!
//! Covers every public function and edge-case branch in `enumeration.rs`:
//!   - `_add_token_to_enumeration` – idempotent on duplicate
//!   - `_remove_token_from_enumeration` – token not in list is a no-op
//!   - `get_owned_tokens` – returns correct set
//!   - `owned_token_count` – reflects additions and removals
//!   - `token_of_owner_by_index` – in-bounds and out-of-bounds
//!   - `tokens_of_owner_page` – happy path, empty result, zero/large page_size
//!   - `iterate_owned_tokens` – happy path, has_more flag, out-of-bounds start
//!   - `MAX_PAGE_SIZE` – exposed via the contract entrypoint

extern crate std;

use crate::{TycoonCollectibles, TycoonCollectiblesClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (TycoonCollectiblesClient<'_>, Address) {
    let contract_id = env.register(TycoonCollectibles, ());
    let client = TycoonCollectiblesClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (client, admin)
}

// ── MAX_PAGE_SIZE ─────────────────────────────────────────────────────────────

#[test]
fn test_max_page_size_is_100() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    assert_eq!(client.max_page_size(), 100);
}

// ── owned_token_count ─────────────────────────────────────────────────────────

#[test]
fn test_owned_token_count_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);
    assert_eq!(client.owned_token_count(&user), 0);
}

#[test]
fn test_owned_token_count_increments_on_mint() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &5);
    assert_eq!(client.owned_token_count(&user), 1);

    client.buy_collectible(&user, &2, &1);
    assert_eq!(client.owned_token_count(&user), 2);
}

#[test]
fn test_owned_token_count_decrements_on_full_burn() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &3);
    client.buy_collectible(&user, &2, &1);
    assert_eq!(client.owned_token_count(&user), 2);

    // Burn all of token 1
    client.burn(&user, &1, &3);
    assert_eq!(client.owned_token_count(&user), 1);

    // Burn all of token 2
    client.burn(&user, &2, &1);
    assert_eq!(client.owned_token_count(&user), 0);
}

#[test]
fn test_owned_token_count_stable_on_partial_burn() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &10, &5);
    // Partial burn: still owns the token type
    client.burn(&user, &10, &2);
    assert_eq!(client.owned_token_count(&user), 1);
}

// ── get_owned_tokens (via tokens_of) ─────────────────────────────────────────

#[test]
fn test_get_owned_tokens_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);
    let tokens = client.tokens_of(&user);
    assert_eq!(tokens.len(), 0);
}

#[test]
fn test_get_owned_tokens_reflects_mint_order() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &5, &1);
    client.buy_collectible(&user, &10, &1);
    client.buy_collectible(&user, &15, &1);

    let tokens = client.tokens_of(&user);
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens.get(0).unwrap(), 5);
    assert_eq!(tokens.get(1).unwrap(), 10);
    assert_eq!(tokens.get(2).unwrap(), 15);
}

// ── token_of_owner_by_index ───────────────────────────────────────────────────

#[test]
fn test_token_of_owner_by_index_returns_correct_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &100, &1);
    client.buy_collectible(&user, &200, &1);

    assert_eq!(client.token_of_owner_by_index(&user, &0), 100);
    assert_eq!(client.token_of_owner_by_index(&user, &1), 200);
}

#[test]
#[should_panic]
fn test_token_of_owner_by_index_panics_out_of_bounds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);
    // Index 5 is out of bounds for a list of 1
    client.token_of_owner_by_index(&user, &5);
}

// ── swap-remove correctness after burn ───────────────────────────────────────

#[test]
fn test_swap_remove_maintains_all_remaining_tokens() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    // Buy 4 tokens: indices 0=100, 1=200, 2=300, 3=400
    client.buy_collectible(&user, &100, &1);
    client.buy_collectible(&user, &200, &1);
    client.buy_collectible(&user, &300, &1);
    client.buy_collectible(&user, &400, &1);

    // Remove token at index 1 (200); last (400) should swap to index 1
    client.burn(&user, &200, &1);

    let tokens = client.tokens_of(&user);
    assert_eq!(tokens.len(), 3);

    // All remaining token IDs must still be present
    let mut found = [false; 3];
    for i in 0..3u32 {
        let id = tokens.get(i).unwrap();
        if id == 100 {
            found[0] = true;
        }
        if id == 300 {
            found[1] = true;
        }
        if id == 400 {
            found[2] = true;
        }
    }
    assert!(found[0], "token 100 should still be present");
    assert!(found[1], "token 300 should still be present");
    assert!(found[2], "token 400 should still be present");
}

#[test]
fn test_burn_last_element_does_not_swap() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);
    client.buy_collectible(&user, &2, &1);
    client.buy_collectible(&user, &3, &1);

    // Remove the last token (index 2 = token 3)
    client.burn(&user, &3, &1);

    let tokens = client.tokens_of(&user);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens.get(0).unwrap(), 1);
    assert_eq!(tokens.get(1).unwrap(), 2);
}

#[test]
fn test_burn_only_token_leaves_empty_list() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &42, &1);
    client.burn(&user, &42, &1);

    assert_eq!(client.owned_token_count(&user), 0);
    assert_eq!(client.tokens_of(&user).len(), 0);
}

// ── duplicate add is a no-op ──────────────────────────────────────────────────

#[test]
fn test_buying_more_of_existing_token_does_not_duplicate_in_list() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &7, &1);
    client.buy_collectible(&user, &7, &4); // adding more of same token
    // Token 7 should appear exactly once in the enumeration list
    assert_eq!(client.owned_token_count(&user), 1);
    assert_eq!(client.balance_of(&user, &7), 5);
}

// ── tokens_of_owner_page ─────────────────────────────────────────────────────

#[test]
fn test_tokens_of_owner_page_first_page() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    for i in 1u128..=5 {
        client.buy_collectible(&user, &i, &1);
    }

    let page = client.tokens_of_owner_page(&user, &0, &3);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap(), 1);
    assert_eq!(page.get(1).unwrap(), 2);
    assert_eq!(page.get(2).unwrap(), 3);
}

#[test]
fn test_tokens_of_owner_page_second_page() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    for i in 1u128..=5 {
        client.buy_collectible(&user, &i, &1);
    }

    let page = client.tokens_of_owner_page(&user, &1, &3);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap(), 4);
    assert_eq!(page.get(1).unwrap(), 5);
}

#[test]
fn test_tokens_of_owner_page_out_of_bounds_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    // Page 10 with page_size 3 — well beyond single token
    let page = client.tokens_of_owner_page(&user, &10, &3);
    assert_eq!(page.len(), 0);
}

#[test]
fn test_tokens_of_owner_page_zero_page_size_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    let result = client.try_tokens_of_owner_page(&user, &0, &0);
    assert!(result.is_err(), "page_size=0 must return InvalidPageSize");
}

#[test]
fn test_tokens_of_owner_page_exceeds_max_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    // page_size > MAX_PAGE_SIZE (100) must be rejected
    let result = client.try_tokens_of_owner_page(&user, &0, &101);
    assert!(result.is_err(), "page_size > MAX_PAGE_SIZE must return error");
}

#[test]
fn test_tokens_of_owner_page_exact_max_page_size_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    let result = client.try_tokens_of_owner_page(&user, &0, &100);
    assert!(result.is_ok(), "page_size == MAX_PAGE_SIZE must be allowed");
}

#[test]
fn test_tokens_of_owner_page_no_tokens_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    let page = client.tokens_of_owner_page(&user, &0, &10);
    assert_eq!(page.len(), 0);
}

// ── iterate_owned_tokens ──────────────────────────────────────────────────────

#[test]
fn test_iterate_owned_tokens_first_batch_has_more() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    for i in 1u128..=5 {
        client.buy_collectible(&user, &i, &1);
    }

    let (batch, has_more) = client.iterate_owned_tokens(&user, &0, &3);
    assert_eq!(batch.len(), 3);
    assert!(has_more, "should indicate more tokens remain");
}

#[test]
fn test_iterate_owned_tokens_last_batch_no_more() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    for i in 1u128..=5 {
        client.buy_collectible(&user, &i, &1);
    }

    let (batch, has_more) = client.iterate_owned_tokens(&user, &3, &3);
    assert_eq!(batch.len(), 2);
    assert!(!has_more, "last batch should have no more");
}

#[test]
fn test_iterate_owned_tokens_start_out_of_bounds_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    let (batch, has_more) = client.iterate_owned_tokens(&user, &100, &10);
    assert_eq!(batch.len(), 0);
    assert!(!has_more);
}

#[test]
fn test_iterate_owned_tokens_zero_batch_size_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    let result = client.try_iterate_owned_tokens(&user, &0, &0);
    assert!(result.is_err(), "batch_size=0 must return InvalidPageSize");
}

#[test]
fn test_iterate_owned_tokens_exceeds_max_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &1, &1);

    let result = client.try_iterate_owned_tokens(&user, &0, &101);
    assert!(
        result.is_err(),
        "batch_size > MAX_PAGE_SIZE must return error"
    );
}

#[test]
fn test_iterate_owned_tokens_exact_batch_fits() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let user = Address::generate(&env);

    client.buy_collectible(&user, &10, &1);
    client.buy_collectible(&user, &20, &1);

    let (batch, has_more) = client.iterate_owned_tokens(&user, &0, &2);
    assert_eq!(batch.len(), 2);
    assert!(!has_more, "exact fit: no more tokens");
}

// ── enumeration consistency after transfer ────────────────────────────────────

#[test]
fn test_enumeration_consistent_after_transfer_removes_from_sender() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.buy_collectible(&alice, &1, &1);
    client.buy_collectible(&alice, &2, &1);

    client.transfer(&alice, &bob, &1, &1);

    assert_eq!(client.owned_token_count(&alice), 1);
    assert_eq!(client.owned_token_count(&bob), 1);

    let alice_tokens = client.tokens_of(&alice);
    assert_eq!(alice_tokens.get(0).unwrap(), 2);

    let bob_tokens = client.tokens_of(&bob);
    assert_eq!(bob_tokens.get(0).unwrap(), 1);
}

#[test]
fn test_transfer_partial_balance_keeps_token_in_sender_enumeration() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.buy_collectible(&alice, &99, &10);
    client.transfer(&alice, &bob, &99, &4);

    // Alice still holds 6, token stays in her list
    assert_eq!(client.owned_token_count(&alice), 1);
    assert_eq!(client.balance_of(&alice, &99), 6);
    // Bob now owns it
    assert_eq!(client.owned_token_count(&bob), 1);
}
