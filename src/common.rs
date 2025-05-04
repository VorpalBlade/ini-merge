//! Common types and definitons for INI merge

/// Describes a property
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
        value: &'a crate::source_loader::SourceValue,
    ) -> Self {
        Self {
            section,
            key,
            val: value.value(),
            raw: value.raw(),
        }
    }

    /// Convert from INI parser value to Property
    pub(crate) const fn try_from_ini(
        section: &'a str,
        value: ini_roundtrip::Item<'a>,
    ) -> Option<Self> {
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
