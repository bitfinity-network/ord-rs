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
mod inscription;
mod result;
mod utils;

pub mod brc20;
pub mod transaction;

pub use bitcoin;
pub use error::OrdError;
pub use inscription::Inscription;
pub use result::OrdResult;
pub use transaction::OrdTransactionBuilder;
