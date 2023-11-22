use bitcoin::{
    absolute::LockTime,
    opcodes::{
        all::{OP_CHECKSIG, OP_DUP, OP_ENDIF, OP_EQUALVERIFY, OP_HASH160, OP_IF},
        OP_0, OP_FALSE,
    },
    script::{Builder as ScriptBuilder, PushBytesBuf},
    secp256k1,
    sighash::SighashCache,
    transaction::Version,
    Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
};
use bitcoin_hashes::Hash;
use bytes::{BufMut, BytesMut};

use crate::{
    utils::{h160sum, sha256sum},
    Brc20Op, Brc20Result,
};

const REDEEM_SCRIPT_FIXED_LEN: usize = 1 + 1 + 1 + 1 + 1 + 1 + 3 + 16;
const JSON_CONTENT_TYPE: &str = "application/json";
const POSTAGE: u64 = 333;

pub fn create_commit_transaction(
    private_key: &PrivateKey,
    input_tx: Txid,
    input_index: u32,
    input_balance_msat: u64,
    inscription: Brc20Op,
    origin_address: Address,
    commit_fee: u64,
    reveal_fee: u64,
    network: Network,
) -> Brc20Result<Transaction> {
    // previous output
    let previous_output = OutPoint {
        txid: input_tx,
        vout: input_index,
    };
    // script sig
    let origin_address_bytes = h160sum(
        &private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    );
    let mut origin_address_buf = PushBytesBuf::with_capacity(origin_address_bytes.len());
    origin_address_buf.extend_from_slice(&origin_address_bytes)?;

    let txin_script_pubkey = ScriptBuilder::new()
        .push_opcode(OP_DUP)
        .push_opcode(OP_HASH160)
        .push_slice(origin_address_buf.as_push_bytes())
        .push_opcode(OP_EQUALVERIFY)
        .push_opcode(OP_CHECKSIG)
        .into_script();

    // create txout

    // reedem script
    let public_key = private_key
        .public_key(&secp256k1::Secp256k1::new())
        .to_bytes();
    let encoded_inscription = inscription.encode()?;

    let mut content_type = PushBytesBuf::with_capacity(JSON_CONTENT_TYPE.len());
    content_type.extend_from_slice(JSON_CONTENT_TYPE.as_bytes())?;

    let mut inscription = PushBytesBuf::with_capacity(encoded_inscription.len());
    inscription.extend_from_slice(encoded_inscription.as_bytes())?;

    let reedem_script = ScriptBuilder::new()
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(b"ord")
        .push_int(0x01)
        .push_slice(content_type.as_push_bytes())
        .push_opcode(OP_0)
        .push_slice(inscription.as_push_bytes())
        .push_opcode(OP_ENDIF)
        .into_script();

    // P2WSH script pubkey
    let mut redeem_script_buf = PushBytesBuf::with_capacity(32);
    redeem_script_buf.extend_from_slice(&sha256sum(reedem_script.as_bytes()))?;

    let p2wsh_script = ScriptBuilder::new()
        .push_opcode(OP_0)
        .push_slice(redeem_script_buf.as_push_bytes())
        .into_script();
    // get p2wsh address
    let p2wsh_address = Address::p2wsh(&p2wsh_script, network);

    let leftover_amount = input_balance_msat - POSTAGE - commit_fee - reveal_fee;
    // get tx_out
    let tx_out = vec![
        TxOut {
            value: Amount::from_sat(POSTAGE + reveal_fee),
            script_pubkey: p2wsh_address.script_pubkey(),
        },
        TxOut {
            value: Amount::from_sat(leftover_amount),
            script_pubkey: origin_address.script_pubkey(),
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
        input_index as usize, // TODO: in the example is zero???
        &txin_script_pubkey,
        Amount::ZERO,
        bitcoin::EcdsaSighashType::All,
    )?;

    let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
    let signature = secp256k1::Secp256k1::new().sign_ecdsa(&message, &private_key.inner);

    // convert signature to byte and append SIGHASH_ALL
    let mut signature_buffer = BytesMut::with_capacity(73);
    signature_buffer.put(signature.serialize_der().as_ref());
    signature_buffer.put_u8(bitcoin::EcdsaSighashType::All as u8);
    signature_buffer.put(public_key.as_ref());

    tx.input = vec![TxIn {
        previous_output,
        script_sig: ScriptBuf::from(signature_buffer.to_vec()),
        sequence: Sequence::from_consensus(0xffffffff), // TODO: what is this?
        witness: Witness::new(),
    }];

    // return transaction
    Ok(tx)
}
