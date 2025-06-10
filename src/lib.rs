//! # Library to merge INI files subject to configuration
//!
//! This library forms the backend to
//! <https://github.com/VorpalBlade/chezmoi_modify_manager>.
//! You probably want that tool instead.
//!
//! This library provides processing of INI files. In particular:
//!
//! * Merging of a source INI file with a target INI file. The merging is
//!   asymmetric: The values of the source are preferred unless specific rules
//!   have been provided for those sections and/or keys. Formatting is
//!   preserved. See [`merge::merge_ini`].
//! * Filtering of an INI file based on a rule set

/// Re-export keyring
#[cfg(feature = "keyring")]
pub use keyring;
// Re-export sub-module
pub use merge::mutations;

pub mod actions;
mod common;
pub mod filter;
mod loader;
pub mod merge;
mod source_loader;

pub use common::InputData;
pub use common::OUTSIDE_SECTION;
pub use common::Property;
