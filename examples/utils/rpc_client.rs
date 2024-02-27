use std::str::FromStr;
use std::time::Duration;

use bitcoin::{Amount, Network, Transaction, Txid};
use log::{debug, info};
use ord_rs::transaction::TxInput;

pub async fn broadcast_transaction(
    transaction: &Transaction,
    network: Network,
) -> anyhow::Result<Txid> {
    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        _ => "",
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

pub async fn sats_amount_from_tx_inputs(
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

pub async fn get_tx_by_hash(txid: &Txid, network: Network) -> anyhow::Result<ApiTransaction> {
    let network_str = match network {
        Network::Testnet => "/testnet",
        Network::Regtest => "/regtest",
        Network::Signet => "/signet",
        _ => "",
    };

    let url = format!("https://blockstream.info{network_str}/api/tx/{}", txid);
    let tx = reqwest::get(&url).await?.json().await?;
    Ok(tx)
}

#[allow(dead_code)]
pub async fn wait_for_tx(txid: &Txid, network: Network) -> anyhow::Result<()> {
    loop {
        info!("waiting for transaction to be confirmed...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        if get_tx_by_hash(txid, network).await.is_ok() {
            break;
        }
        debug!("retrying in 10 seconds...");
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiTransaction {
    vout: Vec<ApiVout>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiVout {
    value: u64,
}
