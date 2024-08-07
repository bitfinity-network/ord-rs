mod envelope;

use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
use bitcoin::Transaction;
use serde::{Deserialize, Serialize};

use self::envelope::ParsedEnvelope;
use crate::wallet::RedeemScriptPubkey;
use crate::{Brc20, Inscription, InscriptionId, InscriptionParseError, Nft, OrdError, OrdResult};

/// Encapsulates inscription parsing logic for both Ordinals and BRC20s.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OrdParser {
    /// Denotes a parsed [Nft] inscription.
    Ordinal(Nft),
    /// Denotes a parsed [Brc20] inscription.
    Brc20(Brc20),
}

impl OrdParser {
    /// Parses all inscriptions from a given transaction and categorizes them as either `Self::Brc20` or `Self::Ordinal`.
    ///
    /// This function extracts all inscription data from the transaction, attempts to parse each inscription,
    /// and returns a vector of categorized inscriptions with their corresponding IDs.
    ///
    /// # Errors
    ///
    /// Will return an error if any inscription data cannot be parsed correctly,
    /// or if no valid inscriptions are found in the transaction.
    pub fn parse_all(tx: &Transaction) -> OrdResult<Vec<(InscriptionId, Self)>> {
        let txid = tx.txid();

        ParsedEnvelope::from_transaction(tx)
            .into_iter()
            .map(|envelope| {
                let inscription_id = InscriptionId {
                    txid,
                    index: envelope.input,
                };

                let raw_body = envelope.payload.body.as_ref().ok_or_else(|| {
                    OrdError::InscriptionParser(InscriptionParseError::ParsedEnvelope(
                        "Empty payload body in envelope".to_string(),
                    ))
                })?;

                if let Some(brc20) = Self::parse_brc20(raw_body) {
                    Ok((inscription_id, Self::Brc20(brc20)))
                } else {
                    Ok((inscription_id, Self::Ordinal(envelope.payload)))
                }
            })
            .collect::<Result<Vec<(InscriptionId, Self)>, OrdError>>()
    }

    /// Parses a single inscription from a transaction at a specified index, returning the
    /// parsed inscription along with its ID.
    ///
    /// This method specifically targets one inscription identified by its index within the transaction's inputs.
    /// It extracts the inscription data, attempts to parse it, and categorizes it as either `Self::Brc20` or `Self::Ordinal`.
    ///
    /// # Errors
    ///
    /// Returns an error if the inscription data at the specified index cannot be parsed,
    /// if there is no data at the specified index, or if the data at the index does not contain a valid payload.
    pub fn parse_one(tx: &Transaction, index: usize) -> OrdResult<(InscriptionId, Self)> {
        let envelope = ParsedEnvelope::from_transaction_input(tx, index).ok_or_else(|| {
            OrdError::InscriptionParser(InscriptionParseError::ParsedEnvelope(
                "No data found in envelope at specified index".to_string(),
            ))
        })?;

        let raw_body = envelope.payload.body.as_ref().ok_or_else(|| {
            OrdError::InscriptionParser(InscriptionParseError::ParsedEnvelope(
                "Empty payload body in envelope".to_string(),
            ))
        })?;

        let inscription_id = InscriptionId {
            txid: tx.txid(),
            index: envelope.input,
        };

        if let Some(brc20) = Self::parse_brc20(raw_body) {
            Ok((inscription_id, Self::Brc20(brc20)))
        } else {
            Ok((inscription_id, Self::Ordinal(envelope.payload)))
        }
    }

