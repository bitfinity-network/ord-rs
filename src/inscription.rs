pub mod brc20;
pub mod iid;
pub mod nft;

use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::wallet::RedeemScriptPubkey;
use crate::{OrdError, OrdResult};

/// The `Inscription` trait defines the behavior necessary for handling
/// Bitcoin Ordinal inscriptions within the context of commit and reveal transactions.
///
/// These are methods for encoding, decoding, and managing
/// the inscriptions, tailored to specific types (e.g. `Brc20`, `Nft`).
pub trait Inscription: DeserializeOwned {
    /// Generates the redeem script from a script pubkey and the inscription.
    ///
    /// # Errors
    ///
    /// May return an `OrdError` if (de)serialization of any of the inscription fields
    /// fails while appending the script to the builder.
    fn generate_redeem_script(
        &self,
        builder: ScriptBuilder,
        pubkey: RedeemScriptPubkey,
    ) -> OrdResult<ScriptBuilder>;

    /// Encodes the inscription object into a JSON string.
    ///
    /// # Errors
    ///
    /// May return an `OrdError` if serialization fails.
    fn encode(&self) -> OrdResult<String>
    where
        Self: Serialize,
    {
        serde_json::to_string(self).map_err(OrdError::from)
    }

    /// Returns the MIME content type of the inscription.
    ///
    /// It should provide the MIME type string that best represents the
    /// data format of the inscription (e.g., "text/plain;charset=utf-8", "application/json").
    fn content_type(&self) -> String;

    /// Returns the body of the inscription as to be pushed to the redeem script.
    ///
    /// This body must follow the header of the inscription as presented below:
    ///
    /// Header:
    ///   - Public key
    ///   - OP_CHECKSIG
    ///   - OP_FALSE
    ///   - OP_IF
    ///   - "ord" (the opcode or marker indicating an ordinal inscription)
    ///   - 0x01 (version byte)
    ///   - {self.content_type()}
    ///   - 0x00 (separator byte)
    ///
    /// Next is the inscription data/body (payload)
    ///
    /// Then comes the footer:
    ///   - OP_ENDIF
    ///
    /// So for example in case of a BRC20, this function must return the JSON encoded BRC20 operation as `PushBytes`.
    fn data(&self) -> OrdResult<PushBytesBuf>;

    /// Parses inscription data from the serialized bytes found in the witness script.
    ///
    /// Decodes the inscription data embedded within the witness script of
    /// a Bitcoin transaction, reconstructing the original inscription object.
    ///
    /// # Errors
    ///
    /// May return an `OrdError` if parsing fails.
    fn parse(data: &[u8]) -> OrdResult<Self>
    where
        Self: Sized,
    {
        serde_json::from_slice(data).map_err(OrdError::Codec)
    }
}
