mod utils;

use argh::FromArgs;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, PrivateKey};
use log::{debug, info};
use ord_rs::wallet::{
    CreateCommitTransactionArgsV2, RevealTransactionArgs, SignCommitTransactionArgs,
};
use ord_rs::{Brc20, OrdTransactionBuilder};

use self::utils::rpc_client;
use crate::utils::{calc_fees, Fees};

#[derive(FromArgs, Debug)]
#[argh(description = "Transfer BRC20 tokens")]
struct Args {
    #[argh(option, short = 'T')]
    /// ticker
    ticker: String,

    #[argh(option, short = 'a')]
    /// amount
    amount: u64,

    #[argh(option, short = 'p')]
    /// private key
    private_key: String,

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
    /// tx inputs
    inputs: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args: Args = argh::from_env();

    let inputs = utils::parse_inputs(args.inputs);

    let network = match args.network.as_str() {
        "testnet" | "test" => Network::Testnet,
        "mainnet" | "prod" => Network::Bitcoin,
        _ => panic!("invalid network"),
    };
    let ticker = args.ticker;
    let amount = args.amount;
    let private_key = PrivateKey::from_wif(&args.private_key)?;
    let public_key = private_key.public_key(&Secp256k1::new());
    let sender_address = Address::p2wpkh(&public_key, network).unwrap();
    debug!("sender address: {sender_address}");

    let Fees {
        commit_fee,
        reveal_fee,
        ..
    } = calc_fees(network);
    info!("Commit fee: {commit_fee}, reveal fee: {reveal_fee}",);

    let inputs = rpc_client::sats_amount_from_tx_inputs(&inputs, network).await?;

    debug!("getting commit transaction...");
    let mut builder = match args.script_type.as_str() {
        "p2tr" | "P2TR" => OrdTransactionBuilder::p2tr(private_key),
        "p2wsh" | "P2WSH" => OrdTransactionBuilder::p2wsh(private_key),
        _ => panic!("invalid script type"),
    };

    let commit_tx = builder
        .build_commit_transaction_with_fixed_fees(
            network,
            CreateCommitTransactionArgsV2 {
                inputs: inputs.clone(),
                inscription: Brc20::transfer(ticker, amount),
                txin_script_pubkey: sender_address.script_pubkey(),
                leftovers_recipient: sender_address.clone(),
                commit_fee,
                reveal_fee,
            },
        )
        .await?;
    debug!("commit transaction: {commit_tx:?}");

    let signed_commit_tx = builder
        .sign_commit_transaction(
            commit_tx.unsigned_tx,
            SignCommitTransactionArgs {
                inputs,
                txin_script_pubkey: sender_address.script_pubkey(),
                derivation_path: None,
            },
        )
        .await?;

    let commit_txid = if args.dry_run {
        signed_commit_tx.txid()
    } else {
        info!(
            "broadcasting Commit transaction: {}",
            signed_commit_tx.txid()
        );
        rpc_client::broadcast_transaction(&signed_commit_tx, network).await?
    };
    info!("Commit transaction broadcasted: {}", commit_txid);

    debug!("getting reveal transaction...");
    let reveal_transaction = builder
        .build_reveal_transaction(RevealTransactionArgs {
            input: ord_rs::wallet::Utxo {
                id: commit_txid,
                index: 0,
                amount: commit_tx.reveal_balance,
            },
            recipient_address: sender_address, // NOTE: it's correct, see README.md to read about how transfer works
            redeem_script: commit_tx.redeem_script,
            derivation_path: None,
        })
        .await?;
    debug!("reveal transaction: {reveal_transaction:?}");

    if args.dry_run {
        info!("dry run, exiting...");
        return Ok(());
    }

    // wait for commit transaction to be confirmed
    rpc_client::wait_for_tx(&commit_txid, network).await?;

    info!(
        "commit transaction confirmed; broadcasting reveal transaction: {}",
        reveal_transaction.txid()
    );
    let reveal_txid = rpc_client::broadcast_transaction(&reveal_transaction, network).await?;
    info!("Reveal transaction broadcasted: {}", reveal_txid);

    Ok(())
}
