use bitcoin::absolute::LockTime;
use bitcoin::bip32::DerivationPath;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use ordinals::{Edict, Etching, RuneId, Runestone as OrdRunestone};

use super::Utxo;
use crate::constants::POSTAGE;
use crate::fees::estimate_transaction_fees;
use crate::wallet::builder::TxInputInfo;
use crate::wallet::ScriptType;
use crate::{OrdError, OrdResult, OrdTransactionBuilder};

/// Postage amount for rune transaction.
///
/// The value is same as in `ord` tool.
pub const RUNE_POSTAGE: Amount = Amount::from_sat(10_000);

#[cfg_attr(docsrs, doc(cfg(feature = "rune")))]
/// Runestone wrapper; implemented because FOR SOME REASONS, the `Runestone` of `ordinals` doesn't implement Clone...
#[derive(Debug, Default, Clone)]
pub struct Runestone {
    pub edicts: Vec<Edict>,
    pub etching: Option<Etching>,
    pub mint: Option<RuneId>,
    pub pointer: Option<u32>,
}

#[cfg_attr(docsrs, doc(cfg(feature = "rune")))]
impl From<Runestone> for OrdRunestone {
    fn from(runestone: Runestone) -> Self {
        OrdRunestone {
            edicts: runestone.edicts,
            etching: runestone.etching,
            mint: runestone.mint,
            pointer: runestone.pointer,
        }
    }
}

/// Arguments for the [`OrdTransactionBuilder::create_edict_transaction`] method.
#[cfg_attr(docsrs, doc(cfg(feature = "rune")))]
pub struct CreateEdictTxArgs {
    /// Identifier and amount of the runes to be transferred.
    pub runes: Vec<(RuneId, u128)>,
    /// Inputs that contain rune and funding BTC balances.
    pub inputs: Vec<TxInputInfo>,
    /// Address of the recipient of the rune transfer.
    pub destination: Address,
    /// Address that will receive leftovers of BTC.
    pub change_address: Address,
    /// Address that will receive leftovers of runes.
    pub rune_change_address: Address,
    /// Current BTC fee rate.
    pub fee_rate: FeeRate,
}

impl CreateEdictTxArgs {
    fn input_amount(&self) -> Amount {
        self.inputs
            .iter()
            .fold(Amount::ZERO, |a, b| a + b.tx_out.value)
    }
}

/// Arguments for creating a etching reveal transaction
#[derive(Debug, Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "rune")))]
pub struct EtchingTransactionArgs {
    /// Transaction input (output of commit transaction)
    pub input: Utxo,
    /// Recipient address of the inscription, only support P2PKH
    pub recipient_address: Address,
    /// The redeem script returned by `create_commit_transaction`
    pub redeem_script: ScriptBuf,
    /// Runestone to append to the tx outputs
    pub runestone: Runestone,
    /// The derivation path of the input
    pub derivation_path: Option<DerivationPath>,
}

