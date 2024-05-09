// The envelope-parsing logic is borrowed from
// https://github.com/ordinals/ord/blob/master/src/inscriptions/envelope.rs

use std::collections::BTreeMap;
use std::iter::Peekable;

use bitcoin::script::{
    Builder as ScriptBuilder, Error as ScriptError, Instruction, Instructions, PushBytesBuf,
};
use bitcoin::{opcodes, Script, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::constants::{
    CONTENT_ENCODING_TAG, CONTENT_TYPE_TAG, DELEGATE_TAG, METADATA_TAG, METAPROTOCOL_TAG,
    PARENT_TAG, POINTER_TAG, PROTOCOL_ID, RUNE_TAG,
};
use crate::wallet::RedeemScriptPubkey;
use crate::{Brc20, Inscription, InscriptionParseError, Nft, OrdError, OrdResult};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum InscriptionParser {
    Ordinal(Nft),
    Brc20(Brc20),
}

impl InscriptionParser {
    /// Parses all inscriptions from a given transaction and categorizes them as either Ordinal(NFT) or BRC20.
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
            .map(|envelope| envelope.payload.body.unwrap_or_default())
            .collect::<Vec<_>>();
        Self::inscription_types(data)
    }

    /// Parses a single inscription from a transaction at a specified index.
    ///
    /// This method specifically targets one inscription identified by its index within the transaction's inputs.
    /// It extracts the inscription data, attempts to parse it, and categorizes it as either an Ordinal(NFT) or a BRC20.
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

        Self::inscription_types(vec![data])
            .map(|mut all| all.pop().expect("Expected at least one inscription"))
    }

    /// Takes a list of inscription data and attempts to parse
    /// to determine if they're an Ordinal or a BRC20.
    ///
    /// # Examples
    ///
    /// ```
    /// use ord_rs::{InscriptionParser, Brc20, Nft, Inscription};
    ///
    /// let brc20_data = br#"{
    ///     "p": "brc-20",
    ///     "op": "deploy",
    ///     "tick": "ordi",
    ///     "max": "21000000",
    ///     "lim": "1000",
    ///     "dec": "8",
    ///     "self_mint": "false"
    /// }"#;
    ///
    /// fn create_nft(content_type: &str, body: impl AsRef<[u8]>) -> Nft {
    ///     Nft::new(Some(content_type.into()), Some(body.as_ref().into()))
    /// }
    ///
    /// let ordinal_data = create_nft("text/plain", "Hello, world!").encode().unwrap();
    ///
    /// let inscriptions = vec![ordinal_data.as_bytes().to_vec(), brc20_data.to_vec()];
    ///
    /// assert!(matches!(InscriptionParser::inscription_types(inscriptions.clone()).unwrap()[0], InscriptionParser::Ordinal(_)));
    /// assert!(matches!(InscriptionParser::inscription_types(inscriptions).unwrap()[1], InscriptionParser::Brc20(_)));
    /// ```
    pub fn inscription_types(data: Vec<Vec<u8>>) -> OrdResult<Vec<Self>> {
        data.into_iter()
            .map(|inscription| {
                let result: Result<Value, serde_json::Error> = serde_json::from_slice(&inscription);
                match result {
                    Ok(value) => {
                        if value.get("p").is_some()
                            && value.get("op").is_some()
                            && value.get("tick").is_some()
                        {
                            let brc20: Brc20 =
                                serde_json::from_value(value).map_err(OrdError::Codec)?;
                            Ok(Self::Brc20(brc20))
                        } else {
                            let nft: Nft =
                                serde_json::from_value(value).map_err(OrdError::Codec)?;
                            Ok(Self::Ordinal(nft))
                        }
                    }
                    Err(err) => Err(OrdError::Codec(err)),
                }
            })
            .collect()
    }
}

impl From<Brc20> for InscriptionParser {
    fn from(inscription: Brc20) -> Self {
        Self::Brc20(inscription)
    }
}

impl From<Nft> for InscriptionParser {
    fn from(inscription: Nft) -> Self {
        Self::Ordinal(inscription)
    }
}