    /// Attempts to parse the raw data as a BRC20 inscription.
    /// Returns `Some(Brc20)` if successful, otherwise `None`.
    fn parse_brc20(raw_body: &[u8]) -> Option<Brc20> {
        serde_json::from_slice::<Brc20>(raw_body).ok()
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

impl TryFrom<&OrdParser> for Nft {
    type Error = OrdError;

    fn try_from(parser: &OrdParser) -> Result<Self, Self::Error> {
        match parser {
            OrdParser::Ordinal(nft) => Ok(nft.clone()),
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

impl TryFrom<&OrdParser> for Brc20 {
    type Error = OrdError;

    fn try_from(parser: &OrdParser) -> Result<Self, Self::Error> {
        match parser {
            OrdParser::Brc20(brc20) => Ok(brc20.clone()),
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
    use crate::utils::test_utils::get_transaction_by_id;

    #[tokio::test]
    async fn ord_parser_should_parse_one() {
        let transaction = get_transaction_by_id(
            "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735",
            Network::Bitcoin,
        )
        .await
        .unwrap();

        let (inscription_id, parsed_inscription) = OrdParser::parse_one(&transaction, 0).unwrap();

        assert_eq!(inscription_id.index, 0);
        assert_eq!(inscription_id.txid, transaction.txid());

        let brc20 = Brc20::try_from(parsed_inscription).unwrap();
        assert_eq!(
            brc20,
            Brc20::deploy("ordi", 21000000, Some(1000), None, None)
        );
    }

    #[tokio::test]
    async fn ord_parser_should_parse_valid_brc20_inscription_mainnet() {
        let transaction = get_transaction_by_id(
            "b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735",
            Network::Bitcoin,
        )
        .await
        .unwrap();

        let parsed_data = OrdParser::parse_all(&transaction).unwrap();
        let (parsed_brc20, brc20_iid) = (&parsed_data[0].1, parsed_data[0].0);

        assert_eq!(brc20_iid.txid, transaction.txid());
        assert_eq!(brc20_iid.index, 0);

        let brc20 = Brc20::try_from(parsed_brc20).unwrap();
        assert_eq!(
            brc20,
            Brc20::deploy("ordi", 21000000, Some(1000), None, None)
        );
    }

    #[tokio::test]
    async fn ord_parser_should_not_parse_a_non_brc20_inscription_mainnet() {
        let transaction = get_transaction_by_id(
            "37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8",
            bitcoin::Network::Bitcoin,
        )
        .await
        .unwrap();

        assert!(OrdParser::parse_all(&transaction).unwrap().is_empty());
    }

    #[tokio::test]
    async fn ord_parser_should_not_parse_a_non_brc20_inscription_testnet() {
        let transaction = get_transaction_by_id(
            "5b8ee749df4a3cfc37344892a97f1819fac80fb2432289a474dc0f0fd3711208",
            bitcoin::Network::Testnet,
        )
        .await
        .unwrap();

        assert!(OrdParser::parse_all(&transaction).unwrap().is_empty());
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

        let parsed_data = OrdParser::parse_all(&transaction).unwrap();
        let (parsed_brc20, brc20_iid) = (&parsed_data[0].1, parsed_data[0].0);

        assert_eq!(brc20_iid.txid, transaction.txid());
        assert_eq!(brc20_iid.index, 0);

        let brc20 = Brc20::try_from(parsed_brc20).unwrap();

        assert_eq!(
            brc20,
            Brc20::deploy("kobp", 1000, Some(10), Some(8), Some(true))
        );
    }

    #[test]
    fn ord_parser_should_parse_valid_multiple_inscriptions_from_a_single_input_witness() {
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
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice(b"Hello, world!")
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

        let parsed_data = OrdParser::parse_all(&transaction).unwrap();

        let (parsed_brc20, brc20_iid) = (&parsed_data[0].1, parsed_data[0].0);
        assert_eq!(brc20_iid.txid, transaction.txid());
        assert_eq!(brc20_iid.index, 0);

        assert_eq!(
            Brc20::try_from(parsed_brc20).unwrap(),
            Brc20::deploy("kobp", 1000, Some(10), Some(8), Some(true))
        );

        let (parsed_nft, nft_iid) = (&parsed_data[1].1, parsed_data[1].0);
        assert_eq!(nft_iid.txid, transaction.txid());
        assert_eq!(nft_iid.index, 0);

        let nft = Nft::try_from(parsed_nft).unwrap();
        assert_eq!(nft.content_type().unwrap(), "text/plain;charset=utf-8");
        assert_eq!(nft.body().unwrap(), "Hello, world!");
    }

    #[test]
    fn ord_parser_should_parse_valid_multiple_inscriptions_from_multiple_input_witnesses() {
        let brc20 = br#"{
        "p": "brc-20",
        "op": "deploy",
        "tick": "kobp",
        "max": "1000",
        "lim": "10",
        "dec": "8",
        "self_mint": "true"
    }"#;

        let brc20_script = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice::<&PushBytes>(brc20.as_slice().try_into().unwrap())
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        let nft_script = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice(b"Hello, world!")
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        let brc20_witness = Witness::from_slice(&[brc20_script.into_bytes(), Vec::new()]);
        let nft_witness = Witness::from_slice(&[nft_script.into_bytes(), Vec::new()]);

        let transaction = Transaction {
            version: Version::ONE,
            lock_time: LockTime::ZERO,
            input: vec![
                TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: brc20_witness,
                },
                TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: nft_witness,
                },
            ],
            output: Vec::new(),
        };

        let parsed_data = OrdParser::parse_all(&transaction).unwrap();

        let (brc20_iid, parsed_brc20) = (&parsed_data[0].0, &parsed_data[0].1);
        assert_eq!(brc20_iid.txid, transaction.txid());
        assert_eq!(brc20_iid.index, 0);
        assert_eq!(
            Brc20::try_from(parsed_brc20).unwrap(),
            Brc20::deploy("kobp", 1000, Some(10), Some(8), Some(true))
        );

        let (nft_iid, parsed_nft) = (&parsed_data[1].0, &parsed_data[1].1);
        assert_eq!(nft_iid.txid, transaction.txid());
        assert_eq!(nft_iid.index, 1);
        let nft = Nft::try_from(parsed_nft).unwrap();
        assert_eq!(nft.content_type().unwrap(), "text/plain;charset=utf-8");
        assert_eq!(nft.body().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_should_parse_bitcoin_nft() {
        let tx: MempoolApiTx = reqwest::get("https://mempool.space/api/tx/276e858872a00b1b07312b093c5f2c1fcdd5a2d9379b9ec47d4b91be17aeaf8d")
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        // make transaction
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx
                .vin
                .into_iter()
                .map(|vin| TxIn {
                    previous_output: OutPoint::null(), // not used
                    script_sig: ScriptBuf::new(),      // not used
                    sequence: Sequence::ZERO,          // not used
                    witness: Witness::from_slice(
                        vin.witness
                            .iter()
                            .map(|w| hex::decode(w).unwrap())
                            .collect::<Vec<Vec<u8>>>()
                            .as_slice(),
                    ),
                })
                .collect::<Vec<_>>(),
            output: vec![], // we don't need outputs for this test
        };

        let nft = OrdParser::parse_all(&tx)
            .unwrap()
            .into_iter()
            .find(|(_, ins)| matches!(ins, OrdParser::Ordinal(_)))
            .unwrap()
            .1;
        let nft = Nft::try_from(nft).unwrap();
        assert_eq!(nft.content_type().unwrap(), "image/gif");
        assert_eq!(nft.body.unwrap().len(), 592);
    }

    #[derive(Debug, Clone, Deserialize)]
    struct MempoolApiTx {
        vin: Vec<MempoolApiVin>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct MempoolApiVin {
        witness: Vec<String>,
    }
}
