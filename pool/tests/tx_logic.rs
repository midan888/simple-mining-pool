// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! These are tests concerning the business logic of transactions in the cryptocurrency service.
//! See `api.rs` for tests focused on the API of the service.
//!
//! Note how business logic tests use `TestKit::create_block*` methods to send transactions,
//! the service schema to make assertions about the storage state.

extern crate exonum;
extern crate simple_mining_pool;
#[macro_use]
extern crate exonum_testkit;
extern crate rand;

use exonum::blockchain::Transaction;
use exonum::crypto::{self, Hash, PublicKey, SecretKey};
use exonum_testkit::{TestKit, TestKitBuilder};

// Import data types used in tests from the crate where the service is defined.
use simple_mining_pool::CoreSchema;
use simple_mining_pool::CoreService;
use simple_mining_pool::transactions::{CreateAccount, Transfer, SignIn};
use simple_mining_pool::account::Account;

// Imports shared test constants.
use constants::{ALICE_NAME, BOB_NAME};

mod constants;

#[test]
fn test_create_account() {
    let mut testkit = init_testkit();
    let (tx, _) = create_account(&mut testkit, ALICE_NAME);

    // Check that the user indeed is persisted by the service
    let account = get_account(&testkit, tx.pub_key());
    assert_eq!(account.pub_key(), tx.pub_key());
    assert_eq!(account.name(), ALICE_NAME);
    assert_eq!(account.balance(), 100);
}

///test is created in analogy to respective test for api
#[test]
fn test_sing_into_account() {
    let mut testkit = init_testkit();
    let (tx, key) = create_account(&mut testkit, ALICE_NAME);

    // Check that the user indeed is persisted by the service
    let account = get_account(&testkit, tx.pub_key());
    assert_eq!(account.pub_key(), tx.pub_key());
    assert_eq!(account.name(), ALICE_NAME);
    assert_eq!(account.balance(), 100);
    assert_eq!(account.access_token_hash(), &Hash::default());

    // Create and send a transaction via API
    let pubkey = account.pub_key();
    let tx = sign_into_account(&mut testkit, &pubkey, ALICE_NAME, &key);

    // Check that the user indeed is persisted by the service.
    let account = get_account(&testkit,tx.pub_key());
    assert_eq!(account.pub_key(), tx.pub_key());
    assert_eq!(account.name(), tx.name());
    assert_ne!(account.access_token_hash(), &Hash::default());
}

#[test]
fn test_transfer() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&alice_pubkey, ALICE_NAME, &alice_key),
        CreateAccount::new(&bob_pubkey, BOB_NAME, &bob_key),
        Transfer::new(
            &alice_pubkey,
            &bob_pubkey,
            /* amount */ 10,
            /* seed */ 0,
            &alice_key
        ),
    ]);

    let alice_account = get_account(&testkit, &alice_pubkey);
    assert_eq!(alice_account.balance(), 90);

    let bob_account = get_account(&testkit, &bob_pubkey);
    assert_eq!(bob_account.balance(), 110);
}

#[test]
fn test_transfer_from_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&bob_pubkey, BOB_NAME, &bob_key),
        Transfer::new(
            &alice_pubkey,
            &bob_pubkey,
            /* amount */ 10,
            /* seed */ 0,
            &alice_key
        ),
    ]);

    assert!(try_get_account(&testkit, &alice_pubkey).is_none());

    let bob_account = get_account(&testkit, &bob_pubkey);
    assert_eq!(bob_account.balance(), 100);
}

#[test]
fn test_transfer_to_nonexisting_wallet() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&alice_pubkey, ALICE_NAME, &alice_key),
        Transfer::new(
            &alice_pubkey,
            &bob_pubkey,
            /* amount */ 10,
            /* seed */ 0,
            &alice_key
        ),
        // Although Bob's wallet is created, this occurs after the transfer is executed.
        CreateAccount::new(&bob_pubkey, BOB_NAME, &bob_key),
    ]);

    let alice_account = get_account(&testkit, &alice_pubkey);
    assert_eq!(alice_account.balance(), 100);

    let bob_account = get_account(&testkit, &bob_pubkey);
    assert_eq!(bob_account.balance(), 100);
}

#[test]
fn test_transfer_overcharge() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&alice_pubkey, ALICE_NAME, &alice_key),
        CreateAccount::new(&bob_pubkey, BOB_NAME, &bob_key),
        Transfer::new(
            &alice_pubkey,
            &bob_pubkey,
            /* amount */ 150,
            /* seed */ 0,
            &alice_key
        ),
    ]);

    // The transfer amount is greater than what Alice has at her disposal, so
    // the transfer should fail.
    let alice_account = get_account(&testkit, &alice_pubkey);
    assert_eq!(alice_account.balance(), 100);

    let bob_account    = get_account(&testkit, &bob_pubkey);
    assert_eq!(bob_account.balance(), 100);
}

