//! Define mutations that can be applied.

use crate::{
    actions::{Actions, ActionsBuilder},
    mutations::transforms::TransformSet,
};

use self::transforms::Transformer;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    rc::Rc,
};

pub mod transforms;

/// Describes the action for mutating the input
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Action {
    /// Normal merge logic. This is implied for entries not in the mutations set.
    Pass,
    /// Ignore source value, always use target value
    Ignore,
    /// Remove this entry
    Delete,
    /// Custom transform
    Transform(Rc<dyn Transformer>),
}

impl From<SectionAction> for Action {
    fn from(value: SectionAction) -> Self {
        Self::from(&value)
    }
}

impl From<&SectionAction> for Action {
    fn from(value: &SectionAction) -> Self {
        match value {
            SectionAction::Pass => Action::Pass,
            SectionAction::Ignore => Action::Ignore,
            SectionAction::Delete => Action::Delete,
        }
    }
}

/// Describes actions to apply to whole sections
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SectionAction {
    /// Normal merge logic. This is implied for entries not in the mutations set.
    Pass,
    /// Ignore source value, always use target value
    Ignore,
    /// Remove this whole section
    Delete,
}

/// Collects all the ways we can ignore, transform etc (mutations)
#[derive(Debug)]
pub struct Mutations {
    /// Inner actions
    actions: Actions<Action, SectionAction>,
    /// Section & keys that must exist (used to make "set" work)
    pub(crate) forced_keys: HashMap<String, HashSet<String>>,
}

impl Mutations {
    /// Create a builder for this struct.
    pub fn builder() -> MutationsBuilder {
        MutationsBuilder::new()
    }

    #[inline]
    pub(crate) fn find_section_action(&self, section: &str) -> &SectionAction {
        self.actions
            .find_section_action(section)
            .unwrap_or(&SectionAction::Pass)
    }

    #[inline]
    pub(crate) fn find_action<'this>(&'this self, section: &str, key: &str) -> Cow<'this, Action> {
        self.actions
            .find_action(section, key)
            .unwrap_or(Cow::Borrowed(&Action::Pass))
    }
}

/// Builder for [Mutations].
#[derive(Debug, Default)]
pub struct MutationsBuilder {
    /// Inner builder
    action_builder: ActionsBuilder<Action, SectionAction>,
    /// Note! Only add entries that also exist as a transform here
    forced_keys: HashMap<String, HashSet<String>>,
}

impl MutationsBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an ignore for a given section (exact match)
    pub fn add_section_action(&mut self, section: impl Into<String>, action: SectionAction) {
        self.action_builder.add_section_action(section, action)
    }

    /// Add an action for an exact match of section and key
    pub fn add_literal_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) {
        self.action_builder.add_literal_action(section, key, action)
    }

    /// Add an action for a regex match of a section and key
    pub fn add_regex_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) {
        self.action_builder.add_regex_action(section, key, action)
    }

    /// Add a forced set.
    pub fn add_setter(
        &mut self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl AsRef<str>,
        separator: impl AsRef<str>,
    ) {
        fn inner(
            this: &mut MutationsBuilder,
            section: String,
            key: String,
            value: &str,
            separator: &str,
        ) {
            this.action_builder.add_literal_action(
                &section,
                &key,
                Action::Transform(Rc::new(TransformSet::new(key.clone() + separator + value))),
            );
            this.forced_keys
                .entry(section)
                .and_modify(|v| {
                    v.insert(key.clone());
                })
                .or_insert_with(|| HashSet::from_iter([key]));
        }
        inner(
            self,
            section.into(),
            key.into(),
            value.as_ref(),
            separator.as_ref(),
        )
    }

    /// Build the Mutations struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Mutations, regex::Error> {
        Ok(Mutations {
            actions: self.action_builder.build()?,
            forced_keys: self.forced_keys,
        })
    }
}

#[cfg(test)]
mod tests {}
