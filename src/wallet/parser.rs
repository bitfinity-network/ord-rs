use crate::{error::InscriptionParseError, Inscription, OrdError, OrdResult};

use bitcoin::{blockdata::opcodes::all as all_opcodes, script::Instruction, Script, Transaction};

pub struct OrdParser;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptParseState {
    Signature,
    Checksig,
    Zero,
    If,
    Ord,
    One,
    ContentType,
    Zero2,
    Data,
    Endif,
}

impl ScriptParseState {
    fn begin() -> Self {
        Self::Signature
    }

    fn next(&self) -> Self {
        match self {
            Self::Signature => Self::Checksig,
            Self::Checksig => Self::Zero,
            Self::Zero => Self::If,
            Self::If => Self::Ord,
            Self::Ord => Self::One,
            Self::One => Self::ContentType,
            Self::ContentType => Self::Zero2,
            Self::Zero2 => Self::Data,
            Self::Data => Self::Endif,
            Self::Endif => Self::Signature,
        }
    }

    fn validate(&self, instruction: &Instruction) -> bool {
        match (self, instruction) {
            (Self::Signature, Instruction::PushBytes(data))
                if data.len() == 32 || data.len() == 33 =>
            {
                true
            }
            (Self::Checksig, Instruction::Op(all_opcodes::OP_CHECKSIG)) => true,
            (Self::Zero | Self::Zero2, Instruction::PushBytes(data)) if data.is_empty() => true,
            (Self::If, Instruction::Op(all_opcodes::OP_IF)) => true,
            (Self::Ord, Instruction::PushBytes(data)) if data.as_bytes() == b"ord" => true,
            (Self::One, Instruction::PushBytes(data)) if data.as_bytes() == b"\x01" => true,
            (Self::ContentType | Self::Data, Instruction::PushBytes(_)) => true,
            (Self::Endif, Instruction::Op(all_opcodes::OP_ENDIF)) => true,
            _ => false,
        }
    }
}

impl OrdParser {
    pub fn parse<T>(tx: &Transaction) -> OrdResult<Option<T>>
    where
        T: Inscription,
    {
        let mut err = None;
        for input in tx.input.iter() {
            // otherwise try to decode witness script
            for script in input.witness.iter() {
                let script = Script::from_bytes(script);
                match Self::decode_script(script) {
                    Ok(Some(inscription)) => return Ok(Some(inscription)),
                    Ok(None) => continue,
                    Err(e) => {
                        err = Some(e);
                    }
                }
            }
        }

        match err {
            Some(err) => Err(err),
            None => Ok(None),
        }
    }

    fn decode_script<T>(script: &Script) -> OrdResult<Option<T>>
    where
        T: Inscription,
    {
        let mut parse_state = ScriptParseState::begin();
        // iterate over script instructions
        for instruction in script.instructions() {
            let instruction = match instruction {
                Ok(i) => i,
                Err(e) => return Err(e.into()),
            };

            // validate data
            if !parse_state.validate(&instruction) {
                return Err(if instruction.opcode().is_some() {
                    OrdError::InscriptionParser(InscriptionParseError::UnexpectedOpcode)
                } else {
                    OrdError::InscriptionParser(InscriptionParseError::UnexpectedPushBytes)
                });
            }

            // check current state
            if parse_state == ScriptParseState::Data {
                let data = match instruction.push_bytes() {
                    Some(data) => data,
                    None => {
                        return Err(OrdError::InscriptionParser(
                            InscriptionParseError::UnexpectedOpcode,
                        ))
                    }
                };
                // parse data
                return Ok(Some(T::parse(data.as_bytes())?));
            }

            // update state to next
            parse_state = parse_state.next();
        }

        Ok(None)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::inscription::brc20::Brc20;
    use crate::utils::test_utils::get_transaction_by_id;

    #[tokio::test]
    async fn test_should_parse_inscription_brc20_p2tr() {
        let transaction = get_transaction_by_id(
            "ff314aebaa91a3f10cfba576d3be958127aba982d29146735e612869567e7808",
            bitcoin::Network::Testnet,
        )
        .await
        .unwrap();

        let inscription: Brc20 = OrdParser::parse(&transaction).unwrap().unwrap();
        assert_eq!(inscription, Brc20::transfer("mona", 100));
    }

    #[tokio::test]
    async fn test_should_parse_inscription_brc20_p2wsh() {
        let transaction = get_transaction_by_id(
            "c769750df54ee38fe2bae876dbf1632c779c3af780958a19cee1ca0497c78e80",
            bitcoin::Network::Testnet,
        )
        .await
        .unwrap();

        let inscription: Brc20 = OrdParser::parse(&transaction).unwrap().unwrap();
        assert_eq!(inscription, Brc20::transfer("mona", 100));
    }

    #[tokio::test]
    async fn test_should_not_parse_a_non_inscription() {
        let transaction = get_transaction_by_id(
            "37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8",
            bitcoin::Network::Bitcoin,
        )
        .await
        .unwrap();

        let decode_result: OrdResult<Option<Brc20>> = OrdParser::parse(&transaction);
        assert!(decode_result.is_err());
    }
}
