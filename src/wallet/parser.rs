mod envelope;

use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
use bitcoin::Transaction;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::envelope::ParsedEnvelope;
use crate::wallet::RedeemScriptPubkey;
use crate::{Brc20, Inscription, InscriptionParseError, Nft, OrdError, OrdResult};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OrdParser {
    Ordinal(Nft),
    Brc20(Brc20),
}

impl OrdParser {
    /// Parses all inscriptions from a given transaction and categorizes them as either `Self::Brc20` or `Self::Ordinal`.
    ///
    /// This function extracts all inscription data from the transaction, attempts to parse each inscription,
    /// and returns a vector of categorized inscriptions.
    ///
    /// # Errors
    ///
    /// Will return an error if any inscription data cannot be parsed correctly,
    /// or if no valid inscriptions are found in the transaction.
    pub fn parse_all(tx: &Transaction) -> OrdResult<Vec<Self>> {
        let data = ParsedEnvelope::from_transaction(tx)
            .into_iter()
            .map(|envelope| {
                envelope.payload.body.ok_or(OrdError::InscriptionParser(
                    InscriptionParseError::ParsedEnvelope(
                        "Empty payload body in envelope".to_string(),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Self::from_raw(data)
    }

    /// Parses a single inscription from a transaction at a specified index.
    ///
    /// This method specifically targets one inscription identified by its index within the transaction's inputs.
    /// It extracts the inscription data, attempts to parse it, and categorizes it as either `Self::Brc20` or `Self::Ordinal`.
    ///
    /// # Errors
    ///
    /// Returns an error if the inscription data at the specified index cannot be parsed,
    /// if there is no data at the specified index, or if the data at the index does not contain a valid payload.
    pub fn parse_one(tx: &Transaction, index: usize) -> OrdResult<Self> {
        let data = ParsedEnvelope::from_transaction_input(tx, index)
            .ok_or_else(|| {
                OrdError::InscriptionParser(InscriptionParseError::ParsedEnvelope(
                    "No data found in envelope at specified index".to_string(),
                ))
            })?
            .payload
            .body
            .ok_or_else(|| {
                OrdError::InscriptionParser(InscriptionParseError::ParsedEnvelope(
                    "Empty payload body in envelope".to_string(),
                ))
            })?;

        Self::from_raw(vec![data])
            .map(|mut all| all.pop().expect("Expected at least one inscription"))
    }

    /// Takes a list of inscription data, attempts to parse them, and
    /// categorize each of them as either `Self::Brc20` or `Self::Ordinal`.
    ///
    /// Returns a list of parsed inscription data, or an error if deserialization fails.
    fn from_raw(raw_inscriptions: Vec<Vec<u8>>) -> OrdResult<Vec<Self>> {
        raw_inscriptions
            .into_iter()
            .map(|inscription| Self::categorize(&inscription))
            .collect()
    }

    fn categorize(raw_inscription: &[u8]) -> OrdResult<Self> {
        match serde_json::from_slice::<Value>(raw_inscription) {
            Ok(value) => {
                if value.get("p").is_some()
                    && value.get("op").is_some()
                    && value.get("tick").is_some()
                {
                    let brc20: Brc20 = serde_json::from_value(value).map_err(OrdError::Codec)?;
                    Ok(Self::Brc20(brc20))
                } else {
                    let nft: Nft = serde_json::from_value(value).map_err(OrdError::Codec)?;
                    Ok(Self::Ordinal(nft))
                }
            }
            Err(err) => Err(OrdError::Codec(err)),
        }
    }
}

impl From<Brc20> for OrdParser {
    fn from(inscription: Brc20) -> Self {
        Self::Brc20(inscription)
    }
}

impl From<Nft> for OrdParser {
    fn from(inscription: Nft) -> Self {
        Self::Ordinal(inscription)
    }
}

impl TryFrom<OrdParser> for Nft {
    type Error = OrdError;

    fn try_from(parser: OrdParser) -> Result<Self, Self::Error> {
        match parser {
            OrdParser::Ordinal(nft) => Ok(nft),
            _ => Err(OrdError::InscriptionParser(
                InscriptionParseError::NotOrdinal,
            )),
        }
    }
}

impl TryFrom<OrdParser> for Brc20 {
    type Error = OrdError;

    fn try_from(parser: OrdParser) -> Result<Self, Self::Error> {
        match parser {
            OrdParser::Brc20(brc20) => Ok(brc20),
            _ => Err(OrdError::InscriptionParser(InscriptionParseError::NotBrc20)),
        }
    }
}

impl Inscription for OrdParser {
    fn content_type(&self) -> String {
        match self {
            Self::Brc20(inscription) => inscription.content_type(),
            Self::Ordinal(inscription) => Inscription::content_type(inscription),
        }
    }

    fn data(&self) -> OrdResult<PushBytesBuf> {
        match self {
            Self::Brc20(inscription) => inscription.data(),
            Self::Ordinal(inscription) => inscription.data(),
        }
    }

    fn generate_redeem_script(
        &self,
        builder: ScriptBuilder,
        pubkey: RedeemScriptPubkey,
    ) -> OrdResult<ScriptBuilder> {
        match self {
            Self::Brc20(inscription) => inscription.generate_redeem_script(builder, pubkey),
            Self::Ordinal(inscription) => inscription.generate_redeem_script(builder, pubkey),
        }
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::absolute::LockTime;
    use bitcoin::script::{Builder as ScriptBuilder, PushBytes};
    use bitcoin::transaction::Version;
    use bitcoin::{opcodes, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, Witness};

    use super::*;
    use crate::inscription::nft::nft_tests::create_nft;
    use crate::utils::test_utils::get_transaction_by_id;

    #[tokio::test]
    async fn ord_parser_should_parse_valid_brc20_inscription_mainnet() {
        let transaction = get_transaction_by_id(
            "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735",
            Network::Bitcoin,
        )
        .await
        .unwrap();

        let parsed_brc20 = OrdParser::parse_all(&transaction).unwrap()[0].clone();
        let parsed_brc20 = Brc20::try_from(parsed_brc20).unwrap();
        let brc20 = Brc20::deploy("ordi", 21000000, Some(1000), None, None);

        assert_eq!(parsed_brc20, brc20);
    }

    #[tokio::test]
    async fn ord_parser_should_not_parse_a_non_brc20_inscription_mainnet() {
        let transaction = get_transaction_by_id(
            "37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8",
            bitcoin::Network::Bitcoin,
        )
        .await
        .unwrap();

        let parse_result = OrdParser::parse_all(&transaction).unwrap();
        assert!(parse_result.is_empty());
    }

    #[tokio::test]
    async fn ord_parser_should_not_parse_a_non_brc20_inscription_testnet() {
        let transaction = get_transaction_by_id(
            "5b8ee749df4a3cfc37344892a97f1819fac80fb2432289a474dc0f0fd3711208",
            bitcoin::Network::Testnet,
        )
        .await
        .unwrap();

        let parse_result = OrdParser::parse_all(&transaction).unwrap();
        assert!(parse_result.is_empty());
    }

    #[test]
    fn ord_parser_should_return_a_valid_brc20_from_raw_transaction_data() {
        let brc20 = br#"{
            "p": "brc-20",
            "op": "deploy",
            "tick": "kobp",
            "max": "1000",
            "lim": "10",
            "dec": "8",
            "self_mint": "true"
        }"#;

        let script = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice::<&PushBytes>(brc20.as_slice().try_into().unwrap())
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        let witnesses = &[Witness::from_slice(&[script.into_bytes(), Vec::new()])];

        let transaction = Transaction {
            version: Version::ONE,
            lock_time: LockTime::ZERO,
            input: witnesses
                .iter()
                .map(|witness| TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: witness.clone(),
                })
                .collect(),
            output: Vec::new(),
        };

        let parsed_brc20 = OrdParser::parse_all(&transaction).unwrap()[0].clone();
        let brc20 = Brc20::try_from(parsed_brc20).unwrap();

        assert_eq!(
            brc20,
            Brc20::deploy("kobp", 1000, Some(10), Some(8), Some(true))
        );
    }

    #[test]
    fn ord_parser_should_parse_different_valid_inscription_types_from_raw_bytes() {
        let brc20_data = br#"{
            "p": "brc-20",
            "op": "deploy",
            "tick": "ordi",
            "max": "21000000",
            "lim": "1000",
            "dec": "8",
            "self_mint": "false"
        }"#;

        let ordinal_data = create_nft("text/plain", "Hello, world!").encode().unwrap();

        let inscriptions = vec![ordinal_data.as_bytes().to_vec(), brc20_data.to_vec()];

        assert!(matches!(
            OrdParser::from_raw(inscriptions.clone()).unwrap()[0],
            OrdParser::Ordinal(_)
        ));
        assert!(matches!(
            OrdParser::from_raw(inscriptions).unwrap()[1],
            OrdParser::Brc20(_)
        ));
    }
}
