use bitcoin::absolute::LockTime;
use bitcoin::opcodes::all::{OP_CHECKSIG, OP_DUP, OP_ENDIF, OP_EQUALVERIFY, OP_HASH160, OP_IF};
use bitcoin::opcodes::{OP_0, OP_FALSE};
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1, Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction,
    TxIn, TxOut, Txid, Witness,
};

use super::signature::sign_transaction;
use super::POSTAGE;
use crate::utils::{bytes_to_push_bytes, h160sum, sha256sum};
use crate::{Brc20Op, Brc20Result};

#[derive(Debug)]
/// Arguments for creating a commit transaction
pub struct CreateCommitTransactionArgs {
    /// Transaction id of the input
    pub input_tx: Txid,
    /// Index of the input in the transaction
    pub input_index: u32,
    /// Balance of the input in msat, 100k should be enough
    pub input_balance_msat: u64,
    /// Inscription to write
    pub inscription: Brc20Op,
    /// Address to send the leftovers BTC of the trasnsaction
    pub leftovers_recipient: Address,
    /// Fee to pay for the commit transaction
    pub commit_fee: u64,
    /// Fee to pay for the reveal transaction
    pub reveal_fee: u64,
}

#[derive(Debug, Clone)]
pub struct CreateCommitTransaction {
    /// The transaction to be broadcasted
    pub tx: Transaction,
    /// The redeem script to be used in the reveal transaction
    pub redeem_script: ScriptBuf,
}

pub fn create_commit_transaction(
    private_key: &PrivateKey,
    args: CreateCommitTransactionArgs,
) -> Brc20Result<CreateCommitTransaction> {
    // previous output
    let previous_output = OutPoint {
        txid: args.input_tx,
        vout: args.input_index,
    };
    // get txin script pubkey
    let txin_script_pubkey = generate_txin_script_pubkey(private_key)?;

    // get p2wsh address for output of inscription
    let redeem_script = generate_redeem_script(private_key, &args.inscription)?;
    let p2wsh_address = generate_pw2sh_address(private_key.network, &redeem_script)?;

    // exceeding amount of transaction to send to leftovers recipient
    let leftover_amount = args.input_balance_msat - POSTAGE - args.commit_fee - args.reveal_fee;
    // get tx_out
    let tx_out = vec![
        TxOut {
            value: Amount::from_sat(POSTAGE + args.reveal_fee),
            script_pubkey: p2wsh_address.script_pubkey(),
        },
        TxOut {
            value: Amount::from_sat(leftover_amount),
            script_pubkey: args.leftovers_recipient.script_pubkey(),
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
    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: tx_out,
    };
    let signature = sign_transaction(&mut tx, private_key, args.input_index, &txin_script_pubkey)?;
    // put witness
    let mut witness = Witness::new();
    witness.push_ecdsa_signature(&bitcoin::ecdsa::Signature::sighash_all(signature));
    witness.push(
        private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    );
    tx.input[0].witness = witness;

    Ok(CreateCommitTransaction { tx, redeem_script })
}

/// Generate redeem script and then get a pw2sh address to send the commit transaction
fn generate_pw2sh_address(network: Network, redeem_script: &ScriptBuf) -> Brc20Result<Address> {
    let p2wsh_script = ScriptBuilder::new()
        .push_opcode(OP_0)
        .push_slice(bytes_to_push_bytes(&sha256sum(redeem_script.as_bytes()))?.as_push_bytes())
        .into_script();

    // get p2wsh address
    Ok(Address::p2wsh(&p2wsh_script, network))
}

/// Generate redeem script from private key and inscription
fn generate_redeem_script(
    private_key: &PrivateKey,
    inscription: &Brc20Op,
) -> Brc20Result<ScriptBuf> {
    let encoded_inscription = bytes_to_push_bytes(inscription.encode()?.as_bytes())?;

    Ok(ScriptBuilder::new()
        .push_key(&private_key.public_key(&secp256k1::Secp256k1::new()))
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

/// Generate txin script pubkey for commit transaction
fn generate_txin_script_pubkey(private_key: &PrivateKey) -> Brc20Result<ScriptBuf> {
    let origin_address_bytes = h160sum(
        &private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    );

    Ok(ScriptBuilder::new()
        .push_opcode(OP_DUP)
        .push_opcode(OP_HASH160)
        .push_slice(bytes_to_push_bytes(&origin_address_bytes)?.as_push_bytes())
        .push_opcode(OP_EQUALVERIFY)
        .push_opcode(OP_CHECKSIG)
        .into_script())
}
