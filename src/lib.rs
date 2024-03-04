//! # ord-rs
//!
//! # Get started
//!
//! INSERT TEXT HERE
//!
//! ## Example
//!
//! ```rust
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Ok(())
//! }
//! ```
//!

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

mod error;
pub mod inscription;
mod result;
mod utils;

pub(crate) mod constants;
pub mod wallet;

pub use bitcoin;
pub use error::{InscriptionParseError, OrdError};
pub use inscription::{brc20::Brc20, nft::Nft, Inscription};
pub use result::OrdResult;
pub use wallet::{OrdParser, OrdTransactionBuilder};
