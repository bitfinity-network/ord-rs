//! NFT

use crate::{utils, OrdError, OrdResult};

use bitcoin::script::PushBytesBuf;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Represents an arbitrary Ordinal inscription with optional metadata and content.
///
/// For now, we refer to this as an NFT (e.g., like an ERC721 token).
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq, Default)]
pub struct Nft {
    /// The main body of the inscription. This could be the actual data or content
    /// inscribed onto a Bitcoin satoshi.
    pub body: Option<Vec<u8>>,
    /// Specifies the MIME type of the `body` content, such as `text/plain` for text,
    /// `image/png` for images, etc., to inform how the data should be interpreted.
    pub content_type: Option<Vec<u8>>,
    /// Optional metadata associated with the inscription. This could be used to store
    /// additional information about the inscription, such as creator identifiers, timestamps,
    /// or related resources.
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

    /// Encode Self as a JSON string
    pub fn encode(&self) -> OrdResult<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Returns `Self` as a JSON-encoded operation to be pushed to the redeem script.
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
