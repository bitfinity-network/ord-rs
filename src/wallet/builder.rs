pub mod signer;
mod taproot;

use bitcoin::absolute::LockTime;
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1, Address, Amount, FeeRate, Network, OutPoint, PublicKey, ScriptBuf, Sequence,
    Transaction, TxIn, TxOut, Txid, Witness, XOnlyPublicKey,
};
use signer::Wallet;

use super::builder::taproot::{generate_keypair, TaprootPayload};
use crate::inscription::Inscription;
use crate::utils::constants::POSTAGE;
use crate::utils::fees::{estimate_commit_fee, estimate_reveal_fee, MultisigConfig};
use crate::{OrdError, OrdResult};

/// Ordinal-aware transaction builder for arbitrary (`Nft`)
/// and `Brc20` inscriptions.
pub struct OrdTransactionBuilder {
    public_key: PublicKey,
    script_type: ScriptType,
    /// used to sign the reveal transaction when using P2TR
    taproot_payload: Option<TaprootPayload>,
    signer: Wallet,
}

#[derive(Debug)]
/// Arguments for creating a commit transaction
pub struct CreateCommitTransactionArgs<T>
where
    T: Inscription,
{
    /// UTXOs to be used as inputs of the transaction
    pub inputs: Vec<Utxo>,
    /// Inscription to write
    pub inscription: T,
    /// Address to send the leftovers BTC of the trasnsaction
    pub leftovers_recipient: Address,
    /// Fee to pay for the commit transaction
    pub commit_fee: Amount,
    /// Fee to pay for the reveal transaction
    pub reveal_fee: Amount,
    /// Script pubkey of the inputs
    pub txin_script_pubkey: ScriptBuf,
}

#[derive(Debug)]
/// Arguments for creating a commit transaction
pub struct CreateCommitTransactionArgsV2<T>
where
    T: Inscription,
{
    /// UTXOs to be used as inputs of the transaction
    pub inputs: Vec<Utxo>,
    /// Inscription to write
    pub inscription: T,
    /// Address to send the leftovers BTC of the trasnsaction
    pub leftovers_recipient: Address,
    /// Script pubkey of the inputs
    pub txin_script_pubkey: ScriptBuf,
    /// Current fee rate on the network
    pub fee_rate: FeeRate,
    /// Multisig configuration, if applicable
    pub multisig_config: Option<MultisigConfig>,
}

#[derive(Debug, Clone)]
pub struct CreateCommitTransaction {
    /// The transaction to be broadcasted
    pub tx: Transaction,
    /// The redeem script to be used in the reveal transaction
    pub redeem_script: ScriptBuf,
    /// Balance to be passed to reveal transaction
    pub reveal_balance: Amount,
}

/// Arguments for creating a reveal transaction
pub struct RevealTransactionArgs {
    /// Transaction input (output of commit transaction)
    pub input: Utxo,
    /// Recipient address of the inscription, only support P2PKH
    pub recipient_address: Address,
    /// The redeem script returned by `create_commit_transaction`
    pub redeem_script: ScriptBuf,
}

/// Type of the script to use. Both are supported, but P2WSH may not be supported by all the indexers
/// So P2TR is preferred
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    P2WSH,
    P2TR,
}

#[derive(Debug)]
pub enum RedeemScriptPubkey {
    Ecdsa(PublicKey),
    XPublickey(XOnlyPublicKey),
}

impl OrdTransactionBuilder {
    pub fn new(public_key: PublicKey, script_type: ScriptType, signer: Wallet) -> Self {
        Self {
            public_key,
            script_type,
            taproot_payload: None,
            signer,
        }
    }

