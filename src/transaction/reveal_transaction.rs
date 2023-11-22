use bitcoin::absolute::LockTime;
use bitcoin::opcodes::all::{OP_CHECKSIG, OP_DUP, OP_ENDIF, OP_EQUALVERIFY, OP_HASH160, OP_IF};
use bitcoin::opcodes::{OP_0, OP_FALSE};
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::{self};
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
};
use bitcoin_hashes::Hash;

use crate::Brc20Result;

/// Arguments for creating a reveal transaction
pub struct RevealTransactionArgs {
    /// Private key of the sender
    pub private_key: PrivateKey,
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
pub fn create_reveal_transaction(args: RevealTransactionArgs) -> Brc20Result<Transaction> {
    // previous output
    let previous_output = OutPoint {
        txid: args.input_tx,
        vout: args.input_index,
    };
    let destination_address = args.recipient_address.script_pubkey();
}
