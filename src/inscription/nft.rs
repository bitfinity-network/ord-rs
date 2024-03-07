//! NFT

pub mod id;
#[cfg(test)]
mod nft_tests;

use std::io::Cursor;
use std::str::FromStr;

use bitcoin::opcodes;
use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf, ScriptBuf};
use http::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::utils::{self, bytes_to_push_bytes, constants};
use crate::{Inscription, InscriptionParseError, OrdError, OrdResult};

/// Represents an arbitrary Ordinal inscription. We're "unofficially" referring to this as an NFT
/// (e.g., like an ERC721 token). Ordinal inscriptions allow for the embedding of data directly
/// into individual satoshis on the Bitcoin blockchain, enabling a unique form of digital
/// artifact creation and ownership tracking.
///
/// NFTs may include fields before an optional body. Each field consists of two data pushes,
/// a tag and a value.
///
/// [Reference](https://docs.ordinals.com/inscriptions.html#fields)
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Nft {
    /// The main body of the NFT. This is the core data or content of the NFT,
    /// which might represent an image, text, or other types of digital assets.
    pub body: Option<Vec<u8>>,
    /// Has a tag of 1, representing the MIME type of the body. This describes
    /// the format of the body content, such as "image/png" or "text/plain".
    pub content_type: Option<Vec<u8>>,
    /// Has a tag of 2, representing the position of the inscribed satoshi in the outputs.
    /// It is used to locate the specific satoshi that carries this inscription.
    pub pointer: Option<Vec<u8>>,
    /// Has a tag of 3, representing the parent NFT, i.e., the owner of an NFT
    /// can create child NFTs, establishing a hierarchy or a collection of related NFTs.
    pub parent: Option<Vec<u8>>,
    /// Has a tag of 5, representing CBOR (Concise Binary Object Representation) metadata,
    /// stored as data pushes. This is used for storing structured metadata in a compact format.
    pub metadata: Option<Vec<u8>>,
    /// Has a tag of 7, representing the metaprotocol identifier. This field specifies
    /// which metaprotocol, if any, this NFT follows, allowing for interoperability and
    /// standardized behavior across different systems.
    pub metaprotocol: Option<Vec<u8>>,
    /// Indicates if any field is incomplete. This is used
    /// to signal that the data is partially constructed or awaiting further information.
    pub incomplete_field: bool,
    /// Indicates if there are any duplicate fields. Duplicate fields
    /// could arise from errors in data entry or processing and may need resolution for the
    /// this structure to be considered valid.
    pub duplicate_field: bool,
    /// Has a tag of 9, representing the encoding of the body. This specifies how the body content
    /// is encoded, such as "base64" or "utf-8", providing guidance on how to interpret or display the content.
    pub content_encoding: Option<Vec<u8>>,
    /// Indicates if there are any unrecognized even fields. Even tags are reserved for future use
    /// and should not be used by current implementations.
    pub unrecognized_even_field: bool,
    /// Has a tag of 11, representing a nominated NFT. Used to delegate certain rights or
    /// attributes from one NFT to another, effectively linking them in a specified relationship.
    pub delegate: Option<Vec<u8>>,
}

