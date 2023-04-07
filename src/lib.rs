//! # Library to merge INI files subject to configuration
//!
//! This library forms the backend to
//! <https://github.com/VorpalBlade/chezmoi_modify_manager>.
//! You probably want that tool instead.
//!
//! This library provides merging of a source INI file with a target INI file.
//! The merging is asymmetric: The values of the source are preferred unless
//! specific rules have been provided for those sections and/or keys. Formatting
//! is preserved.
use std::io::Read;

use thiserror::Error;

mod loader;
mod merge;
pub mod mutations;
mod source_loader;

/// Error type for INI merger
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IniError {
    /// An error while loading the target INI
    #[error("Failed to load target INI due to {0}")]
    TargetLoad(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
    /// An error while loading the source INI
    #[error("Failed to load source INI due to {0}")]
    SourceLoad(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
    /// A transformer reported an error
    #[error("Failed to apply transform {transformer} on {section}->{key} due to {reason}")]
    TransformerError {
        /// Transformer being applied
        transformer: String,
        section: String,
        key: String,
        reason: String,
    },
}

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

/// Merge two INI files, giving the merged file as a vector of strings, one per line.
pub fn merge_ini(
    target: &mut impl Read,
    source: &mut impl Read,
    mutations: &mutations::Mutations,
) -> Result<Vec<String>, IniError> {
    let mut target =
        loader::load_ini(target).map_err(|inner| IniError::TargetLoad(inner.into()))?;
    let source = source_loader::load_source_ini(source)
        .map_err(|inner| IniError::SourceLoad(inner.into()))?;
    Ok(merge::merge(&mut target, &source, mutations))
}

#[cfg(test)]
mod tests;
