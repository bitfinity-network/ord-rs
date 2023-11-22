use bitcoin::absolute::LockTime;
use bitcoin::opcodes::all::{OP_CHECKSIG, OP_DUP, OP_ENDIF, OP_EQUALVERIFY, OP_HASH160, OP_IF};
use bitcoin::opcodes::{OP_0, OP_FALSE};
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{self};
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
};
use bitcoin_hashes::Hash;

use crate::utils::{bytes_to_push_bytes, h160sum, sha256sum};
use crate::{Brc20Op, Brc20Result};

use super::POSTAGE;

const REDEEM_SCRIPT_FIXED_LEN: usize = 1 + 1 + 1 + 1 + 1 + 1 + 3 + 16;
const JSON_CONTENT_TYPE: &str = "application/json";

/// Arguments for creating a commit transaction
pub struct CreateCommitTransactionArgs {
    /// Private key of the sender
    pub private_key: PrivateKey,
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
    /// Network to use
    pub network: Network,
}

pub struct CreateCommitTransaction {
    /// The transaction to be broadcasted
    pub tx: Transaction,
    /// The redeem script to be used in the reveal transaction
    pub redeem_script: ScriptBuf,
}

pub fn create_commit_transaction(
    args: CreateCommitTransactionArgs,
) -> Brc20Result<CreateCommitTransaction> {
    // previous output
    let previous_output = OutPoint {
        txid: args.input_tx,
        vout: args.input_index,
    };
    // get txin script pubkey
    let txin_script_pubkey = generate_txin_script_pubkey(&args.private_key)?;

    // get p2wsh address for output of inscription
    let redeem_script = generate_redeem_script(&args.private_key, &args.inscription)?;
    let p2wsh_address = generate_pw2sh_address(
        &args.private_key,
        &args.inscription,
        args.network,
        &redeem_script,
    )?;

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
        sequence: Sequence::from_consensus(0xffffffff), // TODO: what is this?
        witness: Witness::new(),
    }];

    // make transaction and sign
    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: tx_out,
    };
    let mut hash = SighashCache::new(&tx);
    let signature_hash = hash.p2wpkh_signature_hash(
        args.input_index as usize, // TODO: in the example is zero???
        &txin_script_pubkey,
        Amount::ZERO,
        bitcoin::EcdsaSighashType::All,
    )?;

    let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
    let signature = secp256k1::Secp256k1::new().sign_ecdsa(&message, &args.private_key.inner);

    // Append script signature to tx input
    append_signature_to_input(&args.private_key, &mut tx, signature)?;

    Ok(CreateCommitTransaction { tx, redeem_script })
}

/// Append signature to tx input
fn append_signature_to_input(
    private_key: &PrivateKey,
    tx: &mut Transaction,
    signature: Signature,
) -> Brc20Result<()> {
    let public_key = bytes_to_push_bytes(
        &private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    )?;
    let script_sig = ScriptBuilder::new()
        .push_slice(bytes_to_push_bytes(signature.serialize_der().as_ref())?.as_push_bytes())
        .push_int(bitcoin::EcdsaSighashType::All as i64)
        .push_slice(public_key.as_push_bytes())
        .into_script();

    if let Some(input) = tx.input.get_mut(0) {
        input.script_sig = script_sig;
    }

    Ok(())
}

/// Generate redeem script and then get a pw2sh address to send the commit transaction
fn generate_pw2sh_address(
    private_key: &PrivateKey,
    inscription: &Brc20Op,
    network: Network,
    redeem_script: &ScriptBuf,
) -> Brc20Result<Address> {
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
    let public_key = bytes_to_push_bytes(
        &private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    )?;
    let encoded_inscription = bytes_to_push_bytes(inscription.encode()?.as_bytes())?;

    Ok(ScriptBuilder::new()
        .push_slice(public_key.as_push_bytes())
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(b"ord")
        .push_int(0x01)
        .push_slice(bytes_to_push_bytes(JSON_CONTENT_TYPE.as_bytes())?.as_push_bytes())
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