    /// Creates the commit transaction.
    pub async fn build_commit_transaction_v2<T>(
        &mut self,
        network: Network,
        recipient_address: Address,
        args: CreateCommitTransactionArgsV2<T>,
    ) -> OrdResult<CreateCommitTransaction>
    where
        T: Inscription,
    {
        let secp_ctx = secp256k1::Secp256k1::new();

        // generate P2TR keyts
        let p2tr_keys = match self.script_type {
            ScriptType::P2WSH => None,
            ScriptType::P2TR => Some(generate_keypair(&secp_ctx)),
        };

        // generate redeem script pubkey based on the current script type
        let redeem_script_pubkey = match self.script_type {
            ScriptType::P2WSH => RedeemScriptPubkey::Ecdsa(self.public_key),
            ScriptType::P2TR => RedeemScriptPubkey::XPublickey(p2tr_keys.unwrap().1),
        };

        let redeem_script = self.generate_redeem_script(&args.inscription, redeem_script_pubkey)?;
        debug!("redeem_script: {redeem_script}");

        let reveal_fee = estimate_reveal_fee(
            vec![OutPoint::null()],
            recipient_address,
            redeem_script.clone(),
            self.script_type,
            args.fee_rate,
            &args.multisig_config,
        );

        let reveal_balance = POSTAGE + reveal_fee.to_sat();
        debug!("reveal_balance: {reveal_balance}");

        let script_output_address = match self.script_type {
            ScriptType::P2WSH => Address::p2wsh(&redeem_script, network),
            ScriptType::P2TR => {
                let taproot_payload = TaprootPayload::build(
                    &secp_ctx,
                    p2tr_keys.unwrap().0,
                    p2tr_keys.unwrap().1,
                    &redeem_script,
                    reveal_balance,
                    network,
                )?;

                let address = taproot_payload.address.clone();
                self.taproot_payload = Some(taproot_payload);
                address
            }
        };
        debug!("script_output_address: {script_output_address}");

        let mut leftover_amount = 0;

        let mut tx_out = vec![
            TxOut {
                value: Amount::from_sat(reveal_balance),
                script_pubkey: script_output_address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(leftover_amount),
                script_pubkey: args.txin_script_pubkey.clone(),
            },
        ];

        let tx_in: Vec<TxIn> = args
            .inputs
            .iter()
            .map(|input| TxIn {
                previous_output: OutPoint {
                    txid: input.id,
                    vout: input.index,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::from_consensus(0xffffffff),
                witness: Witness::new(),
            })
            .collect();

        let commit_fee = estimate_commit_fee(
            Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: tx_in.clone(),
                output: tx_out.clone(),
            },
            self.script_type,
            args.fee_rate,
            &args.multisig_config,
        );

        // calc balance
        // exceeding amount of transaction to send to leftovers recipient
        leftover_amount = args
            .inputs
            .iter()
            .map(|input| input.amount.to_sat())
            .sum::<u64>()
            .checked_sub(POSTAGE)
            .and_then(|v| v.checked_sub(commit_fee.to_sat()))
            .and_then(|v| v.checked_sub(reveal_fee.to_sat()))
            .ok_or(OrdError::InsufficientBalance)?;
        debug!("leftover_amount: {leftover_amount}");

        tx_out[1].value = Amount::from_sat(leftover_amount);

        // make transaction and sign it
        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };

        // sign transaction and update witness
        let tx = self
            .signer
            .sign_commit_transaction(
                &self.public_key,
                &args.inputs,
                unsigned_tx,
                &args.txin_script_pubkey,
            )
            .await?;

