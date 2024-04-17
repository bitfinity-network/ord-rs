mod utils;

use std::str::FromStr;

use argh::FromArgs;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, PrivateKey};
use log::{debug, info};

use self::utils::rpc_client;
use crate::utils::calc_fees;
use crate::utils::transaction::spend_utxo_transaction;

#[derive(FromArgs, Debug)]
#[argh(description = "Transfer BRC20 tokens")]
struct Args {
    #[argh(option, short = 't')]
    /// to address (e.g. tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86)
    to: String,

    #[argh(option, short = 'p')]
    /// private key
    private_key: String,

    #[argh(option, short = 'i')]
    /// utxo to spend input (txid:vout)
    utxo: String,

    #[argh(option, short = 'n')]
    /// network
    network: String,

    #[argh(option, short = 's', default = "String::from(\"p2tr\")")]
    /// script type (p2tr, p2wsh)
    script_type: String,

    #[argh(switch, short = 'd')]
    /// dry run, don't send any transaction
    dry_run: bool,

    #[argh(positional)]
    /// tx inputs to fund the transaction fees
    inputs: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args: Args = argh::from_env();

    let network = match args.network.as_str() {
        "testnet" | "test" => Network::Testnet,
        "mainnet" | "prod" => Network::Bitcoin,
        _ => panic!("invalid network"),
    };

    let mut all_inputs = vec![args.utxo];
    all_inputs.extend(args.inputs);
    let inputs = utils::parse_inputs(all_inputs);

    let recipient = Address::from_str(&args.to)?.require_network(network)?;
    debug!("recipient: {recipient}");
    let private_key = PrivateKey::from_wif(&args.private_key)?;
    let public_key = private_key.public_key(&Secp256k1::new());
    let sender_address =
        utils::address_from_pubkey(&public_key, network, &args.script_type.as_str())
            .expect("Failed to derive a valid Bitcoin address from public key");
    debug!("sender address: {sender_address}");

    let inputs = rpc_client::sats_amount_from_tx_inputs(&inputs, network).await?;
    let inscription_input = inputs[0].clone();

    let fee = calc_fees(network).utxo_fee;

    // send UTXO to recipient
    debug!("getting spend-UTXO transaction");
    let spend_utxo_transaction = spend_utxo_transaction(
        &private_key,
        recipient,
        inscription_input.amount,
        inputs,
        fee,
    )?;
    info!("spend-UTXO transaction: {}", spend_utxo_transaction.txid());
    debug!("spend-UTXO transaction: {spend_utxo_transaction:?}");
    // broadcast spend_utxo_transaction
    if !args.dry_run {
        let spend_utxo_txid =
            rpc_client::broadcast_transaction(&spend_utxo_transaction, network).await?;
        info!("Spend UTXO transaction broadcasted: {}", spend_utxo_txid);
    }

    Ok(())
}
