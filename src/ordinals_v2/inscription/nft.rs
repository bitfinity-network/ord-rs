//! NFT

use crate::{OrdError, OrdResult};

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

    pub fn content_type(&self) -> Option<&str> {
        std::str::from_utf8(self.content_type.as_ref()?).ok()
    }

    pub fn body_bytes(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    pub fn body_str(&self) -> Option<&str> {
        std::str::from_utf8(self.body.as_ref()?).ok()
    }

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
