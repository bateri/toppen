//! What the program makes of what the machine measured, for the page to ask about: the process
//! table, which is sorted, filtered, folded and held here and not in the page.
//!
//! This is `model/`: it knows nothing of the page. It takes and gives plain Rust values.

pub mod filter;
pub mod processes;