impl Inscription for InscriptionParser {
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

type ParseResult<T> = std::result::Result<T, ScriptError>;
type RawEnvelope = Envelope<Vec<Vec<u8>>>;
type ParsedEnvelope = Envelope<Nft>;

#[derive(Debug, Default, PartialEq, Clone)]
struct Envelope<T> {
    input: u32,
    offset: u32,
    payload: T,
    pushnum: bool,
    stutter: bool,
}

impl ParsedEnvelope {
    fn from_transaction(transaction: &Transaction) -> Vec<Self> {
        RawEnvelope::from_transaction(transaction)
            .into_iter()
            .map(|envelope| envelope.into())
            .collect()
    }

    /// Fetch a single parsed envelope from a specific transaction input if it exists.
    fn from_transaction_input(transaction: &Transaction, index: usize) -> Option<Self> {
        transaction.input.get(index).and_then(|input| {
            input.witness.tapscript().and_then(|tapscript| {
                RawEnvelope::from_tapscript(tapscript, index)
                    .ok()
                    .and_then(|envelopes| envelopes.into_iter().next())
                    .map(|raw_envelope| raw_envelope.into())
            })
        })
    }
}

impl RawEnvelope {
    fn from_transaction(transaction: &Transaction) -> Vec<Self> {
        let mut envelopes = Vec::new();

        for (i, input) in transaction.input.iter().enumerate() {
            if let Some(tapscript) = input.witness.tapscript() {
                if let Ok(input_envelopes) = Self::from_tapscript(tapscript, i) {
                    envelopes.extend(input_envelopes);
                }
            }
        }

        envelopes
    }

    fn from_tapscript(tapscript: &Script, input: usize) -> ParseResult<Vec<Self>> {
        let mut envelopes = Vec::new();

        let mut instructions = tapscript.instructions().peekable();

        let mut stuttered = false;
        while let Some(instruction) = instructions.next().transpose()? {
            if instruction == Instruction::PushBytes((&[]).into()) {
                let (stutter, envelope) =
                    Self::from_instructions(&mut instructions, input, envelopes.len(), stuttered)?;
                if let Some(envelope) = envelope {
                    envelopes.push(envelope);
                } else {
                    stuttered = stutter;
                }
            }
        }

        Ok(envelopes)
    }

