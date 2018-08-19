//! This is a helper module which reexports all logging macros from the `log`
//! crate. This means that you can simply say:
//!
//! ```
//! use crate::log::*;
//! ```
//!
//! To import all logging macros.

pub use log::{log, trace, debug, info, warn, error};
