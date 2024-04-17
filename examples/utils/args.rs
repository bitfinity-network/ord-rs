use std::str::FromStr;

use bitcoin::key::UntweakedKeypair;
use bitcoin::{secp256k1, Address, Network, PublicKey, Txid, XOnlyPublicKey};

pub fn parse_inputs(input: Vec<String>) -> Vec<(Txid, u32)> {
    input
        .into_iter()
        .map(|input| {
            let mut parts = input.split(':');
            let txid = Txid::from_str(parts.next().unwrap()).unwrap();
            let vout = parts.next().unwrap().parse::<u32>().unwrap();
            (txid, vout)
        })
        .collect()
}

/// Creates a Bitcoin address from a public key based on the provided script type.
///
/// ## NOTE
///
/// For the `P2SH` script type, for example, we need the redeem script in order to
/// call `Address::p2sh`. Therefore, it's not supported.
pub fn address_from_pubkey(
    public_key: &PublicKey,
    network: Network,
    script_type: &str,
) -> Result<Address, String> {
    match script_type.to_lowercase().as_str() {
        "p2pkh" => Ok(Address::p2pkh(public_key, network)),
        "p2wpkh" => Address::p2wpkh(public_key, network).map_err(|e| e.to_string()),
        "p2tr" => {
            let secp_ctx = secp256k1::Secp256k1::new();
            let keypair = UntweakedKeypair::new(&secp_ctx, &mut rand::thread_rng());
            let x_public_key = XOnlyPublicKey::from_keypair(&keypair).0;
            Ok(Address::p2tr(&secp_ctx, x_public_key, None, network))
        }
        _ => Err("Unsupported script type".to_string()),
    }
}
