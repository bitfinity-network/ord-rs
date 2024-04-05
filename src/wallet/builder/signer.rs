use bitcoin::hashes::Hash as _;
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{self, All};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::taproot::{ControlBlock, LeafVersion};
use bitcoin::{
    PrivateKey, PublicKey, ScriptBuf, SegwitV0Sighash, TapLeafHash, TapSighashType, Transaction,
    Witness,
};

use super::super::builder::Utxo;
use super::taproot::TaprootPayload;
use crate::wallet::builder::TxInputInfo;
use crate::{OrdError, OrdResult};

/// An abstraction over a transaction signer.
#[async_trait::async_trait]
pub trait ExternalSigner {
    /// Retrieves the ECDSA public key at the given derivation path.
    async fn ecdsa_public_key(&self) -> String;

    /// Signs a message with an ECDSA key and returns the signature.
    async fn sign_with_ecdsa(&self, message: &str) -> String;

    /// Verifies an ECDSA signature against a message and a public key.
    async fn verify_ecdsa(&self, signature_hex: &str, message: &str, public_key_hex: &str) -> bool;
}

/// Types of signers.
pub enum WalletType {
    Local {
        private_key: PrivateKey,
    },
    External {
        signer: Box<dyn ExternalSigner + Send + Sync>,
    },
}

/// An Ordinal-aware Bitcoin wallet.
pub struct Wallet {
    pub signer: WalletType,
    secp: Secp256k1<All>,
}

impl Wallet {
    pub fn new_with_signer(signer: WalletType) -> Self {
        Self {
            signer,
            secp: Secp256k1::new(),
        }
    }

    pub async fn sign_commit_transaction(
        &mut self,
        own_pubkey: &PublicKey,
        inputs: &[Utxo],
        transaction: Transaction,
        txin_script: &ScriptBuf,
    ) -> OrdResult<Transaction> {
        self.sign_ecdsa(
            own_pubkey,
            inputs,
            transaction,
            txin_script,
            TransactionType::Commit,
        )
        .await
    }

    pub async fn sign_reveal_transaction_ecdsa(
        &mut self,
        own_pubkey: &PublicKey,
        input: &Utxo,
        transaction: Transaction,
        redeem_script: &bitcoin::ScriptBuf,
    ) -> OrdResult<Transaction> {
        self.sign_ecdsa(
            own_pubkey,
            &[input.clone()],
            transaction,
            redeem_script,
            TransactionType::Reveal,
        )
        .await
    }

