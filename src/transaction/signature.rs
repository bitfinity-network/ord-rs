use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::sighash::SighashCache;
use bitcoin::{secp256k1, PrivateKey, ScriptBuf, Transaction, Witness};
use bitcoin_hashes::Hash;

use super::TxInput;
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
