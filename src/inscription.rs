pub mod brc20;
pub mod nft;

use bitcoin::script::PushBytesBuf;
use serde::de::DeserializeOwned;

use crate::OrdResult;

/// The inscription trait is used to write data to the redeem script of a commit and reveal transaction.
pub trait Inscription: DeserializeOwned {
    /// Returns the content type of the inscription.
    fn content_type(&self) -> String;

    /// Returns the inscription as to be pushed to the redeem script.
    ///
    /// This data follows the header of the inscription:
    ///
    /// - public key
    /// - OP_CHECKSIG
    /// - OP_FALSE
    /// - OP_IF
    /// - ord
    /// - 0x01
    /// - {inscription.content_type()}
    /// - 0x00
    ///
    /// then it comes your data
    ///
    /// and then the footer:
    ///
    /// - OP_ENDIF
    ///
    /// So for example in case of a BRC20, this function must return the JSON encoded BRC20 operation as `PushBytes`.
    fn data(&self) -> OrdResult<PushBytesBuf>;

    /// Returns the inscription data from the serialized inscription bytes in the witness script.
    fn parse(data: &[u8]) -> OrdResult<Self>
    where
        Self: Sized;
}
