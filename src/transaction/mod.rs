const POSTAGE: u64 = 333;

mod commit_transaction;
mod reveal_transaction;
mod signature;

use bitcoin::{PrivateKey, Transaction};
use commit_transaction::create_commit_transaction;
pub use commit_transaction::{CreateCommitTransaction, CreateCommitTransactionArgs};
use reveal_transaction::create_reveal_transaction;
pub use reveal_transaction::RevealTransactionArgs;

use crate::Brc20Result;

/// Builder for BRC20 transactions
pub struct Brc20TransactionBuilder {
    private_key: PrivateKey,
}

impl Brc20TransactionBuilder {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }

    /// Create the commit transaction
    pub fn build_commit_transaction(
        &self,
        args: CreateCommitTransactionArgs,
    ) -> Brc20Result<CreateCommitTransaction> {
        create_commit_transaction(&self.private_key, args)
    }

    /// Create the reveal transaction
    pub fn build_reveal_transaction(
        &self,
        args: RevealTransactionArgs,
    ) -> Brc20Result<Transaction> {
        create_reveal_transaction(&self.private_key, args)
    }
}

impl From<PrivateKey> for Brc20TransactionBuilder {
    fn from(private_key: PrivateKey) -> Self {
        Self::new(private_key)
    }
}
