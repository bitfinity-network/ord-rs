use std::str::FromStr;

use argh::FromArgs;
use bitcoin::opcodes::all::{OP_CHECKSIG, OP_ENDIF, OP_IF};
use bitcoin::opcodes::{OP_0, OP_FALSE};
use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Amount, Network, PrivateKey, ScriptBuf, Transaction, Txid};
use brc20::transaction::RevealTransactionArgs;
use brc20::{Brc20Op, Brc20TransactionBuilder};
use log::{debug, info};

#[derive(FromArgs, Debug)]
#[argh(description = "Transfer BRC20 tokens")]
struct Args {
    #[argh(option, short = 't')]
    /// to address (e.g. tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86)
    to: String,

    #[argh(option, short = 'T')]
    /// ticker
    ticker: String,

    #[argh(option, short = 'a')]
    /// amount
    amount: u64,

    #[argh(option, short = 'i')]
    /// commit tx id
    input_tx: String,

    #[argh(option, short = 'r')]
    /// reveal fee
    reveal_fee: u64,

    #[argh(option, short = 'p')]
    /// private key
    private_key: String,

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

    let network = match args.network.as_str() {
        "testnet" | "test" => Network::Testnet,
        "mainnet" | "prod" => Network::Bitcoin,
        _ => panic!("invalid network"),
    };

    let input_tx = Txid::from_str(&args.input_tx)?;

    let recipient = Address::from_str(&args.to)?.require_network(network)?;
    debug!("recipient: {recipient}");
    let ticker = args.ticker;
    let amount = args.amount;
    let private_key = PrivateKey::from_wif(&args.private_key)?;
    let public_key = private_key.public_key(&Secp256k1::new());
    let sender_address = Address::p2wpkh(&public_key, network).unwrap();
    debug!("sender address: {sender_address}");

    let inscription = Brc20Op::transfer(ticker, amount);
    let redeem_script = redeem_script(&private_key, inscription.clone())?;
    debug!("redeem_script: {redeem_script}");
    debug!(
        "redeem script hex: {}",
        hex::encode(redeem_script.as_bytes())
    );

    let builder = Brc20TransactionBuilder::new(private_key);

    debug!("getting reveal transaction...");
    let reveal_transaction = builder.build_reveal_transaction(RevealTransactionArgs {
        input: brc20::transaction::TxInput {
            id: input_tx,
            index: 0,
            amount: Amount::from_sat(args.reveal_fee),
        },
        recipient_address: recipient,
        redeem_script,
    })?;
    debug!("reveal transaction: {reveal_transaction:?}");

    if !args.dry_run {
        info!("broadcasting transaction: {}", reveal_transaction.txid());
        let txid = broadcast_transaction(reveal_transaction, network).await?;
        info!("Transaction broadcasted: {}", txid);
    }

    Ok(())
}

fn redeem_script(private_key: &PrivateKey, inscription: Brc20Op) -> anyhow::Result<ScriptBuf> {
    let public_key = private_key.public_key(&Secp256k1::new());
    let encoded_inscription = bytes_to_push_bytes(inscription.encode()?.as_bytes())?;

    Ok(ScriptBuilder::new()
        .push_key(&public_key)
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(b"ord")
        .push_slice(bytes_to_push_bytes(&[0x01])?.as_push_bytes())
        .push_slice(b"text/plain;charset=utf-8") // NOTE: YES, IT'S CORRECT, DON'T ASK!!! It's not json for some reasons
        .push_opcode(OP_0)
        .push_slice(encoded_inscription.as_push_bytes())
        .push_opcode(OP_ENDIF)
        .into_script())
}

pub fn bytes_to_push_bytes(bytes: &[u8]) -> anyhow::Result<PushBytesBuf> {
    let mut push_bytes = PushBytesBuf::with_capacity(bytes.len());
    push_bytes.extend_from_slice(bytes)?;

    Ok(push_bytes)
}

async fn broadcast_transaction(transaction: Transaction, network: Network) -> anyhow::Result<Txid> {
    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        Network::Bitcoin | _ => "",
    };

    let url = format!("https://blockstream.info{network_str}/api/tx");
    let tx_hex = hex::encode(&bitcoin::consensus::serialize(&transaction));
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
