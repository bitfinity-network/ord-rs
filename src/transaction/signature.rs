use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::sighash::SighashCache;
use bitcoin::{secp256k1, PrivateKey, ScriptBuf, Transaction, Witness};
use bitcoin_hashes::Hash;

use super::TxInput;
use crate::utils::bytes_to_push_bytes;
use crate::Brc20Result;

/// Sign transaction
pub fn sign_transaction(
    tx: &mut Transaction,
    private_key: &PrivateKey,
    inputs: &[TxInput],
    txin_script: &ScriptBuf,
) -> Brc20Result<()> {
    for (index, input) in inputs.iter().enumerate() {
        let mut hash = SighashCache::new(tx.clone());
        let signature_hash = hash.p2wsh_signature_hash(
            index as usize,
            txin_script,
            input.amount,
            bitcoin::EcdsaSighashType::All,
        )?;

        let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
        let signature = secp256k1::Secp256k1::new().sign_ecdsa(&message, &private_key.inner);

        // Append script signature to tx input
        append_signature_to_input(private_key, tx, signature, index)?;

        // append witness
        append_witness_to_input(private_key, tx, signature, index)?;
    }

    Ok(())
}

fn append_witness_to_input(
    private_key: &PrivateKey,
    tx: &mut Transaction,
    signature: Signature,
    index: usize,
) -> Brc20Result<()> {
    let mut witness = Witness::new();
    witness.push_ecdsa_signature(&bitcoin::ecdsa::Signature::sighash_all(signature));
    witness.push(
        private_key
            .public_key(&secp256k1::Secp256k1::new())
            .to_bytes(),
    );
    if let Some(input) = tx.input.get_mut(index) {
        input.witness = witness;
        Ok(())
    } else {
        Err(crate::Brc20Error::InputNotFound(index))
    }
}

/// Append signature to tx input
fn append_signature_to_input(
    private_key: &PrivateKey,
    tx: &mut Transaction,
    signature: Signature,
    index: usize,
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

    if let Some(input) = tx.input.get_mut(index) {
        input.script_sig = script_sig;
        Ok(())
    } else {
        Err(crate::Brc20Error::InputNotFound(index))
    }
}
