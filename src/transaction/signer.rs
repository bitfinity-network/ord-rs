use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::{All, Secp256k1};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::taproot::{ControlBlock, LeafVersion};
use bitcoin::{
    secp256k1, PrivateKey, ScriptBuf, TapLeafHash, TapSighashType, Transaction, Witness,
};

use super::taproot::TaprootPayload;
use super::TxInput;
use crate::{OrdError, OrdResult};

/// Type of the transaction to sign
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionType {
    Commit,
    Reveal,
}

enum Signature {
    Schnorr(bitcoin::taproot::Signature),
    Ecdsa(bitcoin::ecdsa::Signature),
}

impl From<bitcoin::taproot::Signature> for Signature {
    fn from(sig: bitcoin::taproot::Signature) -> Self {
        Self::Schnorr(sig)
    }
}

impl From<bitcoin::ecdsa::Signature> for Signature {
    fn from(sig: bitcoin::ecdsa::Signature) -> Self {
        Self::Ecdsa(sig)
    }
}

/// Transaction signer
pub struct Signer<'a> {
    private_key: &'a PrivateKey,
    secp: &'a Secp256k1<All>,
    transaction: Transaction,
}

impl<'a> Signer<'a> {
    pub fn new(
        private_key: &'a PrivateKey,
        secp: &'a Secp256k1<All>,
        transaction: Transaction,
    ) -> Self {
        Self {
            private_key,
            secp,
            transaction,
        }
    }

    /// Sign the commit transaction with the given txin script
    pub fn sign_commit_transaction(
        &mut self,
        inputs: &[TxInput],
        txin_script: &ScriptBuf,
    ) -> OrdResult<Transaction> {
        self.sign(inputs, txin_script, TransactionType::Commit)
    }

    /// Sign the reveal transaction with the given redeem script using ECDSA (for P2WSH)
    pub fn sign_reveal_transaction_ecdsa(
        &mut self,
        input: &TxInput,
        redeem_script: &ScriptBuf,
    ) -> OrdResult<Transaction> {
        self.sign(&[input.clone()], redeem_script, TransactionType::Reveal)
    }

    /// Sign the reveal transaction with the given redeem script (for P2TR)
    pub fn sign_reveal_transaction_schnorr(
        &mut self,
        taproot: &TaprootPayload,
        redeem_script: &ScriptBuf,
    ) -> OrdResult<Transaction> {
        let prevouts_array = vec![taproot.prevouts.clone()];
        let prevouts = Prevouts::All(&prevouts_array);

        let mut sighash_cache = SighashCache::new(self.transaction.clone());
        let sighash_sig = sighash_cache.taproot_script_spend_signature_hash(
            0,
            &prevouts,
            TapLeafHash::from_script(redeem_script, LeafVersion::TapScript),
            TapSighashType::Default,
        )?;

        let msg = secp256k1::Message::from_digest(sighash_sig.to_byte_array());
        let sig = self.secp.sign_schnorr_no_aux_rand(&msg, &taproot.keypair);

        // verify
        self.secp
            .verify_schnorr(&sig, &msg, &taproot.keypair.x_only_public_key().0)?;

        // append witness
        let signature = bitcoin::taproot::Signature {
            sig,
            hash_ty: TapSighashType::Default,
        }
        .into();
        self.append_witness_to_input(
            &mut sighash_cache,
            signature,
            0,
            &taproot.keypair.public_key(),
            Some(redeem_script),
            Some(&taproot.control_block),
        )?;

        Ok(sighash_cache.into_transaction())
    }

    fn sign(
        &mut self,
        inputs: &[TxInput],
        script: &ScriptBuf,
        transaction_type: TransactionType,
    ) -> OrdResult<Transaction> {
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
            let signature = self.secp.sign_ecdsa(&message, &self.private_key.inner);
            debug!("signature: {}", signature.serialize_der());

            let pubkey = self.private_key.inner.public_key(self.secp);
            // verify signature
            debug!("verifying signature");
            self.secp.verify_ecdsa(&message, &signature, &pubkey)?;
            debug!("signature verified");
            // append witness
            let signature = bitcoin::ecdsa::Signature::sighash_all(signature).into();
            match transaction_type {
                TransactionType::Commit => {
                    self.append_witness_to_input(&mut hash, signature, index, &pubkey, None, None)?;
                }
                TransactionType::Reveal => {
                    self.append_witness_to_input(
                        &mut hash,
                        signature,
                        index,
                        &pubkey,
                        Some(script),
                        None,
                    )?;
                }
            }
        }

        Ok(hash.into_transaction())
    }

    /// Build and append witness to the transaction input
    fn append_witness_to_input(
        &self,
        sighasher: &mut SighashCache<Transaction>,
        signature: Signature,
        index: usize,
        pubkey: &bitcoin::secp256k1::PublicKey,
        redeem_script: Option<&ScriptBuf>,
        control_block: Option<&ControlBlock>,
    ) -> OrdResult<()> {
        // push redeem script if necessary
        let witness = if let Some(redeem_script) = redeem_script {
            let mut witness = Witness::new();
            match signature {
                Signature::Ecdsa(signature) => witness.push_ecdsa_signature(&signature),
                Signature::Schnorr(signature) => witness.push(signature.to_vec()),
            }
            witness.push(redeem_script.as_bytes());
            if let Some(control_block) = control_block {
                witness.push(control_block.serialize());
            }
            witness
        } else {
            // otherwise, push pubkey
            match signature {
                Signature::Ecdsa(signature) => Witness::p2wpkh(&signature, pubkey),
                Signature::Schnorr(_) => return Err(OrdError::UnexpectedSignature),
            }
        };
        debug!("witness: {witness:?}");

        // append witness
        *sighasher
            .witness_mut(index)
            .ok_or(OrdError::InputNotFound(index))? = witness;

        Ok(())
    }
}
