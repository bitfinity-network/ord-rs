use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::sighash::SighashCache;
use bitcoin::{secp256k1, PrivateKey, Script, ScriptBuf, Transaction, Witness};

use super::TxInput;
use crate::{OrdError, OrdResult, ScriptType};

/// Type of the transaction to sign
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionType {
    Commit,
    Reveal,
}

/// Transaction signer
pub struct Signer<'a> {
    private_key: &'a PrivateKey,
    script_type: ScriptType,
    transaction: &'a mut Transaction,
}

impl<'a> Signer<'a> {
    pub fn new(
        private_key: &'a PrivateKey,
        script_type: ScriptType,
        transaction: &'a mut Transaction,
    ) -> Self {
        Self {
            private_key,
            script_type,
            transaction,
        }
    }

    /// Sign the commit transaction with the given txin script
    pub fn sign_commit_transaction(
        &mut self,
        inputs: &[TxInput],
        txin_script: &ScriptBuf,
    ) -> OrdResult<()> {
        self.sign(inputs, txin_script, TransactionType::Commit)
    }

    /// Sign the reveal transaction with the given redeem script
    pub fn sign_reveal_transaction(
        &mut self,
        input: &TxInput,
        redeem_script: &ScriptBuf,
    ) -> OrdResult<()> {
        self.sign(&[input.clone()], redeem_script, TransactionType::Reveal)
    }

    fn sign(
        &mut self,
        inputs: &[TxInput],
        script: &ScriptBuf,
        transaction_type: TransactionType,
    ) -> OrdResult<()> {
        let ctx = secp256k1::Secp256k1::new();

        let mut hash = SighashCache::new(self.transaction.clone());
        for (index, input) in inputs.iter().enumerate() {
            let signature_hash = match transaction_type {
                TransactionType::Commit => hash.p2wpkh_signature_hash(
                    index,
                    script,
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )?,
                TransactionType::Reveal => hash.p2wsh_signature_hash(
                    index,
                    script,
                    input.amount,
                    bitcoin::EcdsaSighashType::All,
                )?,
            };

            let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
            let signature = ctx.sign_ecdsa(&message, &self.private_key.inner);
            debug!("signature: {}", signature.serialize_der());

            let pubkey = self.private_key.inner.public_key(&ctx);
            // verify signature
            debug!("verifying signature");
            ctx.verify_ecdsa(&message, &signature, &pubkey)?;
            debug!("signature verified");
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
                        Some(script),
                    )?;
                }
            }
        }

        *self.transaction = hash.into_transaction();

        Ok(())
    }

    /// Build and append witness to the transaction input
    fn append_witness_to_input(
        &self,
        sighasher: &mut SighashCache<Transaction>,
        signature: Signature,
        index: usize,
        pubkey: &bitcoin::secp256k1::PublicKey,
        redeem_script: Option<&ScriptBuf>,
    ) -> OrdResult<()> {
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
        debug!("witness: {witness:?}");

        // append witness
        *sighasher
            .witness_mut(index)
            .ok_or(OrdError::InputNotFound(index))? = witness;

        Ok(())
    }
}
