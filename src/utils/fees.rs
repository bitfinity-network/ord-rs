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
        unsigned_commit_tx.vsize(),
        current_fee_rate,
        multisig_config,
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
        output: tx_out,
    };

    estimate_transaction_fees(
        script_type,
        unsigned_reveal_tx.vsize(),
        current_fee_rate,
        multisig_config,
    )
}

fn estimate_transaction_fees(
    script_type: ScriptType,
    unsigned_tx_size: usize,
    current_fee_rate: FeeRate,
    multisig_config: &Option<MultisigConfig>,
) -> Amount {
    let estimated_sig_size = estimate_signature_size(script_type, multisig_config);
    let total_estimated_tx_size = unsigned_tx_size + estimated_sig_size;

    current_fee_rate
        .fee_vb(total_estimated_tx_size as u64)
        .unwrap()
}

/// Estimates the total size of signatures for a given script type and multisig configuration.
fn estimate_signature_size(
    script_type: ScriptType,
    multisig_config: &Option<MultisigConfig>,
) -> usize {
    match script_type {
        // For P2WSH, calculate based on the multisig configuration if provided.
        ScriptType::P2WSH => match multisig_config {
            Some(config) => ECDSA_SIGHASH_SIZE * config.required,
            None => ECDSA_SIGHASH_SIZE, // Default to single signature size if no multisig config is provided.
        },
        // For P2TR, use the fixed Schnorr signature size.
        ScriptType::P2TR => SCHNORR_SIGHASH_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_transaction_fees_p2wsh_single_signature() {
        let script_type = ScriptType::P2WSH;
        let unsigned_tx_size = 100; // in vbytes
        let current_fee_rate = FeeRate::from_sat_per_vb(5_u64).unwrap();
        let multisig_config: Option<MultisigConfig> = None; // No multisig config for single signature

        let fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );

        // Expected fee calculation: (100 + 73) * 5 = 865 satoshis
        assert_eq!(fee, Amount::from_sat(865));
    }

    #[test]
    fn estimate_transaction_fees_p2wsh_multisig() {
        let script_type = ScriptType::P2WSH;
        let unsigned_tx_size = 200; // in vbytes
        let current_fee_rate = FeeRate::from_sat_per_vb(10_u64).unwrap();
        let multisig_config = Some(MultisigConfig {
            required: 2,
            total: 3,
        }); // 2-of-3 multisig

        let fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );

        // Expected fee calculation: (200 + (73 * 2)) * 10 = 3460 satoshis
        assert_eq!(fee, Amount::from_sat(3460));
    }

    #[test]
    fn estimate_transaction_fees_p2tr() {
        let script_type = ScriptType::P2TR;
        let unsigned_tx_size = 150; // in vbytes
        let current_fee_rate = FeeRate::from_sat_per_vb(1_u64).unwrap();
        let multisig_config = None;

        let fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );

        // Expected fee calculation: (150 + 65) * 1 = 215 satoshis
        assert_eq!(fee, Amount::from_sat(215));
    }

    #[test]
    #[should_panic]
    fn estimate_transaction_fees_overflow() {
        let script_type = ScriptType::P2TR;
        let unsigned_tx_size = usize::MAX;
        let current_fee_rate = FeeRate::from_sat_per_vb(1_u64).unwrap();
        let multisig_config = None;

        let _fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );
    }

    #[test]
    fn estimate_transaction_fees_low_fee_rate() {
        let script_type = ScriptType::P2WSH;
        let unsigned_tx_size = 250; // in vbytes
        let current_fee_rate = FeeRate::from_sat_per_vb(1_u64).unwrap(); // Low fee rate
        let multisig_config = Some(MultisigConfig {
            required: 3,
            total: 5,
        }); // 3-of-5 multisig

        let fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );

        // Expected fee calculation: (250 + (73 * 3)) * 1 = 469 satoshis
        assert_eq!(fee, Amount::from_sat(469));
    }

    #[test]
    fn estimate_transaction_fees_high_fee_rate() {
        let script_type = ScriptType::P2TR;
        let unsigned_tx_size = 180; // in vbytes
        let current_fee_rate = FeeRate::from_sat_per_vb(50_u64).unwrap(); // High fee rate
        let multisig_config = None;

        let fee = estimate_transaction_fees(
            script_type,
            unsigned_tx_size,
            current_fee_rate,
            &multisig_config,
        );

        // Expected fee calculation: (180 + 65) * 50 = 12250 satoshis
        assert_eq!(fee, Amount::from_sat(12250));
    }

    #[test]
    fn estimate_transaction_fees_varying_fee_rate() {
        let script_type = ScriptType::P2WSH;
        let unsigned_tx_size = 300; // in vbytes
                                    // Simulating a fee rate that might be seen during network congestion
        let fee_rates: Vec<u64> = vec![5, 10, 20, 30, 40];

        for fee_rate in fee_rates {
            let current_fee_rate = FeeRate::from_sat_per_vb(fee_rate).unwrap();
            let multisig_config = Some(MultisigConfig {
                required: 2,
                total: 3,
            }); // 2-of-3 multisig

            let fee = estimate_transaction_fees(
                script_type,
                unsigned_tx_size,
                current_fee_rate,
                &multisig_config,
            );

            // Expected fee calculation changes with the fee_rate
            let expected_fee = (300 + (73 * 2)) as u64 * fee_rate;
            assert_eq!(
                fee,
                Amount::from_sat(expected_fee),
                "Fee mismatch at rate: {}",
                fee_rate
            );
        }
    }
}
