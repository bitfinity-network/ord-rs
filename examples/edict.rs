mod utils;

use std::str::FromStr;

use argh::FromArgs;
use bitcoin::bip32::DerivationPath;
use bitcoin::consensus::Encodable;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Amount, FeeRate, Network, OutPoint, PrivateKey, TxOut};
use log::debug;
use ord_rs::wallet::{CreateEdictTxArgs, LocalSigner, ScriptType, TxInputInfo};
use ord_rs::{OrdTransactionBuilder, Wallet};
use ordinals::RuneId;

#[derive(FromArgs, Debug)]
#[argh(description = "Create and sign edict transaction")]
struct Args {
    #[argh(option, short = 'a')]
    /// amount
    amount: u128,

    #[argh(option, short = 'p')]
    /// private key
    private_key: String,

    #[argh(option, short = 'i')]
    /// input amounts
    input_amounts: Vec<u64>,

    #[argh(option, short = 'r')]
    /// rune id
    rune_id: RuneId,

    #[argh(option, short = 'd')]
    /// destination
    destination: String,

    #[argh(positional)]
    /// tx inputs
    inputs: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args: Args = argh::from_env();

    let inputs = utils::parse_inputs(args.inputs);
    let amount = args.amount;
    let private_key = PrivateKey::from_wif(&args.private_key)?;
    let public_key = private_key.public_key(&Secp256k1::new());
    let network = Network::Regtest;
    let sender_address = Address::p2wpkh(&public_key, network).unwrap();
    debug!("sender address: {sender_address}");

    let wallet = Wallet::new_with_signer(LocalSigner::new(private_key));
    let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

    let inputs: Vec<_> = inputs
        .into_iter()
        .enumerate()
        .map(|(index, (txid, vout))| TxInputInfo {
            outpoint: OutPoint { txid, vout },
            tx_out: TxOut {
                value: Amount::from_sat(args.input_amounts[index]),
                script_pubkey: sender_address.script_pubkey(),
            },
            derivation_path: DerivationPath::default(),
        })
        .collect();

    let destination = Address::from_str(&args.destination)?.assume_checked();

    let unsigned_tx = builder.create_edict_transaction(&CreateEdictTxArgs {
        rune: args.rune_id,
        inputs: inputs.clone(),
        destination,
        change_address: sender_address.clone(),
        rune_change_address: sender_address,
        amount,
        fee_rate: FeeRate::from_sat_per_vb(10).unwrap(),
    })?;

    let signed_tx = builder.sign_transaction(&unsigned_tx, &inputs).await?;
    let mut bytes = vec![];
    signed_tx.consensus_encode(&mut bytes)?;
    eprintln!("Raw signed transaction:");
    eprintln!("{}", hex::encode(bytes));

    Ok(())
}
