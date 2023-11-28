use bitcoin::script::PushBytesBuf;

use crate::OrdResult;

/// The inscription trait is used to write data to the redeem script of a commit and reveal transaction.
pub trait Inscription {
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
    fn data(&self) -> OrdResult<PushBytesBuf>;
}