    fn accept(
        instructions: &mut Peekable<Instructions>,
        instruction: Instruction,
    ) -> ParseResult<bool> {
        if instructions.peek() == Some(&Ok(instruction)) {
            instructions.next().transpose()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn from_instructions(
        instructions: &mut Peekable<Instructions>,
        input: usize,
        offset: usize,
        stutter: bool,
    ) -> ParseResult<(bool, Option<Self>)> {
        if !Self::accept(instructions, Instruction::Op(opcodes::all::OP_IF))? {
            let stutter = instructions.peek() == Some(&Ok(Instruction::PushBytes((&[]).into())));
            return Ok((stutter, None));
        }

        if !Self::accept(instructions, Instruction::PushBytes((&PROTOCOL_ID).into()))? {
            let stutter = instructions.peek() == Some(&Ok(Instruction::PushBytes((&[]).into())));
            return Ok((stutter, None));
        }

        let mut pushnum = false;

        let mut payload = Vec::new();

        loop {
            match instructions.next().transpose()? {
                None => return Ok((false, None)),
                Some(Instruction::Op(opcodes::all::OP_ENDIF)) => {
                    return Ok((
                        false,
                        Some(Envelope {
                            input: input.try_into().unwrap(),
                            offset: offset.try_into().unwrap(),
                            payload,
                            pushnum,
                            stutter,
                        }),
                    ));
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_NEG1)) => {
                    pushnum = true;
                    payload.push(vec![0x81]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_1)) => {
                    pushnum = true;
                    payload.push(vec![1]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_2)) => {
                    pushnum = true;
                    payload.push(vec![2]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_3)) => {
                    pushnum = true;
                    payload.push(vec![3]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_4)) => {
                    pushnum = true;
                    payload.push(vec![4]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_5)) => {
                    pushnum = true;
                    payload.push(vec![5]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_6)) => {
                    pushnum = true;
                    payload.push(vec![6]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_7)) => {
                    pushnum = true;
                    payload.push(vec![7]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_8)) => {
                    pushnum = true;
                    payload.push(vec![8]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_9)) => {
                    pushnum = true;
                    payload.push(vec![9]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_10)) => {
                    pushnum = true;
                    payload.push(vec![10]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_11)) => {
                    pushnum = true;
                    payload.push(vec![11]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_12)) => {
                    pushnum = true;
                    payload.push(vec![12]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_13)) => {
                    pushnum = true;
                    payload.push(vec![13]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_14)) => {
                    pushnum = true;
                    payload.push(vec![14]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_15)) => {
                    pushnum = true;
                    payload.push(vec![15]);
                }
                Some(Instruction::Op(opcodes::all::OP_PUSHNUM_16)) => {
                    pushnum = true;
                    payload.push(vec![16]);
                }
                Some(Instruction::PushBytes(data)) => {
                    payload.push(data.as_bytes().to_vec());
                }
                Some(_) => return Ok((false, None)),
            }
        }
    }
}

impl From<RawEnvelope> for ParsedEnvelope {
    fn from(envelope: RawEnvelope) -> Self {
        let body = envelope
            .payload
            .iter()
            .enumerate()
            .position(|(i, push)| i % 2 == 0 && push.is_empty());

        let mut fields: BTreeMap<&[u8], Vec<&[u8]>> = BTreeMap::new();

        let mut incomplete_field = false;

        for item in envelope.payload[..body.unwrap_or(envelope.payload.len())].chunks(2) {
            match item {
                [key, value] => fields.entry(key).or_default().push(value),
                _ => incomplete_field = true,
            }
        }

        let duplicate_field = fields.iter().any(|(_key, values)| values.len() > 1);

        let content_encoding = remove_field(&mut fields, &CONTENT_ENCODING_TAG);
        let content_type = remove_field(&mut fields, &CONTENT_TYPE_TAG);
        let delegate = remove_field(&mut fields, &DELEGATE_TAG);
        let metadata = remove_and_concatenate_field(&mut fields, &METADATA_TAG);
        let metaprotocol = remove_field(&mut fields, &METAPROTOCOL_TAG);
        let parents = remove_array(&mut fields, PARENT_TAG);
        let pointer = remove_field(&mut fields, &POINTER_TAG);
        let rune = remove_field(&mut fields, &RUNE_TAG);

        let unrecognized_even_field = fields
            .keys()
            .any(|tag| tag.first().map(|lsb| lsb % 2 == 0).unwrap_or_default());

        Self {
            payload: Nft {
                body: body.map(|i| {
                    envelope.payload[i + 1..]
                        .iter()
                        .flatten()
                        .cloned()
                        .collect()
                }),
                metaprotocol,
                parents,
                delegate,
                content_encoding,
                content_type,
                duplicate_field,
                incomplete_field,
                metadata,
                pointer,
                unrecognized_even_field,
                rune,
            },
            input: envelope.input,
            offset: envelope.offset,
            pushnum: envelope.pushnum,
            stutter: envelope.stutter,
        }
    }
}

fn remove_field(fields: &mut BTreeMap<&[u8], Vec<&[u8]>>, field: &[u8]) -> Option<Vec<u8>> {
    let values = fields.get_mut(field)?;

    if values.is_empty() {
        None
    } else {
        let value = values.remove(0).to_vec();

        if values.is_empty() {
            fields.remove(field);
        }

        Some(value)
    }
}

fn remove_and_concatenate_field(
    fields: &mut BTreeMap<&[u8], Vec<&[u8]>>,
    field: &[u8],
) -> Option<Vec<u8>> {
    let value = fields.remove(field)?;

    if value.is_empty() {
        None
    } else {
        Some(value.into_iter().flatten().cloned().collect())
    }
}

fn remove_array(fields: &mut BTreeMap<&[u8], Vec<&[u8]>>, tag: [u8; 1]) -> Vec<Vec<u8>> {
    fields
        .remove(tag.as_slice())
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use bitcoin::absolute::LockTime;
    use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
    use bitcoin::transaction::Version;
    use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};

    use super::*;
    use crate::inscription::nft::nft_tests::create_nft;

    fn envelope(payload: &[&[u8]]) -> Witness {
        let mut builder = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF);

        for data in payload {
            let mut buf = PushBytesBuf::new();
            buf.extend_from_slice(data).unwrap();
            builder = builder.push_slice(buf);
        }

