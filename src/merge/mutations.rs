//! Define mutations that can be applied to merging

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::actions::Actions;
use crate::actions::ActionsBuilder;
use crate::actions::ActionsBuilderError;
use crate::mutations::transforms::TransformSet;

use self::transforms::TransformerDispatch;

pub mod transforms;

/// Describes the action for mutating the input
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Action {
    /// Ignore source value, always use target value
    Ignore,
    /// Remove this entry
    Delete,
    /// Custom transform
    Transform(TransformerDispatch),
}

impl From<SectionAction> for Action {
    fn from(value: SectionAction) -> Self {
        Self::from(&value)
    }
}

impl From<&SectionAction> for Action {
    fn from(value: &SectionAction) -> Self {
        match value {
            SectionAction::Ignore => Action::Ignore,
            SectionAction::Delete => Action::Delete,
        }
    }
}

/// Describes actions to apply to whole sections
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SectionAction {
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
    pub(crate) fn find_section_action(&self, section: &str) -> Option<&SectionAction> {
        self.actions.find_section_action(section)
    }

    #[inline]
    pub(crate) fn find_action<'this>(
        &'this self,
        section: &str,
        key: &str,
    ) -> Option<Cow<'this, Action>> {
        self.actions.find_action(section, key)
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
    pub fn add_section_action(
        &mut self,
        section: impl Into<String>,
        action: SectionAction,
    ) -> &mut Self {
        self.action_builder.add_section_action(section, action);
        self
    }

    /// Add an action for an exact match of section and key
    pub fn add_literal_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) -> &mut Self {
        self.action_builder.add_literal_action(section, key, action);
        self
    }

    /// Add an action for a regex match of a section and key
    pub fn add_regex_action(
        &mut self,
        section: impl AsRef<str>,
        key: impl AsRef<str>,
        action: Action,
    ) -> &mut Self {
        self.action_builder.add_regex_action(section, key, action);
        self
    }

    /// Add a forced set.
    pub fn add_setter(
        &mut self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl AsRef<str>,
        separator: impl AsRef<str>,
    ) -> &mut Self {
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
                Action::Transform(
                    TransformSet::new((key.clone() + separator + value).into()).into(),
                ),
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
        );
        self
    }

    pub fn warn_on_multiple_matches(&mut self, warn: bool) -> &mut Self {
        self.action_builder.warn_on_multiple_matches(warn);
        self
    }

    /// Build the Mutations struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Mutations, ActionsBuilderError> {
        Ok(Mutations {
            actions: self.action_builder.build()?,
            forced_keys: self.forced_keys,
        })
    }
}

#[cfg(test)]
mod tests {}