        Ok(CreateCommitTransaction {
            tx,
            redeem_script,
            reveal_balance: Amount::from_sat(reveal_balance),
        })
    }

    /// Creates the commit transaction.
    pub async fn build_commit_transaction<T>(
        &mut self,
        network: Network,
        args: CreateCommitTransactionArgs<T>,
    ) -> OrdResult<CreateCommitTransaction>
    where
        T: Inscription,
    {
        let secp_ctx = secp256k1::Secp256k1::new();

        // generate P2TR keyts
        let p2tr_keys = match self.script_type {
            ScriptType::P2WSH => None,
            ScriptType::P2TR => Some(generate_keypair(&secp_ctx)),
        };

        // generate redeem script pubkey based on the current script type
        let redeem_script_pubkey = match self.script_type {
            ScriptType::P2WSH => RedeemScriptPubkey::Ecdsa(self.public_key),
            ScriptType::P2TR => RedeemScriptPubkey::XPublickey(p2tr_keys.unwrap().1),
        };

        // calc balance
        // exceeding amount of transaction to send to leftovers recipient
        let leftover_amount = args
            .inputs
            .iter()
            .map(|input| input.amount.to_sat())
            .sum::<u64>()
            .checked_sub(POSTAGE)
            .and_then(|v| v.checked_sub(args.commit_fee.to_sat()))
            .and_then(|v| v.checked_sub(args.reveal_fee.to_sat()))
            .ok_or(OrdError::InsufficientBalance)?;
        debug!("leftover_amount: {leftover_amount}");

        let reveal_balance = POSTAGE + args.reveal_fee.to_sat();
        debug!("reveal_balance: {reveal_balance}");

        // get p2wsh or p2tr address for output of inscription
        let redeem_script = self.generate_redeem_script(&args.inscription, redeem_script_pubkey)?;
        debug!("redeem_script: {redeem_script}");
        let script_output_address = match self.script_type {
            ScriptType::P2WSH => Address::p2wsh(&redeem_script, network),
            ScriptType::P2TR => {
                let taproot_payload = TaprootPayload::build(
                    &secp_ctx,
                    p2tr_keys.unwrap().0,
                    p2tr_keys.unwrap().1,
                    &redeem_script,
                    reveal_balance,
                    network,
                )?;

                let address = taproot_payload.address.clone();
                self.taproot_payload = Some(taproot_payload);
                address
            }
        };
        debug!("script_output_address: {script_output_address}");

        let tx_out = vec![
            TxOut {
                value: Amount::from_sat(reveal_balance),
                script_pubkey: script_output_address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(leftover_amount),
                script_pubkey: args.txin_script_pubkey.clone(),
            },
        ];

        // txin
        let tx_in = args
            .inputs
            .iter()
            .map(|input| TxIn {
                previous_output: OutPoint {
                    txid: input.id,
                    vout: input.index,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::from_consensus(0xffffffff),
                witness: Witness::new(),
            })
            .collect();

        // make transaction and sign it
        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };

        // sign transaction and update witness
        let tx = self
            .signer
            .sign_commit_transaction(
                &self.public_key,
                &args.inputs,
                unsigned_tx,
                &args.txin_script_pubkey,
            )
            .await?;

        Ok(CreateCommitTransaction {
            tx,
            redeem_script,
            reveal_balance: Amount::from_sat(reveal_balance),
        })
    }

    /// Create the reveal transaction
    pub async fn build_reveal_transaction(
        &mut self,
        args: RevealTransactionArgs,
    ) -> OrdResult<Transaction> {
        // previous output
        let previous_output = OutPoint {
            txid: args.input.id,
            vout: args.input.index,
        };
        // tx out
        let tx_out = vec![TxOut {
            value: Amount::from_sat(POSTAGE),
            script_pubkey: args.recipient_address.script_pubkey(),
        }];
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
            Some(taproot_payload) => self.signer.sign_reveal_transaction_schnorr(
                taproot_payload,
                &args.redeem_script,
                unsigned_tx,
            ),
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

    /// Generate redeem script from script pubkey and inscription
    fn generate_redeem_script<T>(
        &self,
        inscription: &T,
        pubkey: RedeemScriptPubkey,
    ) -> OrdResult<ScriptBuf>
    where
        T: Inscription,
    {
        Ok(inscription
            .generate_redeem_script(ScriptBuilder::new(), pubkey)?
            .into_script())
    }

    /// Initialize a new `OrdTransactionBuilder` with the given private key and use P2TR as script type (preferred).
    pub fn p2tr(private_key: bitcoin::PrivateKey) -> Self {
        use signer::WalletType;

        let public_key = private_key.public_key(&secp256k1::Secp256k1::new());
        let wallet = Wallet::new_with_signer(WalletType::Local { private_key });
        Self::new(public_key, ScriptType::P2TR, wallet)
    }

    /// Initialize a new `OrdTransactionBuilder` with the given private key and use P2WSH as script type.
    /// P2WSH may not be supported by all the indexers, so P2TR should be preferred.
    pub fn p2wsh(private_key: bitcoin::PrivateKey) -> Self {
        use signer::WalletType;

        let public_key = private_key.public_key(&secp256k1::Secp256k1::new());
        let wallet = Wallet::new_with_signer(WalletType::Local { private_key });
        Self::new(public_key, ScriptType::P2WSH, wallet)
    }
}

