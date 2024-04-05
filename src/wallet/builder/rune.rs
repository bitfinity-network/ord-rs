use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{Address, Amount, FeeRate, ScriptBuf, Transaction, TxIn, TxOut};
use ordinals::{Edict, RuneId, Runestone};

use crate::fees::estimate_transaction_fees;
use crate::wallet::builder::TxInputInfo;
use crate::wallet::ScriptType;
use crate::{OrdError, OrdTransactionBuilder};

/// Postage amount for rune transaction.
///
/// The value is same as in `ord` tool.
pub const RUNE_POSTAGE: Amount = Amount::from_sat(10_000);

/// Arguments for the [`OrdTransactionBuilder::create_edict_transaction`] method.
pub struct CreateEdictTxArgs {
    /// Identifier of the rune to be transferred.
    rune: RuneId,
    /// Inputs that contain rune balance to be transferred.
    rune_inputs: Vec<TxInputInfo>,
    /// Inputs that contain BTC balance to cover outputs and transaction fees.
    funding_inputs: Vec<TxInputInfo>,
    /// Address of the recipient of the rune transfer.
    destination: Address,
    /// Address that will receive leftovers of runes and BTC.
    change_address: Address,
    /// Amount of the rune to be transferred.
    amount: u128,
    /// Current BTC fee rate.
    fee_rate: FeeRate,
}

impl CreateEdictTxArgs {
    fn inputs(&self) -> impl Iterator<Item = &TxInputInfo> {
        self.rune_inputs.iter().chain(self.funding_inputs.iter())
    }

    fn input_amount(&self) -> Amount {
        self.inputs().fold(Amount::ZERO, |a, b| a + b.tx_out.value)
    }
}

impl OrdTransactionBuilder {
    /// Creates an unsigned rune edict transaction.
    ///
    /// This method doesn't check the runes balances, so it's the responsibility of the caller to
    /// check that the inputs have enough of the given rune balance to make the transfer. As per
    /// runes standard, if the inputs rune balance is less than specified transfer amount, the
    /// amount will be reduced to the available balance amount.
    ///
    /// # Errors
    /// * Returns [`OrdError::InsufficientBalance`] if the inputs BTC amount is not enough
    ///   to cover the outputs and transaction fee.
    pub fn create_edict_transaction(
        &self,
        args: CreateEdictTxArgs,
    ) -> Result<Transaction, OrdError> {
        let runestone = Runestone {
            edicts: vec![Edict {
                id: args.rune,
                amount: args.amount,
                output: 2,
            }],
            etching: None,
            mint: None,
            pointer: None,
        };

        let runestone_out = TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::from_bytes(runestone.encipher().into_bytes()),
        };
        let rune_change_out = TxOut {
            value: RUNE_POSTAGE,
            script_pubkey: args.change_address.script_pubkey(),
        };
        let rune_destination_out = TxOut {
            value: RUNE_POSTAGE,
            script_pubkey: args.destination.script_pubkey(),
        };
        let funding_change_out = TxOut {
            value: Amount::ZERO,
            script_pubkey: args.change_address.script_pubkey(),
        };

        let outputs = vec![
            runestone_out,
            rune_change_out,
            rune_destination_out,
            funding_change_out,
        ];

        let inputs = args
            .inputs()
            .map(|rune_input| TxIn {
                previous_output: rune_input.outpoint,
                script_sig: Default::default(),
                sequence: Default::default(),
                witness: Default::default(),
            })
            .collect();

        let mut unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        let fee_amount =
            estimate_transaction_fees(ScriptType::P2WSH, unsigned_tx.vsize(), args.fee_rate, &None);
        let change_amount = args
            .input_amount()
            .checked_sub(fee_amount + RUNE_POSTAGE * 2)
            .ok_or(OrdError::InsufficientBalance)?;

        unsigned_tx.output[3].value = change_amount;

        Ok(unsigned_tx)
    }
}
