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
#[derive(Debug)]
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

/// Compare the value as an unsorted list.
///
/// Useful because Konversation likes to reorder lists.
///
/// Arguments:
/// * `separator`: Separating character
#[derive(Debug)]
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
                .map(|x| x.as_ref())
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
#[derive(Debug)]
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
        _args: &HashMap<impl Borrow<str> + Eq + Hash, impl AsRef<str>>,
    ) -> Result<Self, TransformerError>
    where
        Self: Sized,
    {
        Ok(Self)
    }
}

#[cfg(feature = "keyring")]
mod keyring_transform {
    use std::borrow::Borrow;
    use std::hash::Hash;

    use crate::InputData;

    use super::{Transformer, TransformerAction, TransformerError};

    /// Get value from keyring (kwallet or secret service). Useful for passwords
    /// etc that you do not want in your dotfiles repo, but sync via some more
    /// secure manner.
    ///
    /// Note! Requires the python library keyring.
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
    #[derive(Debug)]
    pub struct TransformKeyring {
        service: String,
        user: String,
    }

    impl TransformKeyring {
        pub fn new(service: String, user: String) -> Self {
            Self { service, user }
        }
    }

    impl Transformer for TransformKeyring {
        fn call<'a>(&self, src: &InputData<'a>, tgt: &InputData<'a>) -> TransformerAction<'a> {
            let password: Option<_> = {
                match keyring::Entry::new(self.service.as_str(), self.user.as_str()) {
                    Ok(entry) => match entry.get_password() {
                        Ok(v) => Some(v),
                        Err(err) => {
                            eprintln!("Keyring lookup error: {err}");
                            eprintln!("Keyring query: service={} user={}", self.service, self.user);
                            None
                        }
                    },
                    Err(err) => {
                        eprintln!("Keyring error: {err}");
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
                .map(|x| x.as_ref())
                .ok_or(TransformerError::Construct("Failed to get service"))?;
            let user = args
                .get("user")
                .map(|x| x.as_ref())
                .ok_or(TransformerError::Construct("Failed to get user"))?;
            Ok(Self::new(service.to_string(), user.to_string()))
        }
    }
}
