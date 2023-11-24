use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};

use super::signature::sign_transaction;
use super::{TxInput, POSTAGE};
use crate::Brc20Result;

/// Arguments for creating a reveal transaction
pub struct RevealTransactionArgs {
    /// Transaction id of the input
    pub input: TxInput,
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
        txid: args.input.id,
        vout: args.input.index,
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
    sign_transaction(&mut tx, private_key, &[args.input], &args.redeem_script)?;

    Ok(tx)
}