        let script = builder.push_opcode(opcodes::all::OP_ENDIF).into_script();

        Witness::from_slice(&[script.into_bytes(), Vec::new()])
    }

    fn parse(witnesses: &[Witness]) -> Vec<ParsedEnvelope> {
        ParsedEnvelope::from_transaction(&Transaction {
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
        })
    }

    #[test]
    fn empty_witness() {
        assert_eq!(parse(&[Witness::new()]), Vec::new())
    }

    #[test]
    fn parse_from_tapscript() {
        assert_eq!(
            parse(&[Witness::from_slice(&[
                bitcoin::script::Builder::new()
                    .push_opcode(opcodes::OP_FALSE)
                    .push_opcode(opcodes::all::OP_IF)
                    .push_slice(b"ord")
                    .push_opcode(opcodes::all::OP_ENDIF)
                    .into_script()
                    .into_bytes(),
                Vec::new()
            ])]),
            vec![ParsedEnvelope {
                ..Default::default()
            }]
        );
    }

    #[test]
    fn no_inscription() {
        assert_eq!(
            parse(&[Witness::from_slice(&[
                ScriptBuf::new().into_bytes(),
                Vec::new()
            ])]),
            Vec::new()
        );
    }

    #[test]
    fn duplicate_field() {
        assert_eq!(
            parse(&[envelope(&[b"ord", &[255], &[], &[255], &[]])]),
            vec![ParsedEnvelope {
                payload: Nft {
                    duplicate_field: true,
                    ..Default::default()
                },
                ..Default::default()
            }]
        );
    }

    #[test]
    fn with_content_type() {
        assert_eq!(
            parse(&[envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"ord",
            ])]),
            vec![ParsedEnvelope {
                payload: create_nft("text/plain;charset=utf-8", "ord"),
                ..Default::default()
            }]
        );
    }

    #[test]
    fn no_body() {
        assert_eq!(
            parse(&[envelope(&[b"ord", &[1], b"text/plain;charset=utf-8"])]),
            vec![ParsedEnvelope {
                payload: Nft {
                    content_type: Some(b"text/plain;charset=utf-8".to_vec()),
                    ..Default::default()
                },
                ..Default::default()
            }],
        );
    }

    #[test]
    fn no_content_type() {
        assert_eq!(
            parse(&[envelope(&[b"ord", &[], b"foo"])]),
            vec![ParsedEnvelope {
                payload: Nft {
                    body: Some(b"foo".to_vec()),
                    ..Default::default()
                },
                ..Default::default()
            }],
        );
    }

    #[test]
    fn valid_body_in_multiple_pushes() {
        assert_eq!(
            parse(&[envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"foo",
                b"bar"
            ])]),
            vec![ParsedEnvelope {
                payload: create_nft("text/plain;charset=utf-8", "foobar"),
                ..Default::default()
            }],
        );
    }

    #[test]
    fn valid_body_in_zero_pushes() {
        assert_eq!(
            parse(&[envelope(&[b"ord", &[1], b"text/plain;charset=utf-8", &[]])]),
            vec![ParsedEnvelope {
                payload: create_nft("text/plain;charset=utf-8", ""),
                ..Default::default()
            }]
        );
    }

    #[test]
    fn multiple_inscriptions_in_a_single_witness() {
        let script = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice(b"foo")
            .push_opcode(opcodes::all::OP_ENDIF)
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice([1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice([])
            .push_slice(b"bar")
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        assert_eq!(
            parse(&[Witness::from_slice(&[script.into_bytes(), Vec::new()])]),
            vec![
                ParsedEnvelope {
                    payload: create_nft("text/plain;charset=utf-8", "foo"),
                    ..Default::default()
                },
                ParsedEnvelope {
                    payload: create_nft("text/plain;charset=utf-8", "bar"),
                    offset: 1,
                    ..Default::default()
                },
            ],
        );
    }

    #[test]
    fn extract_from_transaction() {
        assert_eq!(
            parse(&[envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"ord"
            ])]),
            vec![ParsedEnvelope {
                payload: create_nft("text/plain;charset=utf-8", "ord"),
                ..Default::default()
            }],
        );
    }
}
