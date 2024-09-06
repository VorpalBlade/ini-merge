//! Define transfomers that can be applied as mutations

use crate::InputData;
use itertools::Itertools;
#[cfg(feature = "keyring")]
pub use keyring_transform::TransformKeyring;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use thiserror::Error;

/// The action that a transform decides should happen for a line it processes.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransformerAction<'a> {
    /// No output
    Nothing,
    /// A line of output
    Line(Cow<'a, str>),
}

/// Error type for loading the source.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransformerConstructionError {
    #[error("Failed to construct transformer due to {0}")]
    Construct(&'static str),
}

/// Error type for loading the source.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransformerCallError {
    #[error("Invalid data for specific transform: {0}")]
    InvalidData(&'static str),
}

/// Trait for transformers operating on the input.
pub trait Transformer: std::fmt::Debug {
    /// Apply transformer to a property.
    /// The source and target data will always match (i.e. be the same property)
    fn call<'a>(
        &self,
        src: &InputData<'a>,
        tgt: &InputData<'a>,
    ) -> Result<TransformerAction<'a>, TransformerCallError>;

    /// Construct from a mapping of user provided arguments
    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerConstructionError>
    where
        Self: Sized;
}

/// Enum to avoid dynamic dispatch
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TransformerDispatch {
    UnsortedLists(TransformUnsortedLists),
    KdeShortcut(TransformKdeShortcut),
    #[cfg(feature = "keyring")]
    Keyring(TransformKeyring),
    #[doc(hidden)]
    Set(TransformSet),
}

impl Transformer for TransformerDispatch {
    fn call<'a>(
        &self,
        src: &InputData<'a>,
        tgt: &InputData<'a>,
    ) -> Result<TransformerAction<'a>, TransformerCallError> {
        match self {
            TransformerDispatch::UnsortedLists(v) => v.call(src, tgt),
            TransformerDispatch::KdeShortcut(v) => v.call(src, tgt),
            TransformerDispatch::Set(v) => v.call(src, tgt),
            #[cfg(feature = "keyring")]
            TransformerDispatch::Keyring(v) => v.call(src, tgt),
        }
    }

    fn from_user_input(
        _args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerConstructionError>
    where
        Self: Sized,
    {
        panic!("Can not construct dispatcher from user input. Invalid API usage!");
    }
}

macro_rules! dispatch_from {
    ($type:ty, $name:tt) => {
        impl From<$type> for TransformerDispatch {
            fn from(value: $type) -> Self {
                Self::$name(value)
            }
        }
    };
}

dispatch_from!(TransformUnsortedLists, UnsortedLists);
dispatch_from!(TransformKdeShortcut, KdeShortcut);
dispatch_from!(TransformSet, Set);
#[cfg(feature = "keyring")]
dispatch_from!(TransformKeyring, Keyring);

/// Compare the value as an unsorted list.
///
/// Useful because Konversation likes to reorder lists.
///
/// Arguments:
/// * `separator`: Separating character in the list
#[derive(Debug, Clone)]
pub struct TransformUnsortedLists {
    separator: char,
}

impl TransformUnsortedLists {
    pub fn new(separator: char) -> Self {
        Self { separator }
    }
}

impl Transformer for TransformUnsortedLists {
    fn call<'a>(
        &self,
        src: &InputData<'a>,
        tgt: &InputData<'a>,
    ) -> Result<TransformerAction<'a>, TransformerCallError> {
        // Deal with case of line in just target or source.
        // At least one of them will exist (or we wouldn't be here).
        match (src, tgt) {
            (None, None) => unreachable!(),
            (None, Some(_)) => Ok(TransformerAction::Nothing),
            (Some(val), None) => Ok(TransformerAction::Line(val.raw.into())),
            (Some(sval), Some(tval)) => {
                let ss: HashSet<_> = sval
                    .val
                    .ok_or(TransformerCallError::InvalidData(
                        "Key is missing value in source",
                    ))?
                    .split(self.separator)
                    .collect();
                let ts: HashSet<_> = tval
                    .val
                    .ok_or(TransformerCallError::InvalidData(
                        "Key is missing value in system",
                    ))?
                    .split(self.separator)
                    .collect();
                // If the sets are equal, return the target line to minimise uneeded diffs
                if ss == ts {
                    Ok(TransformerAction::Line(tval.raw.into()))
                } else {
                    Ok(TransformerAction::Line(sval.raw.into()))
                }
            }
        }
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerConstructionError>
    where
        Self: Sized,
    {
        Ok(Self::new(
            args.get("separator")
                .map(AsRef::as_ref)
                .ok_or(TransformerConstructionError::Construct(
                    "Failed to get separator",
                ))?
                .chars()
                .exactly_one()
                .map_err(|_| {
                    TransformerConstructionError::Construct(
                        "Failed to get character from separator",
                    )
                })?,
        ))
    }
}

