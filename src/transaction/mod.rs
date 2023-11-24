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

    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::{Address, Amount, Network, Sequence, Txid};
    use hex_literal::hex;

    use super::*;
    use crate::utils::test_utils::generate_btc_address;
    use crate::Brc20Op;

    const WIF: &str = "cVkWbHmoCx6jS8AyPNQqvFr8V9r2qzDHJLaxGDQgDJfxT73w6fuU";

    #[test]
    fn test_should_build_deploy_transactions_from_existing_data() {
        // this test refers to this testnet transaction:
        // <https://mempool.space/testnet/tx/https://mempool.space/testnet/tx/a2153d0c0efba1b8499fdeb61b86a768034c3541d6056754e23a44ce4a03a883>
        // made by address tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark
        let private_key = PrivateKey::from_wif(WIF).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());
        let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        let builder = Brc20TransactionBuilder::new(private_key);

        let commit_transaction_args = CreateCommitTransactionArgs {
            inputs: vec![(
                Txid::from_str("a2153d0c0efba1b8499fdeb61b86a768034c3541d6056754e23a44ce4a03a883")
                    .unwrap(), // the transaction that funded our walle
                0,
            )], // the index of the input that funds the transaction
            input_balance_msat: 8_000,
            inscription: Brc20Op::deploy("mona".to_string(), 21_000_000, Some(1_000), None),
            leftovers_recipient: address.clone(),
            commit_fee: 2307,
            reveal_fee: 4667,
        };
        let tx_result = builder
            .build_commit_transaction(commit_transaction_args)
            .unwrap();

        println!("tx_result: {:?}", tx_result);

        let witness = tx_result.tx.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 2);
        //for w in witness.to_vec() {
        //    println!("{}", hex::encode(w));
        //}
        // assert_eq!(witness[0], hex!("3045022100f351dbd93f0c58cbdd4475515c646324bf2ec04098727e39ee57ac8b6e39564b022059b8b88b6e159471efe9fb199cf7afeb4a8c8396c0c8f63208197b2d11c58ea401"));
        assert_eq!(
            witness[1],
            hex!("02d1c2aebced475b0c672beb0336baa775a44141263ee82051b5e57ad0f2248240")
        );

        // txin
        assert_eq!(tx_result.tx.input.len(), 1);
        assert_eq!(
            tx_result.tx.input[0].sequence,
            Sequence::from_consensus(0xffffffff)
        );
        assert_eq!(
            tx_result.tx.input[0].previous_output.txid,
            Txid::from_str("a2153d0c0efba1b8499fdeb61b86a768034c3541d6056754e23a44ce4a03a883",)
                .unwrap()
        );

        // txout
        assert_eq!(tx_result.tx.output.len(), 2);
        assert_eq!(tx_result.tx.output[0].value, Amount::from_sat(5_000));
        assert_eq!(tx_result.tx.output[1].value, Amount::from_sat(693));

        println!("{}", tx_result.redeem_script);

        println!("\n\n\n\n\n\n");
    }

    #[test]
    fn test_should_build_reveal_transaction() {
        let (address, privkey) = generate_btc_address(Network::Bitcoin);

        let builder = Brc20TransactionBuilder::new(privkey);

        let reveal_fee = 7_000;

        let commit_tx = builder
            .build_commit_transaction(CreateCommitTransactionArgs {
                inputs: vec![(
                    Txid::from_str(
                        "5b3cf3573442df94895dfdef2509a6bc38c245bb9c403c9879933bb4c47452b1",
                    )
                    .unwrap(),
                    0,
                )],
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
