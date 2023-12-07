use thiserror::Error;

/// BRC-20 error
#[derive(Error, Debug)]
pub enum OrdError {
    #[error("Ord codec error: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("Bitcoin sighash error: {0}")]
    BitcoinSigHash(#[from] bitcoin::sighash::Error),
    #[error("Bitcoin script error: {0}")]
    PushBytes(#[from] bitcoin::script::PushBytesError),
    #[error("bad transaction input: {0}")]
    InputNotFound(usize),
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("invalid signature: {0}")]
    Signature(#[from] bitcoin::secp256k1::Error),
    #[error("invalid signature")]
    UnexpectedSignature,
    #[error("taproot builder error: {0}")]
    TaprootBuilder(#[from] bitcoin::taproot::TaprootBuilderError),
    #[error("taproot compute error")]
    TaprootCompute,
    #[error("no transaction inputs")]
    NoInputs,
}
