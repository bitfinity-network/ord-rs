use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{Address, Amount, FeeRate, ScriptBuf, Transaction, TxIn, TxOut};
use ordinals::{Edict, RuneId, Runestone};

use crate::fees::estimate_transaction_fees;
use crate::wallet::builder::TxInputInfo;
use crate::wallet::ScriptType;
use crate::{OrdError, OrdTransactionBuilder};

/// Postage amount for rune transaction.
///
/// The value is same as in `ord` tool.
pub const RUNE_POSTAGE: Amount = Amount::from_sat(10_000);

/// Arguments for the [`OrdTransactionBuilder::create_edict_transaction`] method.
pub struct CreateEdictTxArgs {
    /// Identifier of the rune to be transferred.
    rune: RuneId,
    /// Inputs that contain rune and funding BTC balances.
    inputs: Vec<TxInputInfo>,
    /// Address of the recipient of the rune transfer.
    destination: Address,
    /// Address that will receive leftovers of BTC.
    change_address: Address,
    /// Address that will receive leftovers of runes.
    rune_change_address: Address,
    /// Amount of the rune to be transferred.
    amount: u128,
    /// Current BTC fee rate.
    fee_rate: FeeRate,
}

impl CreateEdictTxArgs {
    fn input_amount(&self) -> Amount {
        self.inputs
            .iter()
            .fold(Amount::ZERO, |a, b| a + b.tx_out.value)
    }
}

impl OrdTransactionBuilder {
    /// Creates an unsigned rune edict transaction.
    ///
    /// This method doesn't check the runes balances, so it's the responsibility of the caller to
    /// check that the inputs have enough of the given rune balance to make the transfer. As per
    /// runes standard, if the inputs rune balance is less than specified transfer amount, the
    /// amount will be reduced to the available balance amount.
    ///
    /// # Errors
    /// * Returns [`OrdError::InsufficientBalance`] if the inputs BTC amount is not enough
    ///   to cover the outputs and transaction fee.
    pub fn create_edict_transaction(
        &self,
        args: &CreateEdictTxArgs,
    ) -> Result<Transaction, OrdError> {
        let runestone = Runestone {
            edicts: vec![Edict {
                id: args.rune,
                amount: args.amount,
                output: 2,
            }],
            etching: None,
            mint: None,
            pointer: None,
        };

        let runestone_out = TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::from_bytes(runestone.encipher().into_bytes()),
        };
        let rune_change_out = TxOut {
            value: RUNE_POSTAGE,
            script_pubkey: args.rune_change_address.script_pubkey(),
        };
        let rune_destination_out = TxOut {
            value: RUNE_POSTAGE,
            script_pubkey: args.destination.script_pubkey(),
        };
        let funding_change_out = TxOut {
            value: Amount::ZERO,
            script_pubkey: args.change_address.script_pubkey(),
        };

        let outputs = vec![
            runestone_out,
            rune_change_out,
            rune_destination_out,
            funding_change_out,
        ];

        let inputs = args
            .inputs
            .iter()
            .map(|rune_input| TxIn {
                previous_output: rune_input.outpoint,
                script_sig: Default::default(),
                sequence: Default::default(),
                witness: Default::default(),
            })
            .collect();

        let mut unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        let fee_amount = estimate_transaction_fees(
            ScriptType::P2TR,
            unsigned_tx.vsize(),
            unsigned_tx.input.len(),
            args.fee_rate,
            &None,
        );
        let change_amount = args
            .input_amount()
            .checked_sub(fee_amount + RUNE_POSTAGE * 2)
            .ok_or(OrdError::InsufficientBalance)?;

        unsigned_tx.output[3].value = change_amount;

        Ok(unsigned_tx)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::str::FromStr;

    use bitcoin::consensus::Decodable;
    use bitcoin::key::Secp256k1;
    use bitcoin::{Network, OutPoint, PrivateKey, PublicKey, Txid};

    use crate::{Wallet, WalletType};

    use super::*;

