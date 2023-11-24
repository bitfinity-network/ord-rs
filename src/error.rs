use thiserror::Error;

/// BRC-20 error
#[derive(Error, Debug)]
pub enum Brc20Error {
    #[error("BRC-20 codec error: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("Bitcoin sighash error: {0}")]
    BitcoinSigHash(#[from] bitcoin::sighash::Error),
    #[error("Bitcoin script error: {0}")]
    PushBytes(#[from] bitcoin::script::PushBytesError),
    #[error("bad transaction input: {0}")]
    InputNotFound(usize),
    #[error("insufficient balance")]
    InsufficientBalance,
}
