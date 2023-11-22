//! # BRC20
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
extern crate serde;

mod error;
mod op;
mod result;
mod transaction;
mod utils;

pub use error::Brc20Error;
pub use op::{Brc20Deploy, Brc20Mint, Brc20Op, Brc20Transfer};
pub use result::Brc20Result;
