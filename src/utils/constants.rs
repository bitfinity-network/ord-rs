pub const PROTOCOL_ID: [u8; 3] = *b"ord";
pub const BODY_TAG: [u8; 0] = [];
/// Tag 1, representing the MIME type of the body.
pub const CONTENT_TYPE_TAG: [u8; 1] = [1];
/// Tag 2, representing the position of the inscribed sat in the outputs.
pub const POINTER_TAG: [u8; 1] = [2];
/// Tag 3, representing the parent inscription.
pub const PARENT_TAG: [u8; 1] = [3];
/// Tag 5, representing CBOR metadata, stored as data pushes.
pub const METADATA_TAG: [u8; 1] = [5];
/// Tag 7, representing the metaprotocol identifier.
pub const METAPROTOCOL_TAG: [u8; 1] = [7];
/// Tag 9, representing the encoding of the body.
pub const CONTENT_ENCODING_TAG: [u8; 1] = [9];
/// Tag 11, representing a nominated inscription.
#[allow(unused)]
pub const DELEGATE_TAG: [u8; 1] = [11];
/// Tag 13, denoting an optional rune.
pub const RUNE_TAG: [u8; 1] = [13];
/// Maximum allowed postage
pub const POSTAGE: u64 = 333;
