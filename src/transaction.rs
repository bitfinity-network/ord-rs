use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash as _;
use bitcoin::opcodes::all::{OP_CHECKSIG, OP_ENDIF, OP_IF};
use bitcoin::opcodes::{OP_0, OP_FALSE};
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1, Address, Amount, OutPoint, PrivateKey, PublicKey, ScriptBuf, Sequence, Transaction,
    TxIn, TxOut, Txid, Witness,
};

use crate::utils::bytes_to_push_bytes;
use crate::{Brc20Error, Brc20Op, Brc20Result};

const POSTAGE: u64 = 333;

enum TransactionType {
    Commit,
    Reveal,
}

/// Builder for BRC20 transactions
pub struct Brc20TransactionBuilder {
    private_key: PrivateKey,
    public_key: PublicKey,
}

#[derive(Debug)]
/// Arguments for creating a commit transaction
pub struct CreateCommitTransactionArgs {
    /// Inputs of the transaction
    pub inputs: Vec<TxInput>,
    /// Inscription to write
    pub inscription: Brc20Op,
    /// Address to send the leftovers BTC of the trasnsaction
    pub leftovers_recipient: Address,
    /// Fee to pay for the commit transaction
    pub commit_fee: u64,
    /// Fee to pay for the reveal transaction
    pub reveal_fee: u64,
    /// Script pubkey of the inputs
    pub txin_script_pubkey: ScriptBuf,
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
    pub input: TxInput,
    /// Recipient address of the inscription, only support P2PKH
    pub recipient_address: Address,
    /// The redeem script returned by `create_commit_transaction`
    pub redeem_script: ScriptBuf,
}

impl Brc20TransactionBuilder {
    pub fn new(private_key: PrivateKey) -> Self {
        let public_key = private_key.public_key(&bitcoin::secp256k1::Secp256k1::new());
        Self {
            private_key,
            public_key,
        }
    }

    /// Create the commit transaction
    pub fn build_commit_transaction(
        &self,
        args: CreateCommitTransactionArgs,
    ) -> Brc20Result<CreateCommitTransaction> {
        // get p2wsh address for output of inscription
        let redeem_script = self.generate_redeem_script(&args.inscription)?;
        let script_output_address = Address::p2wsh(&redeem_script, self.private_key.network);

        // exceeding amount of transaction to send to leftovers recipient
        let leftover_amount = args
            .inputs
            .iter()
            .map(|input| input.amount.to_sat())
            .sum::<u64>()
            .checked_sub(POSTAGE)
            .and_then(|v| v.checked_sub(args.commit_fee))
            .and_then(|v| v.checked_sub(args.reveal_fee))
            .ok_or(Brc20Error::InsufficientBalance)?;

        let reveal_balance = POSTAGE + args.reveal_fee;

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
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };

        // sign transaction and update witness
        self.sign_transaction(
            &mut tx,
            &args.inputs,
            &args.txin_script_pubkey,
            TransactionType::Commit,
        )?;

