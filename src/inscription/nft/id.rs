use crate::error::InscriptionParseError;

use bitcoin::{hashes::Hash, Txid};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq, PartialOrd, Ord)]
pub struct NftId {
    pub txid: Txid,
    pub index: u32,
}

impl NftId {
    pub fn value(self) -> Vec<u8> {
        let index = self.index.to_le_bytes();
        let mut index_slice = index.as_slice();

        while index_slice.last().copied() == Some(0) {
            index_slice = &index_slice[0..index_slice.len() - 1];
        }

        self.txid.to_byte_array().iter().chain(index_slice).copied().collect()
    }
}

pub fn inscription_id(n: u32) -> NftId {
    let hex = format!("{n:x}");

    if hex.is_empty() || hex.len() > 1 {
        panic!();
    }

    format!("{}i{n}", hex.repeat(64)).parse().unwrap()
}

impl Default for NftId {
    fn default() -> Self {
        Self {
            txid: Txid::all_zeros(),
            index: 0,
        }
    }
}

impl std::fmt::Display for NftId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}i{}", self.txid, self.index)
    }
}

impl Serialize for NftId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for NftId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        DeserializeFromStr::with(deserializer)
    }
}

pub struct DeserializeFromStr<T: FromStr>(pub T);

impl<'de, T: FromStr> DeserializeFromStr<T>
where
    T::Err: std::fmt::Display,
{
    pub fn with<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DeserializeFromStr::<T>::deserialize(deserializer)?.0)
    }
}

impl<'de, T: FromStr> Deserialize<'de> for DeserializeFromStr<T>
where
    T::Err: std::fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(
            FromStr::from_str(&String::deserialize(deserializer)?)
                .map_err(serde::de::Error::custom)?,
        ))
    }
}

impl FromStr for NftId {
    type Err = InscriptionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(char) = s.chars().find(|char| !char.is_ascii()) {
            return Err(InscriptionParseError::Character(char));
        }

        const TXID_LEN: usize = 64;
        const MIN_LEN: usize = TXID_LEN + 2;

        if s.len() < MIN_LEN {
            return Err(InscriptionParseError::InscriptionIdLength(s.len()));
        }

        let txid = &s[..TXID_LEN];

        let separator = s.chars().nth(TXID_LEN).unwrap();

        if separator != 'i' {
            return Err(InscriptionParseError::CharacterSeparator(separator));
        }

        let vout = &s[TXID_LEN + 1..];

        Ok(Self {
            txid: txid.parse().map_err(InscriptionParseError::Txid)?,
            index: vout.parse().map_err(InscriptionParseError::Index)?,
        })
    }
}
