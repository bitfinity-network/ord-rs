use bitcoin::absolute::LockTime;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, FeeRate, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use serde::{Deserialize, Serialize};

use super::constants::POSTAGE;
use crate::wallet::ScriptType;

/// Single ECDSA signature + SIGHASH type size in bytes.
const ECDSA_SIGHASH_SIZE: usize = 72 + 1;
/// Single Schnorr signature + SIGHASH type size for Taproot in bytes.
const SCHNORR_SIGHASH_SIZE: usize = 64 + 1;

/// Represents multisig configuration (m of n) for a transaction, if applicable.
/// Encapsulates the number of required signatures and the total number of signatories.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MultisigConfig {
    /// Number of required signatures (m)
    pub required: usize,
    /// Total number of signatories (n)
    pub total: usize,
}

pub fn estimate_commit_fee(
    unsigned_commit_tx: Transaction,
    script_type: ScriptType,
    current_fee_rate: FeeRate,
    multisig_config: &Option<MultisigConfig>,
) -> Amount {
    estimate_transaction_fees(
        script_type,
        unsigned_commit_tx.input.len(),
        current_fee_rate,
        multisig_config,
        unsigned_commit_tx.output,
    )
}

pub fn estimate_reveal_fee(
    inputs: Vec<OutPoint>,
    recipient_address: Address,
    redeem_script: ScriptBuf,
    script_type: ScriptType,
    current_fee_rate: FeeRate,
    multisig_config: &Option<MultisigConfig>,
) -> Amount {
    let tx_out = vec![TxOut {
        value: Amount::from_sat(POSTAGE),
        script_pubkey: recipient_address.script_pubkey(),
    }];

    let mut tx_in: Vec<TxIn> = inputs
        .iter()
        .map(|outpoint| TxIn {
            previous_output: *outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(0xffffffff),
            witness: Witness::new(),
        })
        .collect();

    tx_in[0].witness.push(redeem_script.into_bytes());

    let unsigned_reveal_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_in,
        output: tx_out.clone(),
    };

    estimate_transaction_fees(
        script_type,
        unsigned_reveal_tx.input.len(),
        current_fee_rate,
        multisig_config,
        unsigned_reveal_tx.output,
    )
}

pub fn estimate_transaction_fees(
    script_type: ScriptType,
    number_of_inputs: usize,
    current_fee_rate: FeeRate,
    multisig_config: &Option<MultisigConfig>,
    outputs: Vec<TxOut>,
) -> Amount {
    let vbytes = estimate_vbytes(number_of_inputs, script_type, multisig_config, outputs);

    current_fee_rate.fee_vb(vbytes as u64).unwrap()
}

pub fn calculate_transaction_fees(transaction: &Transaction, current_fee_rate: FeeRate) -> Amount {
    let vbytes = transaction.vsize();
    current_fee_rate.fee_vb(vbytes as u64).unwrap()
}

fn estimate_vbytes(
    inputs: usize,
    script_type: ScriptType,
    multisig_config: &Option<MultisigConfig>,
    outputs: Vec<TxOut>,
) -> usize {
    let sighash_size = match script_type {
        // For P2WSH, calculate based on the multisig configuration if provided.
        ScriptType::P2WSH => match multisig_config {
            Some(config) => ECDSA_SIGHASH_SIZE * config.required,
            None => ECDSA_SIGHASH_SIZE, // Default to single signature size if no multisig config is provided.
        },
        // For P2TR, use the fixed Schnorr signature size.
        ScriptType::P2TR => SCHNORR_SIGHASH_SIZE,
    };

    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: (0..inputs)
            .map(|_| TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::from_slice(&[&vec![0; sighash_size]]),
            })
            .collect(),
        output: outputs,
    }
    .vsize()
}

#[cfg(test)]
mod tests {
    use bitcoin::address::NetworkUnchecked;

    use super::*;

    const ADDITIONAL_INPUT_VBYTES: usize = 58;
    const ADDITIONAL_OUTPUT_VBYTES: usize = 43;

    fn outputs(amount: usize) -> Vec<TxOut> {
        let dummy_address = "bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k"
            .parse::<Address<NetworkUnchecked>>()
            .unwrap()
            .assume_checked();
        vec![
            TxOut {
                value: Amount::ZERO,
                script_pubkey: dummy_address.script_pubkey(),
            };
            amount
        ]
    }

    #[test]
    fn test_should_estimate_vbytes() {
        let before = estimate_vbytes(0, ScriptType::P2TR, &None, Vec::new());
        let after = estimate_vbytes(1, ScriptType::P2TR, &None, Vec::new());
        assert_eq!(after - before, ADDITIONAL_INPUT_VBYTES);

        let before = estimate_vbytes(0, ScriptType::P2TR, &None, Vec::new());
        let after = estimate_vbytes(2, ScriptType::P2TR, &None, Vec::new());
        assert_eq!(after - before, ADDITIONAL_INPUT_VBYTES * 2 - 1);
    }