        Ok(CreateCommitTransaction {
            tx,
            redeem_script,
            reveal_balance: Amount::from_sat(reveal_balance),
        })
    }

    /// Create the reveal transaction
    pub fn build_reveal_transaction(
        &self,
        args: RevealTransactionArgs,
    ) -> Brc20Result<Transaction> {
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
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };
        self.sign_transaction(
            &mut tx,
            &[args.input],
            &args.redeem_script,
            TransactionType::Reveal,
        )?;

        Ok(tx)
    }

    /// Generate redeem script from private key and inscription
    fn generate_redeem_script(&self, inscription: &Brc20Op) -> Brc20Result<ScriptBuf> {
        let encoded_inscription = bytes_to_push_bytes(inscription.encode()?.as_bytes())?;

        Ok(ScriptBuilder::new()
            .push_key(&self.public_key)
            .push_opcode(OP_CHECKSIG)
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_slice(b"ord")
            .push_slice(bytes_to_push_bytes(&[0x01])?.as_push_bytes())
            .push_slice(b"text/plain;charset=utf-8") // NOTE: YES, IT'S CORRECT, DON'T ASK!!! It's not json for some reasons
            .push_opcode(OP_0)
            .push_slice(encoded_inscription.as_push_bytes())
            .push_opcode(OP_ENDIF)
            .into_script())
    }

    /// Sign transaction p2wsh
    fn sign_transaction(
        &self,
        tx: &mut Transaction,
        inputs: &[TxInput],
        txin_script: &ScriptBuf,
        transaction_type: TransactionType,
    ) -> Brc20Result<()> {
        let ctx = secp256k1::Secp256k1::new();

        let mut hash = SighashCache::new(tx.clone());
        for (index, input) in inputs.iter().enumerate() {
            let signature_hash = match transaction_type {
                TransactionType::Commit => hash.p2wpkh_signature_hash(
                    index,
                    txin_script,
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )?,
                TransactionType::Reveal => hash.p2wsh_signature_hash(
                    index,
                    txin_script,
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )?,
            };

            let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
            let signature = ctx.sign_ecdsa(&message, &self.private_key.inner);

            let pubkey = self.private_key.inner.public_key(&ctx);
            // verify signature
            ctx.verify_ecdsa(&message, &signature, &pubkey)?;
            // append witness
            match transaction_type {
                TransactionType::Commit => {
                    self.append_witness_to_input(&mut hash, signature, index, &pubkey, None)?;
                }
                TransactionType::Reveal => {
                    self.append_witness_to_input(
                        &mut hash,
                        signature,
                        index,
                        &pubkey,
                        Some(txin_script),
                    )?;
                }
            }
        }

        *tx = hash.into_transaction();

        Ok(())
    }

    fn append_witness_to_input(
        &self,
        sighasher: &mut SighashCache<Transaction>,
        signature: Signature,
        index: usize,
        pubkey: &bitcoin::secp256k1::PublicKey,
        redeem_script: Option<&ScriptBuf>,
    ) -> Brc20Result<()> {
        // push redeem script if necessary
        let witness = if let Some(redeem_script) = redeem_script {
            let mut witness = Witness::new();
            witness.push_ecdsa_signature(&bitcoin::ecdsa::Signature::sighash_all(signature));
            witness.push(redeem_script.as_bytes());
            witness
        } else {
            // otherwise, push pubkey
            Witness::p2wpkh(&bitcoin::ecdsa::Signature::sighash_all(signature), pubkey)
        };

        // append witness
        *sighasher
            .witness_mut(index)
            .ok_or(Brc20Error::InputNotFound(index))? = witness;

        Ok(())
    }
}

impl From<PrivateKey> for Brc20TransactionBuilder {
    fn from(private_key: PrivateKey) -> Self {
        Self::new(private_key)
    }
}

#[derive(Debug, Clone)]
pub struct TxInput {
    pub id: Txid,
    pub index: u32,
    pub amount: Amount,
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
        // <https://mempool.space/testnet/tx/a2153d0c0efba1b8499fdeb61b86a768034c3541d6056754e23a44ce4a03a883>
        // made by address tb1qzc8dhpkg5e4t6xyn4zmexxljc4nkje59dg3ark
        let private_key = PrivateKey::from_wif(WIF).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());
        let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();

        let builder = Brc20TransactionBuilder::new(private_key);

        let commit_transaction_args = CreateCommitTransactionArgs {
            inputs: vec![TxInput {
                id: Txid::from_str(
                    "a2153d0c0efba1b8499fdeb61b86a768034c3541d6056754e23a44ce4a03a883",
                )
                .unwrap(), // the transaction that funded our walle
                index: 0,
                amount: Amount::from_sat(8_000),
            }],
            txin_script_pubkey: address.script_pubkey(),
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
                inputs: vec![TxInput {
                    id: Txid::from_str(
                        "5b3cf3573442df94895dfdef2509a6bc38c245bb9c403c9879933bb4c47452b1",
                    )
                    .unwrap(),
                    index: 0,
                    amount: Amount::from_sat(100_000),
                }],
                txin_script_pubkey: address.script_pubkey(),
                inscription: Brc20Op::deploy("ordi".to_string(), 21_000_000, Some(100_000), None),
                leftovers_recipient: address.clone(),
                commit_fee: 15_000,
                reveal_fee,
            })
            .unwrap();

        let reveal_tx = builder
            .build_reveal_transaction(RevealTransactionArgs {
                input: TxInput {
                    id: Txid::from_str(
                        "afe019fb1556e7eb1626ba85fa92fb90b2ee9769f6d01647454bc389d4431b6f",
                    )
                    .unwrap(),
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: address,
                redeem_script: commit_tx.redeem_script.clone(),
            })
            .unwrap();

        assert_eq!(reveal_tx.input.len(), 1);
        assert_eq!(reveal_tx.output.len(), 1);
        assert_eq!(reveal_tx.output[0].value, Amount::from_sat(POSTAGE));
    }
}
