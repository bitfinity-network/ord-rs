use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};

use super::signature::sign_transaction;
use super::POSTAGE;
use crate::Brc20Result;

/// Arguments for creating a reveal transaction
pub struct RevealTransactionArgs {
    /// Transaction id of the input
    pub input_tx: Txid,
    /// Index of the input in the transaction
    pub input_index: u32,
    /// Balance of the input in sats
    pub input_balance_sats: u64,
    /// Recipient address of the inscription, only support P2PKH
    pub recipient_address: Address,
    /// The redeem script returned by `create_commit_transaction`
    pub redeem_script: ScriptBuf,
}

/// Create the reveal transaction
pub fn create_reveal_transaction(
    private_key: &PrivateKey,
    args: RevealTransactionArgs,
) -> Brc20Result<Transaction> {
    // previous output
    let previous_output = OutPoint {
        txid: args.input_tx,
        vout: args.input_index,
    };
    // tx out
    let tx_out = vec![TxOut {
        value: Amount::from_sat(POSTAGE),
        script_pubkey: args.recipient_address.script_pubkey(),
    }];
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
    sign_transaction(&mut tx, private_key, args.input_index, &args.redeem_script)?;

    Ok(tx)
}