#[cfg_attr(docsrs, doc(cfg(feature = "rune")))]
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
    pub fn create_edict_transaction(&self, args: &CreateEdictTxArgs) -> OrdResult<Transaction> {
        let edicts = args
            .runes
            .iter()
            .map(|(rune, amount)| Edict {
                id: *rune,
                amount: *amount,
                output: 2,
            })
            .collect();

        let runestone = OrdRunestone {
            edicts,
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
            unsigned_tx.input.len(),
            args.fee_rate,
            &None,
            unsigned_tx.output.clone(),
        );
        let change_amount = args
            .input_amount()
            .checked_sub(fee_amount + RUNE_POSTAGE * 2)
            .ok_or(OrdError::InsufficientBalance {
                required: (fee_amount + RUNE_POSTAGE * 2).to_sat(),
                available: args.input_amount().to_sat(),
            })?;

        unsigned_tx.output[3].value = change_amount;

        Ok(unsigned_tx)
    }

    /// Create the reveal transaction
    pub async fn build_etching_transaction(
        &mut self,
        args: EtchingTransactionArgs,
    ) -> OrdResult<Transaction> {
        // previous output
        let previous_output = OutPoint {
            txid: args.input.id,
            vout: args.input.index,
        };

        let runestone = OrdRunestone::from(args.runestone);
        let btc_030_script = runestone.encipher();
        let btc_031_script = ScriptBuf::from_bytes(btc_030_script.to_bytes());

        // tx out
        let tx_out = vec![
            TxOut {
                value: Amount::from_sat(POSTAGE),
                script_pubkey: args.recipient_address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(POSTAGE),
                script_pubkey: args.recipient_address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: btc_031_script,
            },
        ];
        // txin
        let tx_in = vec![TxIn {
            previous_output,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(0xffffffff),
            witness: Witness::new(),
        }];

        // make transaction and sign it
        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };

        let tx = match self.taproot_payload.as_ref() {
            Some(taproot_payload) => {
                self.signer
                    .sign_reveal_transaction_schnorr(
                        &self.public_key,
                        taproot_payload,
                        &args.redeem_script,
                        unsigned_tx,
                        &args.derivation_path.unwrap_or_default(),
                    )
                    .await
            }
            None => {
                self.signer
                    .sign_reveal_transaction_ecdsa(
                        &self.public_key,
                        &args.input,
                        unsigned_tx,
                        &args.redeem_script,
                    )
                    .await
            }
        }?;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::str::FromStr;

    use bitcoin::bip32::DerivationPath;
    use bitcoin::consensus::Decodable;
    use bitcoin::key::Secp256k1;
    use bitcoin::{Network, OutPoint, PrivateKey, PublicKey, Txid};
    use hex_literal::hex;

    use super::*;
    use crate::wallet::{CreateCommitTransactionArgsV2, LocalSigner};
    use crate::{Nft, SignCommitTransactionArgs, Wallet};

    // <https://mempool.space/testnet/address/tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark>
    const WIF: &str = "cVkWbHmoCx6jS8AyPNQqvFr8V9r2qzDHJLaxGDQgDJfxT73w6fuU";

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
        let wallet = Wallet::new_with_signer(LocalSigner::new(private_key));
        let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let args = CreateEdictTxArgs {
            runes: vec![(RuneId::new(219, 1).unwrap(), 9500)],
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
                    derivation_path: DerivationPath::default(),
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
                    derivation_path: DerivationPath::default(),
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
                    derivation_path: DerivationPath::default(),
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

    #[tokio::test]
    async fn test_should_append_runestone() {
        // this test refers to these testnet transactions, commit and reveal:
        // <https://mempool.space/testnet/tx/973f78eb7b3cc666dc4133ff6381c363fd29edda0560d36ea3cfd31f1e85d9f9>
        // <https://mempool.space/testnet/tx/a35802655b63f1c99c1fd3ff8fdf3415f3abb735d647d402c0af5e9a73cbe4c6>
        // made by address tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark

        use ordinals::{Etching, Rune, Terms};
        let private_key = PrivateKey::from_wif(WIF).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());
        let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        let mut builder = OrdTransactionBuilder::p2tr(private_key);

        let inputs = vec![Utxo {
            id: Txid::from_str("791b415dc6946d864d368a0e5ec5c09ee2ad39cf298bc6e3f9aec293732cfda7")
                .unwrap(), // the transaction that funded our wallet
            index: 1,
            amount: Amount::from_sat(8_000),
        }];
        let commit_transaction_args = CreateCommitTransactionArgsV2 {
            inputs: inputs.clone(),
            txin_script_pubkey: address.script_pubkey(),
            inscription: Nft::new(
                Some("text/plain;charset=utf-8".as_bytes().to_vec()),
                Some("SUPERMAXRUNENAME".as_bytes().to_vec()),
            ),
            leftovers_recipient: address.clone(),
            commit_fee: Amount::from_sat(2_500),
            reveal_fee: Amount::from_sat(4_700),
            derivation_path: None,
        };
        let tx_result = builder
            .build_commit_transaction_with_fixed_fees(Network::Testnet, commit_transaction_args)
            .await
            .unwrap();

        assert!(builder.taproot_payload.is_some());

        // sign
        let sign_args = SignCommitTransactionArgs {
            inputs,
            txin_script_pubkey: address.script_pubkey(),
            derivation_path: None,
        };
        let tx = builder
            .sign_commit_transaction(tx_result.unsigned_tx, sign_args)
            .await
            .unwrap();

        let witness = tx.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 2);
        assert_eq!(
            witness[1],
            hex!("02d1c2aebced475b0c672beb0336baa775a44141263ee82051b5e57ad0f2248240")
        );

        let encoded_pubkey = builder.taproot_payload.as_ref().unwrap().pubkey.serialize();
        println!("{} {}", encoded_pubkey.len(), hex::encode(encoded_pubkey));

        // check redeem script contains pubkey for taproot
        let redeem_script = &tx_result.redeem_script;
        assert_eq!(
            redeem_script.as_bytes()[0],
            bitcoin::opcodes::all::OP_PUSHBYTES_32.to_u8()
        );

        let tx_id = tx.txid();
        let recipient_address = Address::from_str("tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap();

        let etching = Etching {
            rune: Some(Rune::from_str("SUPERMAXRUNENAME").unwrap()),
            divisibility: Some(2),
            premine: Some(10_000),
            spacers: None,
            symbol: Some('$'),
            terms: Some(Terms {
                amount: Some(2000),
                cap: Some(500),
                height: (None, None),
                offset: (None, None),
            }),
            turbo: true,
        };
        let runestone = Runestone {
            etching: Some(etching),
            edicts: vec![],
            mint: None,
            pointer: None,
        };

        let expected_script_030 = OrdRunestone::from(runestone.clone()).encipher();
        let expected_script = ScriptBuf::from_bytes(expected_script_030.to_bytes());

        let reveal_transaction = builder
            .build_etching_transaction(EtchingTransactionArgs {
                input: Utxo {
                    id: tx_id,
                    index: 0,
                    amount: tx_result.reveal_balance,
                },
                recipient_address: recipient_address.clone(),
                redeem_script: tx_result.redeem_script,
                runestone: Runestone {
                    edicts: vec![],
                    etching: Some(runestone.etching.unwrap()),
                    mint: None,
                    pointer: None,
                },
                derivation_path: None,
            })
            .await
            .unwrap();

        assert_eq!(reveal_transaction.output.len(), 3);
        assert_eq!(reveal_transaction.output[2].script_pubkey, expected_script);
    }
}
