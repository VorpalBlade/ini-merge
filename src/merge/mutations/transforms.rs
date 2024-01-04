//! Define transfomers that can be applied as mutations

use std::{
    borrow::{Borrow, Cow},
    collections::{HashMap, HashSet},
    hash::Hash,
};

use crate::InputData;
use itertools::Itertools;
use thiserror::Error;

#[cfg(feature = "keyring")]
pub use keyring_transform::TransformKeyring;

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
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransformerError {
    #[error("Failed to construct transformer due to {0}")]
    Construct(&'static str),
}

/// Trait for transformers operating on the input.
pub trait Transformer: std::fmt::Debug {
    /// Apply transformer to a property.
    /// The source and target data will always match (i.e. be the same property)
    fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a>;

    /// Construct from a mapping of user provided arguments
    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerError>
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
    fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a> {
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
    ) -> Result<Self, TransformerError>
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
/// * `separator`: Separating character
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
    fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a> {
        // Deal with case of line in just target or source.
        // At least one of them will exist (or we wouldn't be here).
        match (src, tgt) {
            (None, None) => unreachable!(),
            (None, Some(val)) | (Some(val), None) => TransformerAction::Line(val.raw.into()),
            (Some(sval), Some(tval)) => {
                let ss: HashSet<_> = sval.val.unwrap().split(|x| x == self.separator).collect();
                let ts: HashSet<_> = tval.val.unwrap().split(|x| x == self.separator).collect();
                // If the sets are equal, return the target line to minimise uneeded diffs
                if ss == ts {
                    TransformerAction::Line(tval.raw.into())
                } else {
                    TransformerAction::Line(sval.raw.into())
                }
            }
        }
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerError>
    where
        Self: Sized,
    {
        Ok(Self::new(
            args.get("separator")
                .map(AsRef::as_ref)
                .ok_or(TransformerError::Construct("Failed to get separator"))?
                .chars()
                .exactly_one()
                .map_err(|_| {
                    TransformerError::Construct("Failed to get character from separator")
                })?,
        ))
    }
}

/// Specialised transform to handle KDE changing certain global shortcuts back and forth between formats like:
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
    fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a> {
        // Deal with case of line in just target or source.
        // At least one of them will exist (or we wouldn't be here).
        match (src, tgt) {
            (None, None) => unreachable!(),
            (None, Some(val)) | (Some(val), None) => TransformerAction::Line(val.raw.into()),
            (Some(sval), Some(tval)) => {
                let src_split: Vec<_> = sval.val.unwrap().split(',').collect();
                let tgt_split: Vec<_> = tval.val.unwrap().split(',').collect();
                if src_split.len() == tgt_split.len()
                    && src_split.len() == 3
                    && src_split[0] == tgt_split[0]
                    && src_split[2] == tgt_split[2]
                    && ["", "none"].contains(&src_split[1])
                    && ["", "none"].contains(&tgt_split[1])
                {
                    TransformerAction::Line(tval.raw.into())
                } else {
                    TransformerAction::Line(sval.raw.into())
                }
            }
        }
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerError>
    where
        Self: Sized,
    {
        if args.is_empty() {
            Ok(Self)
        } else {
            Err(TransformerError::Construct("Unexpected arguments"))
        }
    }
}

/// Transform to set to a fixed value.
///
/// This is meant to be used together with templating, to override an entry
/// only on some systems.
///
/// *NOTE*: This is not meant to be used directly, as special support is
/// needed elsewhere. Instead use [`super::MutationsBuilder::add_setter`]
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
    fn call<'a>(&self, _src: &InputData<'a>, _tgt: &InputData<'a>) -> TransformerAction<'a> {
        TransformerAction::Line(Cow::Owned(self.raw.to_string()))
    }

    fn from_user_input(
        args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerError>
    where
        Self: Sized,
    {
        Ok(Self::new(
            args.get("raw")
                .map(AsRef::as_ref)
                .ok_or(TransformerError::Construct("Failed to get raw entry"))?
                .into(),
        ))
    }
}

#[cfg(feature = "keyring")]
mod keyring_transform {
    use std::borrow::Borrow;
    use std::hash::Hash;

    use log::error;

    use crate::InputData;

    use super::{Transformer, TransformerAction, TransformerError};

    /// Get value from system keyring (secrets service). Useful for passwords
    /// etc that you do not want in your dotfiles repo, but sync via some more
    /// secure manner.
    ///
    /// Arguments:
    /// * `service`: Which service name to look under
    /// * `user`: The user name identifying the entry
    ///
    /// Example args:
    /// * service: "chezmoi-modify-manager"
    /// * user: "konversation-login"
    ///
    /// To add a key compatible with the above service and user run a command like the following:
    /// ```console
    /// $ secret-tool store --label="Descriptive name" service chezmoi-modify-manager username konversation-login
    /// ```
    /// and enter the password when prompted
    #[derive(Debug, Clone)]
    pub struct TransformKeyring {
        service: Box<str>,
        user: Box<str>,
    }

    impl TransformKeyring {
        pub fn new(service: Box<str>, user: Box<str>) -> Self {
            Self { service, user }
        }
    }

    impl Transformer for TransformKeyring {
        fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a> {
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
                Some(value) => TransformerAction::Line(format!("{key}={value}").into()),
                None => {
                    // Try to copy from target state, useful if updating
                    // remotely over SSH with keyring not unlocked.
                    if let Some(prop) = tgt {
                        TransformerAction::Line(prop.raw.into())
                    } else {
                        TransformerAction::Line(format!("{key}=<KEYRING ERROR>").into())
                    }
                }
            }
        }

        fn from_user_input(
            args: &std::collections::HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
        ) -> Result<Self, TransformerError>
        where
            Self: Sized,
        {
            let service = args
                .get("service")
                .map(AsRef::as_ref)
                .ok_or(TransformerError::Construct("Failed to get service"))?;
            let user = args
                .get("user")
                .map(AsRef::as_ref)
                .ok_or(TransformerError::Construct("Failed to get user"))?;
            Ok(Self::new(service.into(), user.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Property;

    use super::*;

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
        assert_eq!(action, TransformerAction::Line(Cow::Borrowed("b=c,a,b")));
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
            TransformerAction::Line(Cow::Borrowed("b=none,none,Media volume down"))
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
            TransformerAction::Line(Cow::Owned("a = q".to_owned()))
        );
    }
}
