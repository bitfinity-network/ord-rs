use std::str::FromStr;

use bitcoin::key::{Secp256k1, UntweakedKeypair};
use bitcoin::secp256k1::All;
use bitcoin::{Address, Network, PublicKey, Txid, XOnlyPublicKey};

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
            let secp_ctx = Secp256k1::new();
            let x_public_key = generate_xonly_pubkey(&secp_ctx);
            Ok(Address::p2tr(&secp_ctx, x_public_key, None, network))
        }
        _ => Err("Unsupported script type".to_string()),
    }
}

fn generate_xonly_pubkey(secp_ctx: &Secp256k1<All>) -> XOnlyPublicKey {
    let keypair = UntweakedKeypair::new(secp_ctx, &mut rand::thread_rng());
    XOnlyPublicKey::from_keypair(&keypair).0
}

#[cfg(test)]
mod tests {
    use bitcoin::PrivateKey;
    use rand::Rng;

    use super::*;

    fn get_public_key(network: Network) -> PublicKey {
        let entropy = rand::thread_rng().gen::<[u8; 16]>();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

        let seed = mnemonic.to_seed("");

        let private_key = PrivateKey::from_slice(&seed[..32], network).unwrap();
        private_key.public_key(&Secp256k1::new())
    }

    #[test]
    fn derive_bitcoin_addresses() {
        let public_key = get_public_key(Network::Bitcoin);

        let address = address_from_pubkey(&public_key, Network::Bitcoin, "p2pkh")
            .unwrap()
            .to_string();
        assert!(
            address.starts_with("1") || address.starts_with("m") || address.starts_with("n"),
            "P2PKH addresses should start with '1', 'm', or 'n'"
        );

        let address = address_from_pubkey(&public_key, Network::Bitcoin, "p2wpkh")
            .unwrap()
            .to_string();
        assert!(
            address.starts_with("bc1") || address.starts_with("tb1"),
            "P2WPKH addresses should start with 'bc1' or 'tb1'"
        );

        let address = address_from_pubkey(&public_key, Network::Bitcoin, "p2tr")
            .unwrap()
            .to_string();
        assert!(
            address.starts_with("bc1") || address.starts_with("tb1"),
            "P2TR addresses should start with 'bc1' or 'tb1'"
        );

        assert!(
            address_from_pubkey(&public_key, Network::Bitcoin, "p2sh").is_err(),
            "Should error on unsupported script types like 'P2SH'"
        );
    }
}