    #[test]
    fn additional_output_size_is_correct() {
        let before = estimate_vbytes(0, ScriptType::P2TR, &None, Vec::new());
        let after = estimate_vbytes(0, ScriptType::P2TR, &None, outputs(1));
        assert_eq!(after - before, ADDITIONAL_OUTPUT_VBYTES);
    }

    #[test]
    fn multi_io_size_is_correct() {
        let before = estimate_vbytes(0, ScriptType::P2TR, &None, Vec::new());
        let after = estimate_vbytes(2, ScriptType::P2TR, &None, outputs(2));
        assert_eq!(
            after - before,
            (ADDITIONAL_OUTPUT_VBYTES * 2) + (ADDITIONAL_INPUT_VBYTES * 2 - 1)
        );
    }

    #[test]
    fn estimate_transaction_fees_p2wsh_single_signature() {
        let script_type = ScriptType::P2WSH;
        // let unsigned_tx_size = 100; // in vbytes
        let number_of_inputs = 5_usize;
        let current_fee_rate = FeeRate::from_sat_per_vb(5_u64).unwrap();
        let multisig_config: Option<MultisigConfig> = None; // No multisig config for single signature

        let dummy_address = "bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k"
            .parse::<Address<NetworkUnchecked>>()
            .unwrap()
            .assume_checked();
        let outputs = vec![
            TxOut {
                value: Amount::ZERO,
                script_pubkey: dummy_address.script_pubkey(),
            },
            TxOut {
                value: Amount::ZERO,
                script_pubkey: dummy_address.script_pubkey(),
            },
        ];

        let fee = estimate_transaction_fees(
            script_type,
            number_of_inputs,
            current_fee_rate,
            &multisig_config,
            outputs.clone(),
        );

        // Expected fee calculation: (100 + (5 * 73)) * 5 = 2325 satoshis
        let tx_size = estimate_vbytes(
            number_of_inputs,
            ScriptType::P2WSH,
            &multisig_config,
            outputs,
        );
        assert_eq!(fee, Amount::from_sat((tx_size * 5) as u64));
    }

    #[test]
    fn estimate_transaction_fees_p2wsh_multisig() {
        let script_type = ScriptType::P2WSH;
        // let unsigned_tx_size = 200; // in vbytes
        let number_of_inputs = 10_usize;
        let current_fee_rate = FeeRate::from_sat_per_vb(10_u64).unwrap();
        let multisig_config = Some(MultisigConfig {
            required: 2,
            total: 3,
        }); // 2-of-3 multisig
        let dummy_address = "bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k"
            .parse::<Address<NetworkUnchecked>>()
            .unwrap()
            .assume_checked();
        let outputs = vec![
            TxOut {
                value: Amount::ZERO,
                script_pubkey: dummy_address.script_pubkey(),
            },
            TxOut {
                value: Amount::ZERO,
                script_pubkey: dummy_address.script_pubkey(),
            },
        ];

        // Expected fee calculation: (100 + (5 * 73)) * 5 = 2325 satoshis
        let tx_size = estimate_vbytes(
            number_of_inputs,
            ScriptType::P2WSH,
            &multisig_config,
            outputs.clone(),
        );

        let fee = estimate_transaction_fees(
            script_type,
            number_of_inputs,
            current_fee_rate,
            &multisig_config,
            outputs,
        );

        // Expected fee calculation: (200 + (10 * 73 * 2)) * 10 = 16600 satoshis
        assert_eq!(fee, Amount::from_sat((tx_size * 10) as u64));
    }

    #[test]
    fn estimate_transaction_fees_p2tr() {
        let script_type = ScriptType::P2TR;
        // let unsigned_tx_size = 150; // in vbytes
        let number_of_inputs = 5_usize;
        let current_fee_rate = FeeRate::from_sat_per_vb(1_u64).unwrap();
        let multisig_config = None;

        // Expected fee calculation: (100 + (5 * 73)) * 5 = 2325 satoshis
        let tx_size = estimate_vbytes(
            number_of_inputs,
            ScriptType::P2TR,
            &multisig_config,
            outputs(2),
        );

        let fee = estimate_transaction_fees(
            script_type,
            number_of_inputs,
            current_fee_rate,
            &multisig_config,
            outputs(2),
        );

        // Expected fee calculation: (150 + (5 * 65)) * 1 = 475 satoshis
        assert_eq!(fee, Amount::from_sat(tx_size as u64));
    }
}