/// Unspent transaction output to be used as input of a transaction
#[derive(Debug, Clone)]
pub struct Utxo {
    pub id: Txid,
    pub index: u32,
    pub amount: Amount,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::PrivateKey;
    use hex_literal::hex;

    use super::*;
    use crate::Brc20;

    // <https://mempool.space/testnet/address/tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark>
    const WIF: &str = "cVkWbHmoCx6jS8AyPNQqvFr8V9r2qzDHJLaxGDQgDJfxT73w6fuU";

    #[tokio::test]
    async fn test_should_build_transfer_for_brc20_transactions_from_existing_data_with_p2wsh() {
        // this test refers to these testnet transactions, commit and reveal:
        // <https://mempool.space/testnet/tx/4472899344bce1a6c83c6ec45859f79ab622b55b3faf67e555e3e03cee5139e6>
        // <https://mempool.space/testnet/tx/c769750df54ee38fe2bae876dbf1632c779c3af780958a19cee1ca0497c78e80>
        // made by address tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark
        let private_key = PrivateKey::from_wif(WIF).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());
        let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        let mut builder = OrdTransactionBuilder::p2wsh(private_key);

        let commit_transaction_args = CreateCommitTransactionArgs {
            inputs: vec![Utxo {
                id: Txid::from_str(
                    "791b415dc6946d864d368a0e5ec5c09ee2ad39cf298bc6e3f9aec293732cfda7",
                )
                .unwrap(), // the transaction that funded our wallet
                index: 1,
                amount: Amount::from_sat(8_000),
            }],
            txin_script_pubkey: address.script_pubkey(),
            inscription: Brc20::transfer("mona".to_string(), 100),
            leftovers_recipient: address.clone(),
            commit_fee: Amount::from_sat(2_500),
            reveal_fee: Amount::from_sat(4_700),
        };
        let tx_result = builder
            .build_commit_transaction(Network::Testnet, commit_transaction_args)
            .await
            .unwrap();

        assert!(builder.taproot_payload.is_none());

