use bitcoin::{Amount, Network};

#[allow(dead_code)]
pub struct Fees {
    pub commit_fee: Amount,
    pub reveal_fee: Amount,
    pub utxo_fee: Amount,
}

pub fn calc_fees(network: Network) -> Fees {
    match network {
        Network::Bitcoin => Fees {
            commit_fee: Amount::from_sat(15_000),
            reveal_fee: Amount::from_sat(7_000),
            utxo_fee: Amount::from_sat(10_000),
        },
        Network::Testnet | Network::Regtest | Network::Signet => Fees {
            commit_fee: Amount::from_sat(2_500),
            reveal_fee: Amount::from_sat(4_700),
            utxo_fee: Amount::from_sat(3_000),
        },
        _ => panic!("unknown network"),
    }
}
