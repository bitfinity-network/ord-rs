mod taproot_keypair;

use bitcoin::key::UntweakedKeypair;
use bitcoin::secp256k1::{All, Secp256k1};
use bitcoin::taproot::{ControlBlock, LeafVersion, TaprootBuilder};
use bitcoin::{Address, Amount, Network, ScriptBuf, TxOut, XOnlyPublicKey};

pub use self::taproot_keypair::TaprootKeypair;
use crate::{OrdError, OrdResult};

#[derive(Debug, Clone)]
pub struct TaprootPayload {
    pub address: Address,
    pub control_block: ControlBlock,
    pub prevouts: TxOut,
    pub keypair: UntweakedKeypair,
}

impl TaprootPayload {
    /// Build a taproot payload and get T2PR address
    pub fn build(
        secp: &Secp256k1<All>,
        keypair: UntweakedKeypair,
        x_public_key: XOnlyPublicKey,
        redeem_script: &ScriptBuf,
        reveal_balance: u64,
        network: Network,
    ) -> OrdResult<Self> {
        let taproot_spend_info = TaprootBuilder::new()
            .add_leaf(0, redeem_script.clone())
            .expect("adding leaf should work")
            .finalize(secp, x_public_key)
            .ok()
            .ok_or(OrdError::TaprootCompute)?;

        let address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

        Ok(Self {
            control_block: taproot_spend_info
                .control_block(&(redeem_script.clone(), LeafVersion::TapScript))
                .ok_or(OrdError::TaprootCompute)?,
            keypair,
            prevouts: TxOut {
                value: Amount::from_sat(reveal_balance),
                script_pubkey: address.script_pubkey(),
            },
            address,
        })
    }
}
