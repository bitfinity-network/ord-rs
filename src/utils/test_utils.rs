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

/// Converts a SEC1 ECDSA signature to the DER format.
#[allow(unused)]
pub fn sec1_to_der(sec1_signature: Vec<u8>) -> Result<Vec<u8>, String> {
    if sec1_signature.len() != 64 {
        return Err("Invalid SEC1 signature length".to_string());
    }

    let mut r = sec1_signature[..32].to_vec();
    if r[0] & 0x80 != 0 {
        r.insert(0, 0x00);
    }

    let mut s = sec1_signature[32..].to_vec();
    if s[0] & 0x80 != 0 {
        s.insert(0, 0x00);
    }

    let mut der_signature = Vec::with_capacity(6 + r.len() + s.len());
    der_signature.push(0x30);
    der_signature.push((4 + r.len() + s.len()) as u8);
    der_signature.push(0x02);
    der_signature.push(r.len() as u8);
    der_signature.extend(r);
    der_signature.push(0x02);
    der_signature.push(s.len() as u8);
    der_signature.extend(s);

    Ok(der_signature)
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
