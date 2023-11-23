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

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use bitcoin::{Amount, Network, Txid};

    use super::*;
    use crate::utils::test_utils::generate_btc_address;
    use crate::Brc20Op;

    #[test]
    fn test_should_build_commit_transaction() {
        let (address, privkey) = generate_btc_address(Network::Bitcoin);

        let builder = Brc20TransactionBuilder::new(privkey);

        let tx_result = builder
            .build_commit_transaction(CreateCommitTransactionArgs {
                input_tx: Txid::from_str(
                    "5b3cf3573442df94895dfdef2509a6bc38c245bb9c403c9879933bb4c47452b1",
                )
                .unwrap(),
                input_index: 0,
                input_balance_msat: 100_000,
                inscription: Brc20Op::deploy("ordi".to_string(), 21_000_000, Some(100_000), None),
                leftovers_recipient: address,
                commit_fee: 15_000,
                reveal_fee: 7_000,
            })
            .unwrap();

        assert_eq!(tx_result.tx.input.len(), 1);
        assert_eq!(tx_result.tx.output.len(), 2);
        assert_eq!(
            tx_result.tx.output[0].value,
            Amount::from_sat(POSTAGE + 7_000)
        );
        assert_eq!(
            tx_result.tx.output[1].value,
            Amount::from_sat(100_000 - 15_000 - 7_000 - POSTAGE)
        );
    }

    #[test]
    fn test_should_build_reveal_trnsaction() {
        let (address, privkey) = generate_btc_address(Network::Bitcoin);

        let builder = Brc20TransactionBuilder::new(privkey);

        let reveal_fee = 7_000;

        let commit_tx = builder
            .build_commit_transaction(CreateCommitTransactionArgs {
                input_tx: Txid::from_str(
                    "5b3cf3573442df94895dfdef2509a6bc38c245bb9c403c9879933bb4c47452b1",
                )
                .unwrap(),
                input_index: 0,
                input_balance_msat: 100_000,
                inscription: Brc20Op::deploy("ordi".to_string(), 21_000_000, Some(100_000), None),
                leftovers_recipient: address.clone(),
                commit_fee: 15_000,
                reveal_fee,
            })
            .unwrap();

        let reveal_tx = builder
            .build_reveal_transaction(RevealTransactionArgs {
                input_tx: Txid::from_str(
                    "afe019fb1556e7eb1626ba85fa92fb90b2ee9769f6d01647454bc389d4431b6f",
                )
                .unwrap(),
                input_index: 0,
                input_balance_sats: reveal_fee + POSTAGE,
                recipient_address: address,
                redeem_script: commit_tx.redeem_script.clone(),
            })
            .unwrap();

        assert_eq!(reveal_tx.input.len(), 1);
        assert_eq!(reveal_tx.output.len(), 1);
        assert_eq!(reveal_tx.output[0].value, Amount::from_sat(POSTAGE));
    }
}
