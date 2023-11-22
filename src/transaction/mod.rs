const POSTAGE: u64 = 333;

mod commit_transaction;
mod reveal_transaction;

pub use commit_transaction::{
    create_commit_transaction, CreateCommitTransaction, CreateCommitTransactionArgs,
};