#[test]
fn test_transfers_in_single_block() {
    let mut testkit = init_testkit();
    let (alice_pubkey, alice_key) = crypto::gen_keypair();
    let (bob_pubkey, bob_key) = crypto::gen_keypair();
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&alice_pubkey, ALICE_NAME, &alice_key),
        CreateAccount::new(&bob_pubkey, BOB_NAME, &bob_key),
    ]);

    let tx_a_to_b = Transfer::new(
        &alice_pubkey,
        &bob_pubkey,
        90, // amount
        0,  // seed
        &alice_key,
    );
    let tx_b_to_a = Transfer::new(
        &bob_pubkey,
        &alice_pubkey,
        120, // amount
        0,   // seed
        &bob_key,
    );

    {
        // See what happens if transactions are applied in an "incorrect" order.
        // We use `TestKit::probe_all()` method for this.

        let snapshot = testkit.probe_all(txvec![tx_b_to_a.clone(), tx_a_to_b.clone()]);
        let schema = CoreSchema::new(&snapshot);
        assert_eq!(schema.account(&alice_pubkey).map(|w| w.balance()), Some(10));
        assert_eq!(schema.account(&bob_pubkey).map(|w| w.balance()), Some(190));
    }

    testkit.create_block_with_transactions(txvec![tx_a_to_b, tx_b_to_a]);

    let alice_account = get_account(&testkit, &alice_pubkey);
    assert_eq!(alice_account.balance(), 130);

    let bob_account = get_account(&testkit, &bob_pubkey);
    assert_eq!(bob_account.balance(), 70);
}

/// Generate random transactions to perform [fuzz testing][fuzz] of the service. The service
/// should maintain invariants under all circumstances; e.g., the total amount of tokens
/// in existence should depend only on the number of registered accounts.
///
/// [fuzz]: https://en.wikipedia.org/wiki/Fuzzing
#[test]
#[ignore] //todo fix this test
fn test_fuzz_transfers() {
    use rand::Rng;
    use std::collections::BTreeSet;
    use std::iter::FromIterator;

    const BLOCKS: usize = 50; // number of blocks to create
    const MAX_TRANSACTIONS: usize = 20; // maximum number of transactions in a block

    let mut rng = rand::thread_rng();

    let mut testkit = init_testkit();
    let alice_keys = crypto::gen_keypair();
    let bob_keys = crypto::gen_keypair();
    let keys = &[alice_keys.clone(), bob_keys.clone()];
    testkit.create_block_with_transactions(txvec![
        CreateAccount::new(&alice_keys.0, ALICE_NAME, &alice_keys.1),
        CreateAccount::new(&bob_keys.0, BOB_NAME, &bob_keys.1),
    ]);

    for _ in 0..BLOCKS {
        let n_txs = rng.gen_range(0, MAX_TRANSACTIONS); // number of transactions in the block

        let txs: Vec<Box<Transaction>> = (0..n_txs)
            .map(|_| {
                let (sender, receiver) = (rng.choose(keys).unwrap(), rng.choose(keys).unwrap());
                let amount = rng.gen_range(0, 250);
                Transfer::new(&sender.0, &receiver.0, amount, rng.next_u64(), &sender.1).into()
            })
            .collect();

        testkit.create_block_with_transactions(txs);

        // Test invariants that should be maintained during fuzz testing.
        let schema = CoreSchema::new(testkit.snapshot());
        let accounts = schema.accounts();
        let accounts: Vec<_> = accounts.values().collect();
        // There must be 2 accounts in the storage.
        assert_eq!(accounts.len(), 2);
        // These accounts should belong to Alice and Bob.
        assert_eq!(
            BTreeSet::from_iter(accounts.iter().map(Account::pub_key)),
            BTreeSet::from_iter(vec![&alice_keys.0, &bob_keys.0])
        );
        // The total amount of funds should equal 200, no matter which transactions were executed.
        assert_eq!(accounts.iter().map(|a| a.balance()).sum::<u64>(), 200);
    }
}

/// Initializes testkit with `CurrencyService`.
fn init_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_service(CoreService)
        .create()
}

/// Creates a account with the given name and a random key.
fn create_account(testkit: &mut TestKit, name: &str) -> (CreateAccount, SecretKey) {
    let (pubkey, key) = crypto::gen_keypair();
    let tx = CreateAccount::new(&pubkey, name, &key);
    testkit.create_block_with_transaction(tx.clone());
    (tx, key)
}

/// Returns the account identified by the given public key or `None` such account doesn't exist.
fn try_get_account(testkit: &TestKit, pubkey: &PublicKey) -> Option<Account> {
    let snapshot = testkit.snapshot();
    CoreSchema::new(&snapshot).account(pubkey)
}

/// Returns the account identified by the given public key.
fn get_account(testkit: &TestKit, pubkey: &PublicKey) -> Account {
    try_get_account(testkit, pubkey).expect("No account persisted")
}

/// Create signin transaction given user credentials and private key
fn sign_into_account(testkit: &mut TestKit, pubkey: &PublicKey, name: &str, key: &SecretKey) -> SignIn {
    // Create a pre-signed transaction
    let tx = SignIn::new(&pubkey, name, &key);
    testkit.create_block_with_transaction(tx.clone());
    tx
}