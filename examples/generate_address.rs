use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network};
use rand::Rng as _;

fn main() -> anyhow::Result<()> {
    let args = std::env::args()
        .map(|arg| arg.to_lowercase())
        .collect::<Vec<_>>();

    if args.len() != 2 {
        anyhow::bail!("Usage: generate_address <network>");
    }

    let network = match args[1].as_str() {
        "bitcoin" | "prod" => Network::Bitcoin,
        "testnet" | "test" => Network::Testnet,
        "regtest" => Network::Regtest,
        _ => {
            anyhow::bail!("Invalid network: {}", args[1]);
        }
    };

    let entropy = rand::thread_rng().gen::<[u8; 16]>();
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

    let seed = mnemonic.to_seed("");

    let private_key = bitcoin::PrivateKey::from_slice(&seed[..32], network).unwrap();
    let public_key = private_key.public_key(&Secp256k1::new());

    let address = Address::p2wpkh(&public_key, network).unwrap();

    println!("WIF: {}", private_key.to_wif());
    println!("Mnemonic: {}", mnemonic);
    println!("Address: {}", address);

    Ok(())
}