/// Specialised transform to handle KDE changing certain global shortcuts back
/// and forth between formats like:
///
/// ```ini
/// playmedia=none,,Play media playback
/// playmedia=none,none,Play media playback
/// ```
///
/// No arguments
#[derive(Debug, Clone)]
pub struct TransformKdeShortcut;

impl Transformer for TransformKdeShortcut {
    fn call<'a>(
        &self,
        src: &InputData<'a>,
        tgt: &InputData<'a>,
    ) -> Result<TransformerAction<'a>, TransformerCallError> {
        // Deal with case of line in just target or source.
        // At least one of them will exist (or we wouldn't be here).
        match (src, tgt) {
            (None, None) => unreachable!(),
            (None, Some(_)) => Ok(TransformerAction::Nothing),
            (Some(val), None) => Ok(TransformerAction::Line(val.raw.into())),
            (Some(sval), Some(tval)) => {
                let src_split: Vec<_> = sval
                    .val
                    .ok_or(TransformerCallError::InvalidData(
                        "Key is missing value in source",
                    ))?
                    .split(',')
                    .collect();
                let tgt_split: Vec<_> = tval
                    .val
                    .ok_or(TransformerCallError::InvalidData(
                        "Key is missing value in target",
                    ))?
                    .split(',')
                    .collect();
                if src_split.len() == tgt_split.len()
                    && src_split.len() == 3
                    && src_split[0] == tgt_split[0]
                    && src_split[2] == tgt_split[2]
                    && ["", "none"].contains(&src_split[1])
                    && ["", "none"].contains(&tgt_split[1])
                {
                    Ok(TransformerAction::Line(tval.raw.into()))
                } else {
                    Ok(TransformerAction::Line(sval.raw.into()))
                }
            }
        }
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerConstructionError>
    where
        Self: Sized,
    {
        if args.is_empty() {
            Ok(Self)
        } else {
            Err(TransformerConstructionError::Construct(
                "Unexpected arguments",
            ))
        }
    }
}

/// Transform to set to a fixed value.
///
/// This is meant to be used together with templating, to override an entry
/// only on some systems.
///
/// *NOTE*: This is not meant to be used directly, as special support is
/// needed elsewhere. Instead, use [`super::MutationsBuilder::add_setter`]
///
/// Arguments:
/// * `raw`: Raw line to set
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct TransformSet {
    raw: Box<str>,
}

impl TransformSet {
    pub fn new(raw: Box<str>) -> Self {
        Self { raw }
    }
}

impl Transformer for TransformSet {
    fn call<'a>(
        &self,
        _src: &InputData<'a>,
        _tgt: &InputData<'a>,
    ) -> Result<TransformerAction<'a>, TransformerCallError> {
        Ok(TransformerAction::Line(Cow::Owned(self.raw.to_string())))
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerConstructionError>
    where
        Self: Sized,
    {
        Ok(Self::new(
            args.get("raw")
                .map(AsRef::as_ref)
                .ok_or(TransformerConstructionError::Construct(
                    "Failed to get raw entry",
                ))?
                .into(),
        ))
    }
}

#[cfg(feature = "keyring")]
mod keyring_transform {
    use super::Transformer;
    use super::TransformerAction;
    use super::TransformerConstructionError;
    use crate::InputData;
    use log::error;
    use std::borrow::Borrow;
    use std::hash::Hash;

    /// Get value from system keyring (secrets service). Useful for passwords
    /// etc that you do not want in your dotfiles repo, but sync via some more
    /// secure manner.
    ///
    /// Arguments:
    /// * `service`: Which service name to look under
    /// * `user`: The username identifying the entry
    /// * `separator`: The separator to use between key and value (optional,
    ///   default is `=`)
    ///
    /// Example args:
    /// * service: "my-service"
    /// * user: "my-user"
    ///
    /// To add a key compatible with the above service and user run a command
    /// like the following:
    ///
    /// ```console
    /// $ chezmoi_modify_manager --keyring-set my-service my-user
    /// ```
    /// and enter the password when prompted
    #[derive(Debug, Clone)]
    pub struct TransformKeyring {
        service: Box<str>,
        user: Box<str>,
        separator: Box<str>,
    }

