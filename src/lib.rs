//! # ord-rs
//!
//! # Get started
//!
//! INSERT TEXT HERE
//!
//! ## Example
//!
//! ```rust
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Ok(())
//! }
//! ```
//!

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

mod error;
pub mod inscription;
mod result;
mod utils;
pub mod wallet;

pub use bitcoin;
pub use error::{InscriptionParseError, OrdError};
pub use inscription::brc20::Brc20;
pub use inscription::nft::Nft;
pub use inscription::Inscription;
pub use result::OrdResult;
pub use utils::fees::{self, MultisigConfig};
pub use wallet::{
    CreateCommitTransaction, CreateCommitTransactionArgs, ExternalSigner, OrdParser,
    OrdTransactionBuilder, RevealTransactionArgs, SignCommitTransactionArgs, Utxo, Wallet,
    WalletType,
};
