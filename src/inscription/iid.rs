//! Implements `InscriptionId`

use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, Txid};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::InscriptionParseError;
use crate::{OrdError, OrdResult};

#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq, PartialOrd, Ord)]
pub struct InscriptionId {
    pub txid: Txid,
    pub index: u32,
}

impl Default for InscriptionId {
    fn default() -> Self {
        Self {
            txid: Txid::all_zeros(),
            index: 0,
        }
    }
}

impl InscriptionId {
    /// Retrieves the raw InscriptionId bytes.
    pub fn get_raw(&self) -> Vec<u8> {
        let index = self.index.to_le_bytes();
        let mut index_slice = index.as_slice();

        while index_slice.last().copied() == Some(0) {
            index_slice = &index_slice[0..index_slice.len() - 1];
        }

        self.txid
            .to_byte_array()
            .iter()
            .chain(index_slice)
            .copied()
            .collect()
    }

    /// Creates a new InscriptionId from a transaction's output reference.
    pub fn from_outpoint(outpoint: OutPoint) -> Self {
        Self {
            txid: outpoint.txid,
            index: outpoint.vout,
        }
    }

    /// Creates a new InscriptionId from its string representation.
    pub fn parse_from_str(iid: &str) -> OrdResult<Self> {
        Self::from_str(iid).map_err(OrdError::InscriptionParser)
    }
}

impl std::fmt::Display for InscriptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}i{}", self.txid, self.index)
    }
}

impl Serialize for InscriptionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for InscriptionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        DeserializeFromStr::with(deserializer)
    }
}

struct DeserializeFromStr<T: FromStr>(pub T);

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

impl FromStr for InscriptionId {
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

#[cfg(test)]
fn set_using(n: u32) -> InscriptionId {
    let hex = format!("{n:x}");

    if hex.is_empty() || hex.len() > 1 {
        panic!();
    }

    format!("{}i{n}", hex.repeat(64)).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_matches {
        ($expression:expr, $( $pattern:pat_param )|+ $( if $guard:expr )? $(,)?) => {
          match $expression {
            $( $pattern )|+ $( if $guard )? => {}
            left => panic!(
              "assertion failed: (left ~= right)\n  left: `{:?}`\n right: `{}`",
              left,
              stringify!($($pattern)|+ $(if $guard)?)
            ),
          }
        }
      }

    fn txid(n: u64) -> Txid {
        let hex = format!("{n:x}");

        if hex.is_empty() || hex.len() > 1 {
            panic!();
        }

        hex.repeat(64).parse().unwrap()
    }

    #[test]
    fn display() {
        assert_eq!(
            set_using(1).to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111i1",
        );
        assert_eq!(
            InscriptionId {
                txid: txid(1),
                index: 0,
            }
            .to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111i0",
        );
        assert_eq!(
            InscriptionId {
                txid: txid(1),
                index: 0xFFFFFFFF,
            }
            .to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111i4294967295",
        );
    }

    #[test]
    fn from_str() {
        assert_eq!(
            "1111111111111111111111111111111111111111111111111111111111111111i1"
                .parse::<InscriptionId>()
                .unwrap(),
            set_using(1),
        );
        assert_eq!(
            "1111111111111111111111111111111111111111111111111111111111111111i4294967295"
                .parse::<InscriptionId>()
                .unwrap(),
            InscriptionId {
                txid: txid(1),
                index: 0xFFFFFFFF,
            },
        );
        assert_eq!(
            "1111111111111111111111111111111111111111111111111111111111111111i4294967295"
                .parse::<InscriptionId>()
                .unwrap(),
            InscriptionId {
                txid: txid(1),
                index: 0xFFFFFFFF,
            },
        );
    }

    #[test]
    fn from_str_bad_character() {
        assert_matches!(
            "→".parse::<InscriptionId>(),
            Err(InscriptionParseError::Character('→')),
        );
    }

    #[test]
    fn from_str_bad_length() {
        assert_matches!(
            "foo".parse::<InscriptionId>(),
            Err(InscriptionParseError::InscriptionIdLength(3))
        );
    }

    #[test]
    fn from_str_bad_separator() {
        assert_matches!(
            "0000000000000000000000000000000000000000000000000000000000000000x0"
                .parse::<InscriptionId>(),
            Err(InscriptionParseError::CharacterSeparator('x')),
        );
    }

    #[test]
    fn from_str_bad_index() {
        assert_matches!(
            "0000000000000000000000000000000000000000000000000000000000000000ifoo"
                .parse::<InscriptionId>(),
            Err(InscriptionParseError::Index(_)),
        );
    }

    #[test]
    fn from_str_bad_txid() {
        assert_matches!(
            "x000000000000000000000000000000000000000000000000000000000000000i0"
                .parse::<InscriptionId>(),
            Err(InscriptionParseError::Txid(_)),
        );
    }
}
