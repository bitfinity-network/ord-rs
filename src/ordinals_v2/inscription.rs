pub mod brc20;
pub mod constants;
pub mod nft;

use crate::OrdResult;
use brc20::{Brc20, Brc20Deploy, Brc20Mint, Brc20Transfer};
use nft::Nft;

use serde::{Deserialize, Serialize};

/// Represents the type of digital artifact being inscribed.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub enum InscriptionType {
    /// Satoshis imbued with `deploy`, `mint`, and `transfer` functionalities,
    /// as well as token supply, simulating fungibility (e.g., like ERC20 tokens).
    Fungible { scribe: Brc20 },
    /// For now, we refer to all other inscriptions (i.e. non-BRC20 ones) as
    /// non-fungible (e.g., like ERC721 tokens).
    NonFungible { scribe: Nft },
}

impl InscriptionType {
    /// Encode `Self` as a JSON string.
    pub fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }
}

/// Represents a token standard.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub enum Protocol {
    Brc20 { func: Brc20Func },
    Nft,
}

/// Represents a BRC20 operation/function.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub enum Brc20Func {
    Deploy,
    Mint,
    Transfer,
}

pub fn parse(protocol: Protocol, data: InscriptionType) -> OrdResult<InscriptionType> {
    match protocol {
        Protocol::Brc20 { func } => match func {
            Brc20Func::Deploy => {
                let deploy = serde_json::from_str::<Brc20Deploy>(&data.encode()?)?;
                let deploy_op = Brc20::deploy(deploy.tick, deploy.max, deploy.lim, deploy.dec);
                Ok(InscriptionType::Fungible { scribe: deploy_op })
            }
            Brc20Func::Mint => {
                let mint = serde_json::from_str::<Brc20Mint>(&data.encode()?)?;
                let mint_op = Brc20::mint(mint.tick, mint.amt);
                Ok(InscriptionType::Fungible { scribe: mint_op })
            }
            Brc20Func::Transfer => {
                let transfer = serde_json::from_str::<Brc20Transfer>(&data.encode()?)?;
                let transfer_op = Brc20::transfer(transfer.tick, transfer.amt);
                Ok(InscriptionType::Fungible {
                    scribe: transfer_op,
                })
            }
        },
        Protocol::Nft => {
            let nft = serde_json::from_str::<Nft>(&data.encode()?)?;
            Ok(InscriptionType::NonFungible { scribe: nft })
        }
    }
}
