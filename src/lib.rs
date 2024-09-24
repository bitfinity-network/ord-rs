//! # ord-rs
//!
//! A library for working with Ordinal inscriptions.
//!
//! This library provides a set of tools for working with Ordinal inscriptions, including creating, parsing, and signing transactions.
//! It allows you to work with both BRC20, runes and generic inscriptions.
//!
//! # Get started
//!
//! Add the following to your `Cargo.toml`:
//!
//! ```toml
//! ord-rs = "0.3.0"
//! ```
//!
//! In case you want to enable runes support, you can add the following feature:
//!
//! ```toml
//! ord-rs = { version = "0.3.0", features = ["rune"] }
//! ```
//!
//! ## Example
//!
//! An example for creating a BRC20 inscription:
//!
//! ```rust
//! use bitcoin::secp256k1::Secp256k1;
//! use bitcoin::{Address, Amount, FeeRate, Network, PrivateKey, Txid};
//! use ord_rs::wallet::{
//!     CreateCommitTransactionArgs, RevealTransactionArgs, SignCommitTransactionArgs,
//! };
//! use ord_rs::{Brc20, OrdTransactionBuilder, Utxo};
//!
//! use std::str::FromStr;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//!     let network = Network::Testnet;
//!     let ticker = "ordi".to_string();
//!     let amount = 1_000;
//!
//!     let private_key = PrivateKey::from_wif("cVkWbHmoCx6jS8AyPNQqvFr8V9r2qzDHJLaxGDQgDJfxT73w6fuU")?;
//!     let public_key = private_key.public_key(&Secp256k1::new());
//!     let sender_address = Address::p2wpkh(&public_key, network).unwrap();
//!
//!     let mut builder = OrdTransactionBuilder::p2tr(private_key);
//!
//!     let inputs = vec![Utxo {
//!         id: Txid::from_str("791b415dc6946d864d368a0e5ec5c09ee2ad39cf298bc6e3f9aec293732cfda7")
//!             .unwrap(), // the transaction that funded our wallet
//!         index: 1,
//!         amount: Amount::from_sat(8_000),
//!     }];
//!
//!     let commit_tx = builder
//!     .build_commit_transaction(
//!         network,
//!         sender_address.clone(),
//!         CreateCommitTransactionArgs {
//!             fee_rate: FeeRate::from_sat_per_vb(1).unwrap(),
//!             inputs: inputs.clone(),
//!             inscription: Brc20::transfer(ticker, amount),
//!             txin_script_pubkey: sender_address.script_pubkey(),
//!             leftovers_recipient: sender_address.clone(),
//!             derivation_path: None,
//!             multisig_config: None,
//!         },
//!     )
//!     .await?;
//!
//!     let signed_commit_tx = builder
//!     .sign_commit_transaction(
//!         commit_tx.unsigned_tx,
//!         SignCommitTransactionArgs {
//!             inputs,
//!             txin_script_pubkey: sender_address.script_pubkey(),
//!             derivation_path: None,
//!         },
//!     )
//!     .await?;
//!
//!     let commit_txid = signed_commit_tx.txid();
//!     // TODO: send commit_tx to the network
//!
//!     let reveal_transaction = builder
//!         .build_reveal_transaction(RevealTransactionArgs {
//!             input: ord_rs::wallet::Utxo {
//!                 id: commit_txid,
//!                 index: 0,
//!                 amount: commit_tx.reveal_balance,
//!             },
//!             recipient_address: sender_address, // NOTE: it's correct, see README.md to read about how transfer works
//!             redeem_script: commit_tx.redeem_script,
//!             derivation_path: None,
//!         })
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//!

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

pub use bitcoin;
pub use error::{InscriptionParseError, OrdError};
pub use inscription::brc20::Brc20;
pub use inscription::iid::InscriptionId;
pub use inscription::nft::Nft;
pub use inscription::Inscription;
pub use result::OrdResult;
pub use utils::fees::{self, MultisigConfig};
pub use utils::{constants, push_bytes};
pub use wallet::{
    BtcTxSigner, CreateCommitTransaction, CreateCommitTransactionArgs, OrdParser,
    OrdTransactionBuilder, RevealTransactionArgs, SignCommitTransactionArgs, Utxo, Wallet,
};

mod error;
pub mod inscription;
mod result;
mod utils;
pub mod wallet;
