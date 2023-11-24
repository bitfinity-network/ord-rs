use std::str::FromStr;

use argh::FromArgs;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Amount, Network, PrivateKey, Transaction, Txid};
use brc20::transaction::{CreateCommitTransactionArgs, RevealTransactionArgs, TxInput};
use brc20::{Brc20Op, Brc20TransactionBuilder};
use log::{debug, info};

#[derive(FromArgs, Debug)]
#[argh(description = "Transfer BRC20 tokens")]
struct Args {
    #[argh(option, short = 't')]
    /// to address
    to: String,

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

    let recipient = Address::from_str(&args.to)?.require_network(network)?;
    debug!("recipient: {recipient}");
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
    let builder = Brc20TransactionBuilder::new(private_key);
    let commit_tx = builder.build_commit_transaction(CreateCommitTransactionArgs {
        inputs,
        inscription: Brc20Op::transfer(ticker, amount),
        txin_script_pubkey: sender_address.script_pubkey(),
        leftovers_recipient: sender_address,
        commit_fee,
        reveal_fee,
    })?;
    debug!("commit transaction: {commit_tx:?}");

    let txid = if args.dry_run {
        commit_tx.tx.txid()
    } else {
        info!("broadcasting transaction: {}", commit_tx.tx.txid());
        broadcast_transaction(commit_tx.tx, network).await?
    };
    info!("Transaction broadcasted: {}", txid);

    debug!("getting reveal transaction...");
    let reveal_transaction = builder.build_reveal_transaction(RevealTransactionArgs {
        input: brc20::transaction::TxInput {
            id: txid,
            index: 0,
            amount: commit_tx.reveal_balance,
        },
        recipient_address: recipient,
        redeem_script: commit_tx.redeem_script,
    })?;
    debug!("reveal transaction: {reveal_transaction:?}");

    if !args.dry_run {
        info!("broadcasting transaction: {}", reveal_transaction.txid());
        let txid = broadcast_transaction(reveal_transaction, network).await?;
        info!("Transaction broadcasted: {}", txid);
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
    let tx_hex = hex::encode(bitcoin::consensus::serialize(&transaction).to_vec());
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