    pub fn sign_reveal_transaction_schnorr(
        &mut self,
        taproot: &TaprootPayload,
        redeem_script: &ScriptBuf,
        transaction: Transaction,
    ) -> OrdResult<Transaction> {
        let prevouts_array = vec![taproot.prevouts.clone()];
        let prevouts = Prevouts::All(&prevouts_array);

        let mut sighash_cache = SighashCache::new(transaction.clone());
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

    /// Sign a generic transaction.
    ///
    /// The given transaction must have the same inputs as the ones given in the `prev_outs` argument.
    /// The signature is checked against the given `own_pubkey` public key before being accepted
    /// as valid and returned.
    pub async fn sign_transaction(
        &self,
        transaction: &Transaction,
        prev_outs: &[TxInputInfo],
        own_pubkey: &PublicKey,
    ) -> OrdResult<Transaction> {
        if transaction.input.len() != prev_outs.len() {
            return Err(OrdError::InvalidInputs);
        }

        let mut cache = SighashCache::new(transaction.clone());
        for (index, input) in prev_outs.iter().enumerate() {
            let sighash = match &input.tx_out.script_pubkey {
                s if s.is_p2wpkh() => cache.p2wpkh_signature_hash(
                    index,
                    s,
                    input.tx_out.value,
                    bitcoin::EcdsaSighashType::All,
                )?,
                s if s.is_p2wsh() => cache.p2wsh_signature_hash(
                    index,
                    s,
                    input.tx_out.value,
                    bitcoin::EcdsaSighashType::All,
                )?,
                _ => return Err(OrdError::InvalidScriptType),
            };
            let signature = self.sign_msg_hash(sighash, own_pubkey).await?;
            let ord_signature = bitcoin::ecdsa::Signature::sighash_all(signature).into();

            self.append_witness_to_input(
                &mut cache,
                ord_signature,
                index,
                &own_pubkey.inner,
                None,
                None,
            )?;
        }

        Ok(cache.into_transaction())
    }

    async fn sign_ecdsa(
        &mut self,
        own_pubkey: &PublicKey,
        utxos: &[Utxo],
        transaction: Transaction,
        script: &ScriptBuf,
        transaction_type: TransactionType,
    ) -> OrdResult<Transaction> {
        let mut hash = SighashCache::new(transaction.clone());
        for (index, input) in utxos.iter().enumerate() {
            let sighash = match transaction_type {
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

            let signature = self.sign_msg_hash(sighash, own_pubkey).await?;

            // append witness
            let signature = bitcoin::ecdsa::Signature::sighash_all(signature).into();
            match transaction_type {
                TransactionType::Commit => {
                    self.append_witness_to_input(
                        &mut hash,
                        signature,
                        index,
                        &own_pubkey.inner,
                        None,
                        None,
                    )?;
                }
                TransactionType::Reveal => {
                    self.append_witness_to_input(
                        &mut hash,
                        signature,
                        index,
                        &own_pubkey.inner,
                        Some(script),
                        None,
                    )?;
                }
            }
        }

        Ok(hash.into_transaction())
    }

    async fn sign_msg_hash(
        &self,
        sighash: SegwitV0Sighash,
        own_pubkey: &PublicKey,
    ) -> OrdResult<Signature> {
        debug!("Signing transaction and verifying signature");
        let signature = match &self.signer {
            WalletType::External { signer } => {
                let msg_hex = hex::encode(sighash);
                // sign
                let sig_hex = signer.sign_with_ecdsa(&msg_hex).await;
                let signature = Signature::from_compact(&hex::decode(&sig_hex)?)?;

                // verify
                signer
                    .verify_ecdsa(&sig_hex, &msg_hex, &own_pubkey.to_string())
                    .await;
                signature
            }
            WalletType::Local { private_key } => {
                let message = secp256k1::Message::from_digest(sighash.to_byte_array());

                // sign
                let signature = self.secp.sign_ecdsa(&message, &private_key.inner);
                // verify
                self.secp
                    .verify_ecdsa(&message, &signature, &own_pubkey.inner)?;
                signature
            }
        };

        debug!("signature: {}", signature.serialize_der());

        Ok(signature)
    }

    fn append_witness_to_input(
        &self,
        sighasher: &mut SighashCache<Transaction>,
        signature: OrdSignature,
        index: usize,
        pubkey: &secp256k1::PublicKey,
        redeem_script: Option<&ScriptBuf>,
        control_block: Option<&ControlBlock>,
    ) -> OrdResult<()> {
        // push redeem script if necessary
        let witness = if let Some(redeem_script) = redeem_script {
            let mut witness = Witness::new();
            match signature {
                OrdSignature::Ecdsa(signature) => witness.push_ecdsa_signature(&signature),
                OrdSignature::Schnorr(signature) => witness.push(signature.to_vec()),
            }
            witness.push(redeem_script.as_bytes());
            if let Some(control_block) = control_block {
                witness.push(control_block.serialize());
            }
            witness
        } else {
            // otherwise, push pubkey
            match signature {
                OrdSignature::Ecdsa(signature) => Witness::p2wpkh(&signature, pubkey),
                OrdSignature::Schnorr(_) => return Err(OrdError::UnexpectedSignature),
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

/// Type of the transaction to sign
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionType {
    Commit,
    Reveal,
}

enum OrdSignature {
    Schnorr(bitcoin::taproot::Signature),
    Ecdsa(bitcoin::ecdsa::Signature),
}

impl From<bitcoin::taproot::Signature> for OrdSignature {
    fn from(sig: bitcoin::taproot::Signature) -> Self {
        Self::Schnorr(sig)
    }
}

impl From<bitcoin::ecdsa::Signature> for OrdSignature {
    fn from(sig: bitcoin::ecdsa::Signature) -> Self {
        Self::Ecdsa(sig)
    }
}
