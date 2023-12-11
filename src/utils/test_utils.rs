mod rpc_client;

use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, PrivateKey};
use rand::Rng as _;
pub use rpc_client::get_transaction_by_id;

/// Generate a random P2WPKH BTC address and its private key.
pub fn generate_btc_address(network: Network) -> (Address, PrivateKey) {
    let entropy = rand::thread_rng().gen::<[u8; 16]>();
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

    let seed = mnemonic.to_seed("");

    let private_key = bitcoin::PrivateKey::from_slice(&seed[..32], network).unwrap();
    let public_key = private_key.public_key(&Secp256k1::new());

    let address = Address::p2wpkh(&public_key, network).unwrap();

    (address, private_key)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_generate_btc_address() {
        let (address, private_key) = generate_btc_address(Network::Bitcoin);

        assert_eq!(address.script_pubkey().to_bytes().len(), 22);
        assert_eq!(private_key.network, Network::Bitcoin);
    }
}