    #[tokio::test]
    async fn create_edict_transaction() {
        const PRIVATE_KEY: &str =
            "66c4e94a319776225307f6f89644a827c61150d2ac21b1fc110d330364088024";
        let private_key = PrivateKey::from_slice(
            &hex::decode(PRIVATE_KEY).expect("failed to decode hex private key"),
            Network::Regtest,
        )
        .expect("invalid private key");
        let public_key = PublicKey::from_private_key(&Secp256k1::new(), &private_key);
        let wallet = Wallet::new_with_signer(WalletType::Local { private_key });
        let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let args = CreateEdictTxArgs {
            rune: RuneId::new(219, 1).unwrap(),
            inputs: vec![
                TxInputInfo {
                    outpoint: OutPoint::new(
                        Txid::from_str(
                            "9100acad2da80d2198b257acc5d98a6265fda510bc8f1252334876dad4c289f4",
                        )
                        .unwrap(),
                        1,
                    ),
                    tx_out: TxOut {
                        value: Amount::from_sat(10000),
                        script_pubkey: ScriptBuf::from_hex(
                            "5120c57c572f5401e740701ce673bf6c826890eec9d7898bc0415f140cb252fdaf72",
                        )
                        .unwrap(),
                    },
                },
                TxInputInfo {
                    outpoint: OutPoint::new(
                        Txid::from_str(
                            "9100acad2da80d2198b257acc5d98a6265fda510bc8f1252334876dad4c289f4",
                        )
                        .unwrap(),
                        2,
                    ),
                    tx_out: TxOut {
                        value: Amount::from_sat(10000),
                        script_pubkey: ScriptBuf::from_hex(
                            "51200c7598875b445a85a351dafcb08f05a7dc1e958b5f704d2a3f2aeb31f085abd4",
                        )
                        .unwrap(),
                    },
                },
                TxInputInfo {
                    outpoint: OutPoint::new(
                        Txid::from_str(
                            "9100acad2da80d2198b257acc5d98a6265fda510bc8f1252334876dad4c289f4",
                        )
                        .unwrap(),
                        3,
                    ),
                    tx_out: TxOut {
                        value: Amount::from_sat(9943140),
                        script_pubkey: ScriptBuf::from_hex(
                            "5120ddf99a3af83d2f741c955394345df2abd67a33d4e9b27d6256b65cfb24b64236",
                        )
                        .unwrap(),
                    },
                },
            ],
            destination: Address::from_str(
                "bcrt1pu8kl0t74qn89ljqs6ez558uyjvht3d93hsa2ha3u7654hgqjmadqlm20ps",
            )
            .unwrap()
            .assume_checked(),
            change_address: Address::from_str(
                "bcrt1pxsxjyxykvchklqaz0w6tk5wz28rmqn3efdt472g53s9m9hkwp3fs452s2t",
            )
            .unwrap()
            .assume_checked(),
            rune_change_address: Address::from_str(
                "bcrt1prsz63kjxu8qmgt8m0k6em7k9hkwwqqsykpts4ad5fkvq5yqt985sfl88qq",
            )
            .unwrap()
            .assume_checked(),
            amount: 9500,
            fee_rate: FeeRate::from_sat_per_vb(10).unwrap(),
        };
        let unsigned_tx = builder
            .create_edict_transaction(&args)
            .expect("failed to create transaction");

        let signed_tx = builder
            .sign_transaction(&unsigned_tx, &args.inputs)
            .await
            .expect("failed to sign transaction");

        eprintln!("Signed tx size: {}", signed_tx.vsize());

        const EXPECTED: &str = "02000000000103f489c2d4da76483352128fbc10a5fd65628ad9c5ac57b298210da82dadac00910100000000fffffffff489c2d4da76483352128fbc10a5fd65628ad9c5ac57b298210da82dadac00910200000000fffffffff489c2d4da76483352128fbc10a5fd65628ad9c5ac57b298210da82dadac00910300000000fdffffff0400000000000000000a6a5d0700db01019c4a0210270000000000002251201c05a8da46e1c1b42cfb7db59dfac5bd9ce00204b0570af5b44d980a100b29e91027000000000000225120e1edf7afd504ce5fc810d6454a1f84932eb8b4b1bc3aabf63cf6a95ba012df5a6cab970000000000225120340d221896662f6f83a27bb4bb51c251c7b04e394b575f29148c0bb2dece0c53014037152ea3d7d70f9ff2a6df17413e71beb1b976e0800ff8c0bf285ac7dfc04345ecee174bccaa40a1ac12c80e141f77d616d58ac5520fa6cc5995e7a8ad0ea17b0140cf46d195ff294e0947cb915e5814806155c6db651c50062628ce72ffa4e078b0ed53ace9ac5ab514af0452420cfb30164867b01f6ad71d0fe92a3f90ead69ccf01405aeab94c3a51768d16ba58431f463691aea05e56b99b074f3f2ccb299c516d9901b81effde3b9964969de29d373dd5608c6fdd6a2c54df94e530b89a2b37488b00000000";
        let expected =
            Transaction::consensus_decode(&mut Cursor::new(hex::decode(EXPECTED).unwrap()))
                .expect("failed to decode expected transaction");

        eprintln!("Expected tx size: {}", expected.vsize());

        assert_eq!(signed_tx.version, expected.version);
        assert_eq!(signed_tx.lock_time, expected.lock_time);
        assert_eq!(signed_tx.input.len(), expected.input.len());
        assert_eq!(signed_tx.output.len(), expected.output.len());

        for index in 0..signed_tx.input.len() {
            // Do not compare witness since it depends on randomized value in each tx
            assert_eq!(
                signed_tx.input[index].previous_output, expected.input[index].previous_output,
                "Input {index}"
            );
            assert_eq!(
                signed_tx.input[index].script_sig, expected.input[index].script_sig,
                "Input {index}"
            );
        }

        for index in 0..signed_tx.output.len() {
            assert_eq!(
                signed_tx.output[index].script_pubkey, expected.output[index].script_pubkey,
                "Output {index}"
            );
            //todo: add check of value after https://infinityswap.atlassian.net/browse/EPROD-830
        }
    }
}
