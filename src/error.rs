use thiserror::Error;

/// Ordinal transaction handling error types
#[derive(Error, Debug)]
pub enum OrdError {
    #[error("Ord codec error: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("Bitcoin sighash error: {0}")]
    BitcoinSigHash(#[from] bitcoin::sighash::Error),
    #[error("Bitcoin script error: {0}")]
    PushBytes(#[from] bitcoin::script::PushBytesError),
    #[error("Bad transaction input: {0}")]
    InputNotFound(usize),
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Invalid signature: {0}")]
    Signature(#[from] bitcoin::secp256k1::Error),
    #[error("Invalid signature")]
    UnexpectedSignature,
    #[error("Taproot builder error: {0}")]
    TaprootBuilder(#[from] bitcoin::taproot::TaprootBuilderError),
    #[error("Taproot compute error")]
    TaprootCompute,
    #[error("Scripterror: {0}")]
    Script(#[from] bitcoin::blockdata::script::Error),
    #[error("No transaction inputs")]
    NoInputs,
    #[error("Invalid UTF-8 in: {0}")]
    Utf8Encoding(#[from] std::str::Utf8Error),
    #[error("Inscription parser error: {0}")]
    InscriptionParser(#[from] InscriptionParseError),
}

/// Inscription parsing errors.
#[derive(Error, Debug)]
pub enum InscriptionParseError {
    #[error("unexpected opcode token")]
    UnexpectedOpcode,
    #[error("unexpected push bytes token")]
    UnexpectedPushBytes,
    #[error("bad data syntax")]
    BadDataSyntax,
    #[error("invalid MIME type format")]
    ContentType,
}