    impl TransformKeyring {
        pub fn new(service: Box<str>, user: Box<str>, separator: Box<str>) -> Self {
            Self {
                service,
                user,
                separator,
            }
        }
    }

    impl Transformer for TransformKeyring {
        fn call<'a>(
            &self,
            src: &InputData<'a>,
            tgt: &InputData<'a>,
        ) -> Result<TransformerAction<'a>, super::TransformerCallError> {
            let password: Option<_> = {
                match keyring::Entry::new(&self.service, &self.user) {
                    Ok(entry) => match entry.get_password() {
                        Ok(v) => Some(v),
                        Err(err) => {
                            error!("Keyring lookup error: {err}");
                            error!("Keyring query: service={} user={}", self.service, self.user);
                            None
                        }
                    },
                    Err(err) => {
                        error!("Keyring error: {err}");
                        None
                    }
                }
            };
            let key = {
                if let Some(prop) = src {
                    prop.key
                } else if let Some(prop) = tgt {
                    prop.key
                } else {
                    unreachable!()
                }
            };
            match password {
                Some(value) => Ok(TransformerAction::Line(
                    format!("{key}{}{value}", self.separator).into(),
                )),
                None => {
                    // Try to copy from target state, useful if updating
                    // remotely over SSH with keyring not unlocked.
                    if let Some(prop) = tgt {
                        Ok(TransformerAction::Line(prop.raw.into()))
                    } else {
                        Ok(TransformerAction::Line(
                            format!("{key}{}<KEYRING ERROR>", self.separator).into(),
                        ))
                    }
                }
            }
        }

        fn from_user_input(
            args: &std::collections::HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
        ) -> Result<Self, TransformerConstructionError>
        where
            Self: Sized,
        {
            let service = args.get("service").map(AsRef::as_ref).ok_or(
                TransformerConstructionError::Construct("Failed to get service"),
            )?;
            let user = args.get("user").map(AsRef::as_ref).ok_or(
                TransformerConstructionError::Construct("Failed to get user"),
            )?;
            let separator = args.get("separator").map(AsRef::as_ref).unwrap_or("=");
            Ok(Self::new(service.into(), user.into(), separator.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Property;
    use pretty_assertions::assert_eq;

    #[test]
    fn unsorted_lists() {
        let t = TransformUnsortedLists::new(',');
        let action = t.call(
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("a,b,c"),
                raw: "b=a,b,c",
            }),
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("c,a,b"),
                raw: "b=c,a,b",
            }),
        );
        assert_eq!(
            action,
            Ok(TransformerAction::Line(Cow::Borrowed("b=c,a,b")))
        );

        let t = TransformUnsortedLists::new(',');
        let action = t.call(
            &Some(Property {
                section: "a",
                key: "b",
                val: Some(""),
                raw: "b=",
            }),
            &Some(Property {
                section: "a",
                key: "b",
                val: Some(""),
                raw: "b=",
            }),
        );
        assert_eq!(action, Ok(TransformerAction::Line(Cow::Borrowed("b="))));

        let action = t.call(
            &Some(Property {
                section: "a",
                key: "b",
                val: None,
                raw: "b",
            }),
            &Some(Property {
                section: "a",
                key: "b",
                val: None,
                raw: "b",
            }),
        );
        assert_eq!(
            action,
            Err(TransformerCallError::InvalidData(
                "Key is missing value in source"
            ))
        );
    }

    #[test]
    fn kde_shortcut() {
        let t = TransformKdeShortcut;
        let action = t.call(
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("none,,Media volume down"),
                raw: "b=none,,Media volume down",
            }),
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("none,none,Media volume down"),
                raw: "b=none,none,Media volume down",
            }),
        );
        assert_eq!(
            action,
            Ok(TransformerAction::Line(Cow::Borrowed(
                "b=none,none,Media volume down"
            )))
        );
    }

    #[test]
    fn set() {
        let t = TransformSet::new("a = q".into());
        let action = t.call(
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("c"),
                raw: "b=c",
            }),
            &Some(Property {
                section: "a",
                key: "b",
                val: Some("d"),
                raw: "b=d",
            }),
        );
        assert_eq!(
            action,
            Ok(TransformerAction::Line(Cow::Owned("a = q".to_owned())))
        );
    }
}