impl Inscription for Nft {
    fn content_type(&self) -> String {
        unimplemented!()
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

impl Nft {
    /// Creates a new `Nft` with optional data.
    pub fn new(content_type: Option<Vec<u8>>, body: Option<Vec<u8>>) -> Self {
        Self {
            content_type,
            body,
            ..Default::default()
        }
    }

    /// Validates the NFT's content type.
    pub fn validate_content_type(&self) -> OrdResult<Self> {
        if let Some(content_type) = &self.content_type {
            let content_type_str =
                std::str::from_utf8(content_type).map_err(OrdError::Utf8Encoding)?;

            if !content_type_str.contains('/') {
                return Err(OrdError::InscriptionParser(
                    InscriptionParseError::ContentType,
                ));
            }
        }

        Ok(self.clone())
    }

    pub fn append_reveal_script_to_builder(&self, mut builder: ScriptBuilder) -> ScriptBuilder {
        builder = builder
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(constants::PROTOCOL_ID);

        if let Some(content_type) = self.content_type.clone() {
            builder = builder
                .push_slice(constants::CONTENT_TYPE_TAG)
                .push_slice(PushBytesBuf::try_from(content_type).unwrap());
        }

        if let Some(content_encoding) = self.content_encoding.clone() {
            builder = builder
                .push_slice(constants::CONTENT_ENCODING_TAG)
                .push_slice(PushBytesBuf::try_from(content_encoding).unwrap());
        }

        if let Some(protocol) = self.metaprotocol.clone() {
            builder = builder
                .push_slice(constants::METAPROTOCOL_TAG)
                .push_slice(PushBytesBuf::try_from(protocol).unwrap());
        }

        if let Some(parent) = self.parent.clone() {
            builder = builder
                .push_slice(constants::PARENT_TAG)
                .push_slice(PushBytesBuf::try_from(parent).unwrap());
        }

        if let Some(pointer) = self.pointer.clone() {
            builder = builder
                .push_slice(constants::POINTER_TAG)
                .push_slice(PushBytesBuf::try_from(pointer).unwrap());
        }

        if let Some(metadata) = &self.metadata {
            for chunk in metadata.chunks(520) {
                builder = builder.push_slice(constants::METADATA_TAG);
                builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec()).unwrap());
            }
        }

        if let Some(body) = &self.body {
            builder = builder.push_slice(constants::BODY_TAG);
            for chunk in body.chunks(520) {
                builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec()).unwrap());
            }
        }

        builder.push_opcode(opcodes::all::OP_ENDIF)
    }

    /// Encodes `Self` as a JSON string.
    pub fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Creates a new `Nft` from JSON-encoded string.
    pub fn from_json_str(data: &str) -> OrdResult<Self> {
        Self::from_str(data)?.validate_content_type()
    }

    /// Returns `Self` as a JSON-encoded data to be pushed to the redeem script.
    pub fn as_push_bytes(&self) -> OrdResult<PushBytesBuf> {
        utils::bytes_to_push_bytes(self.encode()?.as_bytes())
    }

    pub fn body(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    pub fn body_str(&self) -> Option<&str> {
        std::str::from_utf8(self.body.as_ref()?).ok()
    }

    pub fn content_length(&self) -> Option<usize> {
        Some(self.body()?.len())
    }

    pub fn content_type(&self) -> Option<&str> {
        std::str::from_utf8(self.content_type.as_ref()?).ok()
    }

    pub fn content_encoding(&self) -> Option<HeaderValue> {
        HeaderValue::from_str(
            std::str::from_utf8(self.content_encoding.as_ref()?).unwrap_or_default(),
        )
        .ok()
    }

    pub fn metadata(&self) -> Option<ciborium::Value> {
        ciborium::from_reader(Cursor::new(self.metadata.as_ref()?)).ok()
    }

    pub fn metaprotocol(&self) -> Option<&str> {
        std::str::from_utf8(self.metaprotocol.as_ref()?).ok()
    }

    pub fn pointer_value(pointer: u64) -> Vec<u8> {
        let mut bytes = pointer.to_le_bytes().to_vec();

        while bytes.last().copied() == Some(0) {
            bytes.pop();
        }

        bytes
    }

    pub fn reveal_script_as_scriptbuf(&self, builder: ScriptBuilder) -> ScriptBuf {
        self.append_reveal_script_to_builder(builder).into_script()
    }
}

impl FromStr for Nft {
    type Err = OrdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(OrdError::from)
    }
}

#[cfg(test)]
mod tests {
    use nft_tests::create_nft;

    use super::*;

    #[test]
    fn nft_creation() {
        let nft = create_nft("text/plain", "Hello, world!");

        assert_eq!(nft.content_type(), Some("text/plain"));
        assert_eq!(nft.body_str(), Some("Hello, world!"));
        assert!(nft.metadata().is_none());
    }

