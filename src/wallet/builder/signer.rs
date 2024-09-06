use bitcoin::bip32::{ChainCode, DerivationPath, Xpriv};
use bitcoin::hashes::Hash as _;
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{self, All, Error, Message};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::taproot::{ControlBlock, LeafVersion};
use bitcoin::{
    Network, PrivateKey, PublicKey, ScriptBuf, TapLeafHash, TapSighashType, Transaction, TxOut,
    Witness, XOnlyPublicKey,
};

use super::super::builder::Utxo;
use super::taproot::TaprootPayload;
use crate::wallet::builder::TxInputInfo;
use crate::{OrdError, OrdResult};

/// An abstraction over a transaction signer.
#[async_trait::async_trait]
pub trait BtcTxSigner {
    /// Retrieves the ECDSA public key at the given derivation path.
    async fn ecdsa_public_key(&self, derivation_path: &DerivationPath) -> PublicKey;

    /// Signs a message with an ECDSA key and returns the signature.
    async fn sign_with_ecdsa(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<Signature, secp256k1::Error>;

    /// Returns the schnorr public key.
    async fn get_schnorr_pubkey(
        &self,
        derivation_path: &DerivationPath,
    ) -> OrdResult<XOnlyPublicKey>;

    /// Signs a message with a Schnorr key and returns the signature.
    async fn sign_with_schnorr(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<secp256k1::schnorr::Signature, secp256k1::Error>;
}

pub struct LocalSigner {
    master_key: Xpriv,
    secp: Secp256k1<All>,
}

impl LocalSigner {
    fn chain_code() -> ChainCode {
        ChainCode::from([0; 32])
    }

    pub fn new(private_key: PrivateKey) -> Self {
        // Network is only used for encoding and decoding the private key and is not important for
        // signing. So we can use any value here.
        let network = Network::Bitcoin;
        Self {
            master_key: Xpriv {
                network,
                depth: 0,
                parent_fingerprint: Default::default(),
                child_number: 0.into(),
                private_key: private_key.inner,
                chain_code: Self::chain_code(),
            },
            secp: Secp256k1::new(),
        }
    }

    fn derived(&self, derivation_path: &DerivationPath) -> Xpriv {
        // Even though API for key derivation returns `Result` there is no actual code path
        // that can return an error. So we can expect this operation to succeed.
        self.master_key
            .derive_priv(&self.secp, derivation_path)
            .expect("key derivation cannot fail")
    }
}

#[async_trait::async_trait]
impl BtcTxSigner for LocalSigner {
    async fn ecdsa_public_key(&self, derivation_path: &DerivationPath) -> PublicKey {
        let child = self.derived(derivation_path);
        let key_pair = child.to_keypair(&self.secp);
        key_pair.public_key().into()
    }

    async fn sign_with_ecdsa(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<Signature, secp256k1::Error> {
        let private_key = self.derived(derivation_path);
        Ok(self.secp.sign_ecdsa(&message, &private_key.private_key))
    }

    /// Returns the schnorr public key.
    async fn get_schnorr_pubkey(
        &self,
        derivation_path: &DerivationPath,
    ) -> OrdResult<XOnlyPublicKey> {
        let keypair = self.derived(derivation_path).to_keypair(&self.secp);
        Ok(XOnlyPublicKey::from_keypair(&keypair).0)
    }

    async fn sign_with_schnorr(
        &self,
        message: Message,
        derivation_path: &DerivationPath,
    ) -> Result<secp256k1::schnorr::Signature, Error> {
        let keypair = self.derived(derivation_path).to_keypair(&self.secp);
        let signature = self.secp.sign_schnorr_no_aux_rand(&message, &keypair);
        Ok(signature)
    }
}

/// An Ordinal-aware Bitcoin wallet.
pub struct Wallet {
    pub signer: Box<dyn BtcTxSigner>,
    secp: Secp256k1<All>,
}

impl Wallet {
    pub fn new_with_signer(signer: impl BtcTxSigner + 'static) -> Self {
        Self {
            signer: Box::new(signer),
            secp: Secp256k1::new(),
        }
    }

    pub async fn sign_commit_transaction(
        &mut self,
        own_pubkey: &PublicKey,
        inputs: &[Utxo],
        transaction: Transaction,
        txin_script: &ScriptBuf,
        derivation_path: &DerivationPath,
    ) -> OrdResult<Transaction> {
        self.sign_ecdsa(
            own_pubkey,
            inputs,
            transaction,
            txin_script,
            TransactionType::Commit,
            derivation_path,
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
            &DerivationPath::default(),
        )
        .await
    }

    pub async fn sign_reveal_transaction_schnorr(
        &mut self,
        own_pubkey: &PublicKey,
        taproot: &TaprootPayload,
        redeem_script: &ScriptBuf,
        transaction: Transaction,
        derivation_path: &DerivationPath,
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
        let sig = self.signer.sign_with_schnorr(msg, derivation_path).await?;

        // verify
        self.secp.verify_schnorr(&sig, &msg, &taproot.pubkey)?;

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
            &own_pubkey.inner,
            Some(redeem_script),
            Some(&taproot.control_block),
        )?;

        Ok(sighash_cache.into_transaction())
    }

    async fn sign_tr(
        &self,
        prev_outs: &[&TxOut],
        index: usize,
        sighash_cache: &mut SighashCache<Transaction>,
        derivation_path: &DerivationPath,
    ) -> OrdResult<()> {
        let prevouts = Prevouts::All(prev_outs);
        let sighash = sighash_cache.taproot_key_spend_signature_hash(
            index,
            &prevouts,
            TapSighashType::Default,
        )?;

        let msg = Message::from(sighash);
        let signature = self.signer.sign_with_schnorr(msg, derivation_path).await?;

        let signature = bitcoin::taproot::Signature {
            sig: signature,
            hash_ty: TapSighashType::Default,
        };

        let mut witness = Witness::new();
        witness.push(signature.to_vec());

        *sighash_cache.witness_mut(index).unwrap() = witness;

        Ok(())
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
    ) -> OrdResult<Transaction> {
        if transaction.input.len() != prev_outs.len() {
            return Err(OrdError::InvalidInputs);
        }

        let mut cache = SighashCache::new(transaction.clone());
        for (index, input) in prev_outs.iter().enumerate() {
            match &input.tx_out.script_pubkey {
                s if s.is_p2wpkh() || s.is_p2wsh() => {
                    let sighash = cache.p2wpkh_signature_hash(
                        index,
                        s,
                        input.tx_out.value,
                        bitcoin::EcdsaSighashType::All,
                    )?;
                    let message = Message::from(sighash);

                    let signature = self
                        .signer
                        .sign_with_ecdsa(message, &input.derivation_path)
                        .await?;
                    let public_key = self.signer.ecdsa_public_key(&input.derivation_path).await;
                    let ord_signature = bitcoin::ecdsa::Signature::sighash_all(signature).into();

                    self.append_witness_to_input(
                        &mut cache,
                        ord_signature,
                        index,
                        &public_key.inner,
                        None,
                        None,
                    )?;
                }
                s if s.is_p2tr() => {
                    self.sign_tr(
                        &prev_outs.iter().map(|v| &v.tx_out).collect::<Vec<_>>(),
                        index,
                        &mut cache,
                        &input.derivation_path,
                    )
                    .await?
                }
                _ => return Err(OrdError::InvalidScriptType),
            }
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
        derivation_path: &DerivationPath,
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

            let message = Message::from(sighash);
            let signature = self
                .signer
                .sign_with_ecdsa(message, derivation_path)
                .await?;

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