        let witness = tx_result.tx.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 2);
        assert_eq!(witness[0], hex!("30440220708c02ce8166b739f4190bf98538c897f676adc1304bb368ebe910f817fd489602205d708a826b416c2852a6bd7ea464fde8ef3a08eb2fc085ec9e71ed71f6dc582901"));
        assert_eq!(
            witness[1],
            hex!("02d1c2aebced475b0c672beb0336baa775a44141263ee82051b5e57ad0f2248240")
        );

        // check redeem script
        let redeem_script = &tx_result.redeem_script;
        assert_eq!(
            redeem_script.as_bytes()[0],
            bitcoin::opcodes::all::OP_PUSHBYTES_33.to_u8()
        );

        // txin
        assert_eq!(tx_result.tx.input.len(), 1);
        assert_eq!(
            tx_result.tx.input[0].sequence,
            Sequence::from_consensus(0xffffffff)
        );
        assert_eq!(
            tx_result.tx.input[0].previous_output.txid,
            Txid::from_str("791b415dc6946d864d368a0e5ec5c09ee2ad39cf298bc6e3f9aec293732cfda7",)
                .unwrap()
        );

        // txout
        assert_eq!(tx_result.tx.output.len(), 2);
        assert_eq!(tx_result.tx.output[0].value, Amount::from_sat(5_033));
        assert_eq!(tx_result.tx.output[1].value, Amount::from_sat(467));

        let tx_id = tx_result.tx.txid();
        let recipient_address = Address::from_str("tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap();

        let reveal_transaction = builder
            .build_reveal_transaction(RevealTransactionArgs {
                input: Utxo {
                    id: tx_id,
                    index: 0,
                    amount: tx_result.reveal_balance,
                },
                recipient_address: recipient_address.clone(),
                redeem_script: tx_result.redeem_script,
            })
            .await
            .unwrap();

        let witness = reveal_transaction.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 2);
        assert_eq!(witness[0], hex!("3045022100a377f8dc92b903a99c39113d834013e231fbe82caf148fe23ae895fdbb0b04a002203b8dcc738ea682e4931ae752ac57883b85f31e9bea9641974488dfd32e2bb48201"));
        assert_eq!(
            witness[1],
            hex!("2102d1c2aebced475b0c672beb0336baa775a44141263ee82051b5e57ad0f2248240ac0063036f7264010118746578742f706c61696e3b636861727365743d7574662d3800387b226f70223a227472616e73666572222c2270223a226272632d3230222c227469636b223a226d6f6e61222c22616d74223a22313030227d68")
        );

        assert_eq!(reveal_transaction.output.len(), 1);
        assert_eq!(
            reveal_transaction.output[0].value,
            Amount::from_sat(POSTAGE)
        );
        assert_eq!(
            reveal_transaction.output[0].script_pubkey,
            recipient_address.script_pubkey()
        );
    }

    #[tokio::test]
    async fn test_should_build_transfer_for_brc20_transactions_from_existing_data_with_p2tr() {
        // this test refers to these testnet transactions, commit and reveal:
        // <https://mempool.space/testnet/tx/973f78eb7b3cc666dc4133ff6381c363fd29edda0560d36ea3cfd31f1e85d9f9>
        // <https://mempool.space/testnet/tx/a35802655b63f1c99c1fd3ff8fdf3415f3abb735d647d402c0af5e9a73cbe4c6>
        // made by address tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark
        let private_key = PrivateKey::from_wif(WIF).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());
        let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        let mut builder = OrdTransactionBuilder::p2tr(private_key);

        let commit_transaction_args = CreateCommitTransactionArgs {
            inputs: vec![Utxo {
                id: Txid::from_str(
                    "791b415dc6946d864d368a0e5ec5c09ee2ad39cf298bc6e3f9aec293732cfda7",
                )
                .unwrap(), // the transaction that funded our walle
                index: 1,
                amount: Amount::from_sat(8_000),
            }],
            txin_script_pubkey: address.script_pubkey(),
            inscription: Brc20::transfer("mona".to_string(), 100),
            leftovers_recipient: address.clone(),
            commit_fee: Amount::from_sat(2_500),
            reveal_fee: Amount::from_sat(4_700),
        };
        let tx_result = builder
            .build_commit_transaction(Network::Testnet, commit_transaction_args)
            .await
            .unwrap();

        assert!(builder.taproot_payload.is_some());

        let witness = tx_result.tx.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 2);
        assert_eq!(
            witness[1],
            hex!("02d1c2aebced475b0c672beb0336baa775a44141263ee82051b5e57ad0f2248240")
        );

        let encoded_pubkey = builder
            .taproot_payload
            .as_ref()
            .unwrap()
            .keypair
            .public_key()
            .serialize();
        println!("{} {}", encoded_pubkey.len(), hex::encode(encoded_pubkey));

        // check redeem script contains pubkey for taproot
        let redeem_script = &tx_result.redeem_script;
        assert_eq!(
            redeem_script.as_bytes()[0],
            bitcoin::opcodes::all::OP_PUSHBYTES_32.to_u8()
        );

        let tx_id = tx_result.tx.txid();
        let recipient_address = Address::from_str("tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap();

        let reveal_transaction = builder
            .build_reveal_transaction(RevealTransactionArgs {
                input: Utxo {
                    id: tx_id,
                    index: 0,
                    amount: tx_result.reveal_balance,
                },
                recipient_address: recipient_address.clone(),
                redeem_script: tx_result.redeem_script,
            })
            .await
            .unwrap();

        let witness = reveal_transaction.input[0].witness.clone().to_vec();
        assert_eq!(witness.len(), 3);
    }
}
