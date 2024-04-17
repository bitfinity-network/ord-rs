use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::{self, Secp256k1};
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use ord_rs::wallet::Utxo;
use ord_rs::OrdError;

pub fn spend_utxo_transaction(
    private_key: &PrivateKey,
    recipient: Address,
    utxo_value: Amount,
    inputs: Vec<Utxo>,
    fee: Amount,
) -> anyhow::Result<Transaction> {
    let secp = Secp256k1::new();

    let pubkey = private_key.public_key(&secp);
    let sender_address = Address::p2wpkh(&pubkey, private_key.network)?;

    let leftover_amount = inputs
        .iter()
        .map(|input| input.amount.to_sat())
        .sum::<u64>()
        .checked_sub(fee.to_sat())
        .ok_or_else(|| anyhow::anyhow!("insufficient funds"))?;

    let tx_out = vec![
        TxOut {
            value: utxo_value,
            script_pubkey: recipient.script_pubkey(),
        },
        TxOut {
            value: Amount::from_sat(leftover_amount),
            script_pubkey: sender_address.script_pubkey(),
        },
    ];

    let tx_in = inputs
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

    let unsigned_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: tx_out,
    };

    let tx = sign_transaction(
        unsigned_tx,
        private_key,
        &secp,
        inputs,
        &sender_address.script_pubkey(),
    )?;
    Ok(tx)
}

fn sign_transaction(
    unsigned_tx: Transaction,
    private_key: &PrivateKey,
    secp: &Secp256k1<secp256k1::All>,
    inputs: Vec<Utxo>,
    sender_script_pubkey: &ScriptBuf,
) -> anyhow::Result<Transaction> {
    let mut hash = SighashCache::new(unsigned_tx);

    for (index, input) in inputs.iter().enumerate() {
        let signature_hash = hash.p2wpkh_signature_hash(
            index,
            sender_script_pubkey,
            input.amount,
            bitcoin::EcdsaSighashType::All,
        )?;

        let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
        let signature = secp.sign_ecdsa(&message, &private_key.inner);

        // verify sig
        let secp_pubkey = private_key.inner.public_key(secp);
        secp.verify_ecdsa(&message, &signature, &secp_pubkey)?;
        let signature = bitcoin::ecdsa::Signature::sighash_all(signature);

        // append witness to input
        let witness = Witness::p2wpkh(&signature, &secp_pubkey);
        *hash
            .witness_mut(index)
            .ok_or(OrdError::InputNotFound(index))? = witness;
    }

    Ok(hash.into_transaction())
}
