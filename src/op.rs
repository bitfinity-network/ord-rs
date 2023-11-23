use std::str::FromStr;

use serde_with::{serde_as, DisplayFromStr};

use crate::{Brc20Error, Brc20Result};

const PROTOCOL: &str = "brc-20";

/// BRC-20 operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Brc20Op {
    /// Deploy a BRC-20 token
    #[serde(rename = "deploy")]
    Deploy(Brc20Deploy),
    /// Mint BRC-20 tokens
    #[serde(rename = "mint")]
    Mint(Brc20Mint),
    /// Transfer BRC-20 tokens
    #[serde(rename = "transfer")]
    Transfer(Brc20Transfer),
}

impl Brc20Op {
    pub fn deploy(tick: impl ToString, max: u64, lim: Option<u64>, dec: Option<u64>) -> Self {
        Self::Deploy(Brc20Deploy {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            max,
            lim,
            dec,
        })
    }

    pub fn mint(tick: impl ToString, amt: u64) -> Self {
        Self::Mint(Brc20Mint {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            amt,
        })
    }

    pub fn transfer(tick: impl ToString, amt: u64) -> Self {
        Self::Transfer(Brc20Transfer {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            amt,
        })
    }

    /// Encode the BRC-20 operation as a JSON string
    pub fn encode(&self) -> Brc20Result<String> {
        serde_json::to_string(self).map_err(Brc20Error::from)
    }
}

impl FromStr for Brc20Op {
    type Err = Brc20Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Brc20Error::from)
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brc20Deploy {
    #[serde(rename = "p")]
    protocol: String,
    pub tick: String,
    #[serde_as(as = "DisplayFromStr")]
    pub max: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub lim: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub dec: Option<u64>,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brc20Mint {
    #[serde(rename = "p")]
    protocol: String,
    pub tick: String,
    #[serde_as(as = "DisplayFromStr")]
    pub amt: u64,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brc20Transfer {
    #[serde(rename = "p")]
    protocol: String,
    pub tick: String,
    #[serde_as(as = "DisplayFromStr")]
    pub amt: u64,
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_decode_deploy() {
        let deploy: Brc20Op = serde_json::from_str(
            r#"
            { 
                "p": "brc-20",
                "op": "deploy",
                "tick": "ordi",
                "max": "21000000",
                "lim": "1000",
                "dec": "8"
              }
            "#,
        )
        .unwrap();

        assert_eq!(
            deploy,
            Brc20Op::Deploy(Brc20Deploy {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                max: 21000000,
                lim: Some(1000),
                dec: Some(8)
            })
        );

        let deploy: Brc20Op = serde_json::from_str(
            r#"
            { 
                "p": "brc-20",
                "op": "deploy",
                "tick": "ordi",
                "max": "21000000"
              }
            "#,
        )
        .unwrap();

        assert_eq!(
            deploy,
            Brc20Op::Deploy(Brc20Deploy {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                max: 21000000,
                lim: None,
                dec: None
            })
        );
    }

    #[test]
    fn test_should_decode_mint() {
        let mint: Brc20Op = serde_json::from_str(
            r#"
            { 
                "p": "brc-20",
                "op": "mint",
                "tick": "ordi",
                "amt": "1000"
              }
            "#,
        )
        .unwrap();
        assert_eq!(
            mint,
            Brc20Op::Mint(Brc20Mint {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                amt: 1000
            })
        );
    }

    #[test]
    fn test_should_decode_transfer() {
        let transfer: Brc20Op = serde_json::from_str(
            r#"{ 
                "p": "brc-20",
                "op": "transfer",
                "tick": "ordi",
                "amt": "100"
              }
              "#,
        )
        .unwrap();
        assert_eq!(
            transfer,
            Brc20Op::Transfer(Brc20Transfer {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                amt: 100
            })
        );
    }

    #[test]
    fn test_should_encode_and_decode() {
        let op = Brc20Op::Transfer(Brc20Transfer {
            protocol: "brc-20".to_string(),
            tick: "ordi".to_string(),
            amt: 100,
        });

        let s = op.encode().unwrap();

        assert_eq!(Brc20Op::from_str(&s).unwrap(), op);
    }
}
