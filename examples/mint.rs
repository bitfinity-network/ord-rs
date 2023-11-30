use std::str::FromStr;
use std::time::Duration;

use argh::FromArgs;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Amount, Network, PrivateKey, Transaction, Txid};
use log::{debug, info};
use ord_rs::brc20::Brc20;
use ord_rs::transaction::{CreateCommitTransactionArgs, RevealTransactionArgs, TxInput};
use ord_rs::OrdTransactionBuilder;

#[derive(FromArgs, Debug)]
#[argh(description = "Mint BRC20 tokens")]
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

    #[argh(positional)]
    /// tx inputs
    inputs: Vec<String>,

    #[argh(option, short = 'n')]
    /// network
    network: String,

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

    let inputs = parse_inputs(args.inputs);

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
    } = calc_fees(network);
    info!("Commit fee: {commit_fee}, reveal fee: {reveal_fee}",);

    let inputs = sats_amount_from_tx_inputs(&inputs, network).await?;

    debug!("getting commit transaction...");
    let mut builder = match args.script_type.as_str() {
        "p2tr" | "P2TR" => OrdTransactionBuilder::p2tr(private_key),
        "p2wsh" | "P2WSH" => OrdTransactionBuilder::p2wsh(private_key),
        _ => panic!("invalid script type"),
    };

    let commit_tx = builder.build_commit_transaction(CreateCommitTransactionArgs {
        inputs,
        inscription: Brc20::mint(ticker, amount),
        txin_script_pubkey: sender_address.script_pubkey(),
        leftovers_recipient: sender_address.clone(),
        commit_fee,
        reveal_fee,
    })?;
    debug!("commit transaction: {commit_tx:?}");

    let commit_txid = if args.dry_run {
        commit_tx.tx.txid()
    } else {
        info!("broadcasting Commit transaction: {}", commit_tx.tx.txid());
        broadcast_transaction(commit_tx.tx, network).await?
    };
    info!("Commit transaction broadcasted: {}", commit_txid);

    debug!("getting reveal transaction...");
    let reveal_transaction = builder.build_reveal_transaction(RevealTransactionArgs {
        input: ord_rs::transaction::TxInput {
            id: commit_txid,
            index: 0,
            amount: commit_tx.reveal_balance,
        },
        recipient_address: sender_address,
        redeem_script: commit_tx.redeem_script,
    })?;
    debug!("reveal transaction: {reveal_transaction:?}");

    if !args.dry_run {
        // wait for commit transaction to be confirmed
        loop {
            info!("waiting for commit transaction to be confirmed...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            if get_tx_by_hash(&commit_txid, network).await.is_ok() {
                break;
            }
            debug!("retrying in 10 seconds...");
        }

        info!(
            "commit transaction confirmed; broadcasting reveal transaction: {}",
            reveal_transaction.txid()
        );
        let txid = broadcast_transaction(reveal_transaction, network).await?;
        info!("Reveal transaction broadcasted: {}", txid);
    }

    Ok(())
}

struct Fees {
    commit_fee: u64,
    reveal_fee: u64,
}

fn calc_fees(network: Network) -> Fees {
    match network {
        Network::Bitcoin => Fees {
            commit_fee: 15_000,
            reveal_fee: 7_000,
        },
        Network::Testnet | Network::Regtest | Network::Signet => Fees {
            commit_fee: 2_500,
            reveal_fee: 4_700,
        },
        _ => panic!("unknown network"),
    }
}

async fn broadcast_transaction(transaction: Transaction, network: Network) -> anyhow::Result<Txid> {
    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        Network::Bitcoin | _ => "",
    };

    let url = format!("https://blockstream.info{network_str}/api/tx");
    let tx_hex = hex::encode(bitcoin::consensus::serialize(&transaction));
    debug!("tx_hex ({}): {tx_hex}", tx_hex.len());

    let result = reqwest::Client::new()
        .post(&url)
        .body(tx_hex)
        .send()
        .await?;

    debug!("result: {:?}", result);

    if result.status().is_success() {
        let txid = result.text().await?;
        debug!("txid: {txid}");
        Ok(Txid::from_str(&txid)?)
    } else {
        Err(anyhow::anyhow!(
            "failed to broadcast transaction: {}",
            result.text().await?
        ))
    }
}

async fn sats_amount_from_tx_inputs(
    inputs: &[(Txid, u32)],
    network: Network,
) -> anyhow::Result<Vec<TxInput>> {
    let mut output_inputs = Vec::with_capacity(inputs.len());
    for (txid, index) in inputs {
        let tx = get_tx_by_hash(txid, network).await?;
        let output = tx
            .vout
            .get(*index as usize)
            .ok_or_else(|| anyhow::anyhow!("invalid index {} for txid {}", index, txid))?;

        output_inputs.push(TxInput {
            id: *txid,
            index: *index,
            amount: Amount::from_sat(output.value),
        });
    }
    Ok(output_inputs)
}

async fn get_tx_by_hash(txid: &Txid, network: Network) -> anyhow::Result<ApiTransaction> {
    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        Network::Bitcoin | _ => "",
    };

    let url = format!("https://blockstream.info{network_str}/api/tx/{}", txid);
    let tx = reqwest::get(&url).await?.json().await?;
    Ok(tx)
}

fn parse_inputs(input: Vec<String>) -> Vec<(Txid, u32)> {
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

#[derive(Debug, serde::Deserialize)]
struct ApiTransaction {
    vout: Vec<ApiVout>,
}

#[derive(Debug, serde::Deserialize)]
struct ApiVout {
    value: u64,
}