    #[test]
    fn json_serialization_deserialization() {
        let nft = create_nft("text/plain", "Hello, world!");

        let encoded = nft.encode().unwrap();
        let decoded: Nft = Nft::from_json_str(&encoded).unwrap();
        assert_eq!(nft, decoded);
    }

    #[test]
    fn to_push_bytes_conversion() {
        let nft = create_nft("text/plain", "Hello, world!");

        let push_bytes = nft.as_push_bytes().unwrap();
        assert_eq!(push_bytes.as_bytes(), nft.encode().unwrap().as_bytes());
    }

    #[test]
    fn invalid_utf8() {
        let invalid_utf8 = vec![0xff, 0xfe, 0xfd];
        let nft = create_nft("text/plain", invalid_utf8);

        assert!(nft.body_str().is_none());
        assert!(nft.metadata().is_none());
        assert!(nft.content_type().is_some());
    }

    #[test]
    fn reveal_script_chunks_body() {
        assert_eq!(
            create_nft("btc", [])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            7
        );

        assert_eq!(
            create_nft("btc", [0; 1])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            8
        );

        assert_eq!(
            create_nft("btc", [0; 520])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            8
        );

        assert_eq!(
            create_nft("btc", [0; 521])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            9
        );

        assert_eq!(
            create_nft("btc", [0; 1040])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            9
        );

        assert_eq!(
            create_nft("btc", [0; 1041])
                .reveal_script_as_scriptbuf(ScriptBuilder::new())
                .instructions()
                .count(),
            10
        );
    }

    #[test]
    fn reveal_script_chunks_metadata() {
        assert_eq!(
            Nft {
                metadata: None,
                ..Default::default()
            }
            .reveal_script_as_scriptbuf(ScriptBuilder::new())
            .instructions()
            .count(),
            4
        );

        assert_eq!(
            Nft {
                metadata: Some(Vec::new()),
                ..Default::default()
            }
            .reveal_script_as_scriptbuf(ScriptBuilder::new())
            .instructions()
            .count(),
            4
        );

        assert_eq!(
            Nft {
                metadata: Some(vec![0; 1]),
                ..Default::default()
            }
            .reveal_script_as_scriptbuf(ScriptBuilder::new())
            .instructions()
            .count(),
            6
        );

        assert_eq!(
            Nft {
                metadata: Some(vec![0; 520]),
                ..Default::default()
            }
            .reveal_script_as_scriptbuf(ScriptBuilder::new())
            .instructions()
            .count(),
            6
        );

        assert_eq!(
            Nft {
                metadata: Some(vec![0; 521]),
                ..Default::default()
            }
            .reveal_script_as_scriptbuf(ScriptBuilder::new())
            .instructions()
            .count(),
            8
        );
    }

    #[test]
    fn invalid_mime_type_nft() {
        let json = r#"{
            "content_type": "plain",
            "body": "SGVsbG8sIHdvcmxkIQ==",
            "metadata": "eyJjcmVhdG9yIjogIkFsaWNlIn0="
        }"#;

        let nft = Nft::from_json_str(json);
        assert!(nft.is_err());
    }

    #[test]
    fn metadata_function_decodes_metadata() {
        assert_eq!(
            Nft {
                metadata: Some(vec![0x44, 0, 1, 2, 3]),
                ..Default::default()
            }
            .metadata()
            .unwrap(),
            ciborium::Value::Bytes(vec![0, 1, 2, 3]),
        );
    }

    #[test]
    fn metadata_function_returns_none_if_no_metadata() {
        assert_eq!(
            Nft {
                metadata: None,
                ..Default::default()
            }
            .metadata(),
            None,
        );
    }

    #[test]
    fn metadata_function_returns_none_if_metadata_fails_to_parse() {
        assert_eq!(
            Nft {
                metadata: Some(vec![0x44]),
                ..Default::default()
            }
            .metadata(),
            None,
        );
    }
}
