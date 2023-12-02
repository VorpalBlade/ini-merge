//! # Library to merge INI files subject to configuration
//!
//! This library forms the backend to
//! <https://github.com/VorpalBlade/chezmoi_modify_manager>.
//! You probably want that tool instead.
//!
//! This library provides processing of INI files. In particular:
//!
//! * Merging of a source INI file with a target INI file.
//!   The merging is asymmetric: The values of the source are preferred unless
//!   specific rules have been provided for those sections and/or keys.
//!   Formatting is preserved. See [merge::merge_ini].
//! * Filtering of an INI file based on a rule set

#![warn(clippy::wildcard_imports)]
#![warn(clippy::needless_pass_by_value)]

pub mod actions;
pub mod filter;
mod loader;
pub mod merge;
mod source_loader;

// Re-export sub-module
pub use merge::mutations;

/// Decribes a property
///
/// This is the type that is passed to mutators.
#[derive(Debug)]
#[non_exhaustive]
pub struct Property<'a> {
    /// Trimmed section
    pub section: &'a str,
    /// Trimmed key
    pub key: &'a str,
    /// Trimmed value (if any)
    pub val: Option<&'a str>,
    /// Raw line
    pub raw: &'a str,
}

impl<'a> Property<'a> {
    /// Convert from `SourceValue` to `Property`
    pub(crate) fn from_src(
        section: &'a str,
        key: &'a str,
        value: &'a source_loader::SourceValue,
    ) -> Self {
        Self {
            section,
            key,
            val: value.value(),
            raw: value.raw(),
        }
    }

    /// Convert from INI parser value to Property
    pub(crate) fn try_from_ini(section: &'a str, value: ini_roundtrip::Item<'a>) -> Option<Self> {
        if let ini_roundtrip::Item::Property { key, val, raw } = value {
            Some(Property {
                section,
                key,
                val,
                raw,
            })
        } else {
            None
        }
    }
}

/// Input type to transformers
pub type InputData<'a> = Option<Property<'a>>;

/// Identifier for things outside sections. We could use None, but that
/// wouldn't allow easily ignoring by regex.
pub const OUTSIDE_SECTION: &str = "<NO_SECTION>";
