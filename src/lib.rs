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
extern crate serde_with;

mod op;

pub use op::{Brc20Deploy, Brc20Mint, Brc20Op, Brc20Transfer};
