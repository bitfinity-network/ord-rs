mod utils;

use std::str::FromStr as _;

use argh::FromArgs;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, PrivateKey};
use log::{debug, info};
use ord_rs::wallet::{
    CreateCommitTransactionArgsV2, EtchingTransactionArgs, Runestone, SignCommitTransactionArgs,
};
use ord_rs::{Nft, OrdTransactionBuilder};
use ordinals::{Etching, Rune, Terms};
use utils::rpc_client;

use crate::utils::{calc_fees, Fees};

#[derive(FromArgs, Debug)]
#[argh(description = "Etch a runestone")]
struct Args {
    #[argh(option, short = 'a')]
    /// amount
    amount: u64,

    #[argh(option, short = 'l')]
    /// amount
    limit: u64,

    #[argh(option, short = 'p')]
    /// private key
    private_key: String,

    #[argh(positional)]
    /// tx inputs
    inputs: Vec<String>,

    #[argh(option, short = 'n')]
    /// network
    network: String,

    #[argh(option, short = 'N')]
    /// rune name
    rune_name: String,

    #[argh(option, short = 's', default = "String::from(\"p2tr\")")]
    /// script type (p2tr, p2wsh)
    script_type: String,

    #[argh(switch, short = 'd')]
    /// dry run, don't send any transaction
    dry_run: bool,
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

    let amount = args.amount;
    let limit = args.limit;
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

    let etching = Etching {
        rune: Some(Rune::from_str(&args.rune_name).unwrap()),
        divisibility: Some(2),
        premine: Some(1_000_000),
        spacers: None,
        symbol: Some('$'),
        terms: Some(Terms {
            amount: Some(amount as u128),
            cap: Some(limit as u128),
            height: (None, None),
            offset: (None, None),
        }),
        turbo: true,
    };

    let mut inscription = Nft::new(
        Some("text/plain;charset=utf-8".as_bytes().to_vec()),
        Some(etching.rune.unwrap().to_string().as_bytes().to_vec()),
    );
    inscription.pointer = Some(vec![]);
    inscription.rune = Some(
        etching
            .rune
            .ok_or(anyhow::anyhow!("Invalid etching data; rune is missing"))?
            .commitment(),
    );

    let commit_tx = builder
        .build_commit_transaction_with_fixed_fees(
            network,
            CreateCommitTransactionArgsV2 {
                inputs: inputs.clone(),
                inscription,
                txin_script_pubkey: sender_address.script_pubkey(),
                leftovers_recipient: sender_address.clone(),
                commit_fee,
                reveal_fee,
                derivation_path: None,
            },
        )
        .await?;
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

    debug!("commit transaction: {signed_commit_tx:?}");

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

    // make runestone
    let runestone = Runestone {
        etching: Some(etching),
        edicts: vec![],
        mint: None,
        pointer: Some(1),
    };

    debug!("getting reveal transaction...");
    let reveal_transaction = builder
        .build_etching_transaction(EtchingTransactionArgs {
            input: ord_rs::wallet::Utxo {
                id: commit_txid,
                index: 0,
                amount: commit_tx.reveal_balance,
            },
            recipient_address: sender_address,
            redeem_script: commit_tx.redeem_script,
            runestone,
            derivation_path: None,
        })
        .await?;
    debug!("reveal transaction: {reveal_transaction:?}");

    if !args.dry_run {
        // wait for commit transaction to be inserted
        rpc_client::wait_for_tx(&commit_txid, network).await?;

        info!(
            "commit transaction confirmed; broadcasting reveal transaction: {}",
            reveal_transaction.txid()
        );
        let txid = rpc_client::broadcast_transaction(&reveal_transaction, network).await?;
        info!("Reveal transaction broadcasted: {}", txid);
    }

    Ok(())
}
