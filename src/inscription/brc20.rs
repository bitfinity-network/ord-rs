use std::str::FromStr;

use bitcoin::script::PushBytesBuf;
use serde_with::{serde_as, DisplayFromStr};

use crate::inscription::Inscription;
use crate::utils::bytes_to_push_bytes;
use crate::{InscriptionParseError, OrdError, OrdResult};

const PROTOCOL: &str = "brc-20";

/// BRC-20 operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Brc20 {
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

impl Brc20 {
    /// Create a new BRC-20 deploy operation
    pub fn deploy(tick: impl ToString, max: u64, lim: Option<u64>, dec: Option<u64>) -> Self {
        Self::Deploy(Brc20Deploy {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            max,
            lim,
            dec,
        })
    }

    /// Create a new BRC-20 mint operation
    pub fn mint(tick: impl ToString, amt: u64) -> Self {
        Self::Mint(Brc20Mint {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            amt,
        })
    }

    /// Create a new BRC-20 transfer operation
    pub fn transfer(tick: impl ToString, amt: u64) -> Self {
        Self::Transfer(Brc20Transfer {
            protocol: PROTOCOL.to_string(),
            tick: tick.to_string(),
            amt,
        })
    }

    /// Encode the BRC-20 operation as a JSON string
    pub fn encode(&self) -> OrdResult<String> {
        serde_json::to_string(self).map_err(OrdError::from)
    }
}

impl FromStr for Brc20 {
    type Err = OrdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(OrdError::from)
    }
}

impl Inscription for Brc20 {
    fn content_type(&self) -> String {
        "text/plain;charset=utf-8".to_string()
    }

    fn data(&self) -> OrdResult<PushBytesBuf> {
        bytes_to_push_bytes(self.encode()?.as_bytes())
    }

    fn parse(data: &[u8]) -> OrdResult<Self>
    where
        Self: Sized,
    {
        let s = String::from_utf8(data.to_vec())
            .map_err(|_| OrdError::InscriptionParser(InscriptionParseError::BadDataSyntax))?;
        let inscription = serde_json::from_str(&s).map_err(OrdError::from)?;

        Ok(inscription)
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
        let deploy: Brc20 = serde_json::from_str(
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
            Brc20::Deploy(Brc20Deploy {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                max: 21000000,
                lim: Some(1000),
                dec: Some(8)
            })
        );

        let deploy: Brc20 = serde_json::from_str(
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
            Brc20::Deploy(Brc20Deploy {
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
        let mint: Brc20 = serde_json::from_str(
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
            Brc20::Mint(Brc20Mint {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                amt: 1000
            })
        );
    }

    #[test]
    fn test_should_decode_transfer() {
        let transfer: Brc20 = serde_json::from_str(
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
            Brc20::Transfer(Brc20Transfer {
                protocol: "brc-20".to_string(),
                tick: "ordi".to_string(),
                amt: 100
            })
        );
    }

    #[test]
    fn test_should_encode_and_decode() {
        let op = Brc20::Transfer(Brc20Transfer {
            protocol: "brc-20".to_string(),
            tick: "ordi".to_string(),
            amt: 100,
        });

        let s = op.encode().unwrap();

        assert_eq!(Brc20::from_str(&s).unwrap(), op);
    }
}
