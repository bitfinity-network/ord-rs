//! NFT

use crate::{utils, InscriptionParseError, OrdError, OrdResult};

use bitcoin::script::PushBytesBuf;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::str::FromStr;

/// Represents an arbitrary Ordinal inscription with optional metadata and content.
///
/// For now, we refer to this as an NFT (e.g., like an ERC721 token).
#[serde_as]
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq, Default)]
pub struct Nft {
    /// Specifies the MIME type of the `body`, such as `text/plain` for text,
    /// `image/png` for images, etc., to inform how the data should be interpreted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Vec<u8>>,
    /// The main body of the inscription. This could be the actual data or content
    /// inscribed onto a Bitcoin satoshi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Vec<u8>>,
    /// Optional metadata associated with the inscription. This could be used to store
    /// additional information about the inscription, such as creator identifiers, timestamps,
    /// or related resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<u8>>,
}

impl Nft {
    /// Creates a new `Nft` with optional data.
    pub fn new(
        content_type: Option<Vec<u8>>,
        body: Option<Vec<u8>>,
        metadata: Option<Vec<u8>>,
    ) -> Self {
        Self {
            content_type,
            body,
            metadata,
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

    /// Returns the NFT inscription's content_type as a string if available, or `None` otherwise.
    pub fn content_type(&self) -> Option<&str> {
        std::str::from_utf8(self.content_type.as_ref()?).ok()
    }

    /// Returns the NFT inscription's body as bytes if available, or `None` otherwise.
    pub fn body_bytes(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    /// Returns the NFT inscription's body as a string if available, or `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the slice is not UTF-8 with a description as to why the provided slice is not UTF-8.
    pub fn body_str(&self) -> Option<&str> {
        std::str::from_utf8(self.body.as_ref()?).ok()
    }

    /// Returns the NFT inscription's metadata as a string if available, or `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the slice is not UTF-8 with a description as to why the provided slice is not UTF-8.
    pub fn metadata(&self) -> Option<&str> {
        std::str::from_utf8(self.metadata.as_ref()?).ok()
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
    use super::*;

    #[test]
    fn nft_creation() {
        let nft = Nft::new(
            Some(b"text/plain".to_vec()),
            Some(b"Hello, world!".to_vec()),
            None,
        );
        assert_eq!(nft.content_type(), Some("text/plain"));
        assert_eq!(nft.body_str(), Some("Hello, world!"));
        assert!(nft.metadata().is_none());
    }

    #[test]
    fn json_serialization_deserialization() {
        let nft = Nft::new(
            Some(b"text/plain".to_vec()),
            Some(b"Hello, world!".to_vec()),
            None,
        );
        let encoded = nft.encode().unwrap();
        let decoded: Nft = Nft::from_json_str(&encoded).unwrap();
        assert_eq!(nft, decoded);
    }

    #[test]
    fn to_push_bytes_conversion() {
        let nft = Nft::new(None, Some(b"Hello, world!".to_vec()), None);
        let push_bytes = nft.as_push_bytes().unwrap();
        assert_eq!(push_bytes.as_bytes(), nft.encode().unwrap().as_bytes());
    }

    #[test]
    fn invalid_utf8() {
        let invalid_utf8 = vec![0xff, 0xfe, 0xfd];
        let nft = Nft::new(None, Some(invalid_utf8.clone()), Some(invalid_utf8.clone()));
        assert!(nft.body_str().is_none());
        assert!(nft.metadata().is_none());
        assert!(nft.content_type().is_none());
    }

    #[test]
    fn invalid_utf8_content_type() {
        let json = "{\"content_type\": [255, 255, 255]}"; // Invalid UTF-8 bytes
        let nft = Nft::from_json_str(json);
        assert!(nft.is_err());
    }

    #[test]
    fn test_valid_nft() {
        // Example JSON with `content_type`, `body`, and `metadata` as arrays of byte values
        let json = r#"{
        "content_type": [116, 101, 120, 116, 47, 112, 108, 97, 105, 110],
        "body": [72, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100, 33],
        "metadata": [123, 34, 99, 114, 101, 97, 116, 111, 114, 34, 58, 32, 34, 65, 108, 105, 99, 101, 34, 125]
    }"#;

        let nft = Nft::from_json_str(json).unwrap();

        // Assuming `content_type` and `metadata` are text, we can convert them to strings for assertion
        assert_eq!(nft.content_type().unwrap(), "text/plain");
        assert_eq!(nft.body_str().unwrap(), "Hello, world!");
        assert_eq!(nft.metadata().unwrap(), "{\"creator\": \"Alice\"}");
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
}
