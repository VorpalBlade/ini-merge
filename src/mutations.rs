//! Define mutations that can be applied.

use crate::mutations::transforms::TransformSet;

use self::transforms::Transformer;
use regex::RegexSet;
use std::collections::{HashMap, HashSet};

pub mod transforms;

/// Describes the action for mutating the input
#[derive(Debug)]
#[non_exhaustive]
pub enum Action {
    /// Normal merge logic. This is implied for entries not in the mutations set.
    Pass,
    /// Ignore source value, always use target value
    Ignore,
    /// Remove this entry
    Delete,
    /// Custom transform
    Transform(Box<dyn Transformer>),
}

/// Describes actions to apply to whole sections
#[derive(Debug)]
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
    /// Actions for whole sections.
    section_actions: HashMap<String, SectionAction>,
    /// Literal matches and associated actions on (section, key)
    literal_actions: HashMap<String, Action>,
    /// Regex matches on (section, key)
    /// We use the null byte as a separator between the key and value here.
    regex_matches: RegexSet,
    /// Associated actions for regex matches
    regex_actions: Vec<Action>,
    /// Section & keys that must exist (used to make "set" work)
    pub(crate) forced_keys: HashMap<String, HashSet<String>>,
}

impl Mutations {
    /// Create a builder for this struct.
    pub fn builder() -> MutationsBuilder {
        MutationsBuilder::new()
    }

    pub(crate) fn find_section_action(&self, section: &str) -> &SectionAction {
        self.section_actions
            .get(section)
            .unwrap_or(&SectionAction::Pass)
    }

    pub(crate) fn find_action(&self, section: &str, key: &str) -> &Action {
        // Section actions have priority.
        match self.find_section_action(section) {
            SectionAction::Pass => (),
            SectionAction::Ignore => return &Action::Ignore,
            SectionAction::Delete => return &Action::Delete,
        }
        let sec_key = section.to_string() + "\0" + key;
        if let Some(act) = self.literal_actions.get(sec_key.as_str()) {
            return act;
        }
        let re_matches = self.regex_matches.matches(sec_key.as_str());
        if re_matches.matched_any() {
            let m = re_matches.iter().next().unwrap();
            return self.regex_actions.get(m).unwrap();
        }
        &Action::Pass
    }
}

/// Builder for [Mutations].
#[derive(Debug, Default)]
pub struct MutationsBuilder {
    section_actions: HashMap<String, SectionAction>,
    literal_actions: HashMap<String, Action>,
    regex_matches: Vec<String>,
    regex_actions: Vec<Action>,
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
    #[must_use]
    pub fn add_section_action(self, section: impl Into<String>, action: SectionAction) -> Self {
        fn inner(
            mut this: MutationsBuilder,
            section: String,
            action: SectionAction,
        ) -> MutationsBuilder {
            this.section_actions.insert(section, action);
            this
        }
        inner(self, section.into(), action)
    }

    /// Add an action for an exact match of section and key
    #[must_use]
    pub fn add_literal_action(
        mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) -> Self {
        self.literal_actions
            .insert(section.into() + "\0" + key.as_ref(), action);
        self
    }

    /// Add an action for a regex match of a section and key
    #[must_use]
    pub fn add_regex_action(
        mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) -> Self {
        self.regex_actions.push(action);
        self.regex_matches
            .push(section.into() + "\0" + key.as_ref());
        self
    }

    /// Add a forced set.
    pub fn add_setter(
        self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl AsRef<str>,
    ) -> Self {
        fn inner(
            mut this: MutationsBuilder,
            section: String,
            key: String,
            value: &str,
        ) -> MutationsBuilder {
            this.literal_actions.insert(
                section.clone() + "\0" + key.as_ref(),
                Action::Transform(Box::new(TransformSet::new(key.clone() + " = " + value))),
            );
            this.forced_keys
                .entry(section)
                .and_modify(|v| {
                    v.insert(key.clone());
                })
                .or_insert_with(|| HashSet::from_iter([key]));
            this
        }
        inner(self, section.into(), key.into(), value.as_ref())
    }

    /// Build the Mutations struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Mutations, regex::Error> {
        Ok(Mutations {
            section_actions: self.section_actions,
            literal_actions: self.literal_actions,
            regex_matches: RegexSet::new(self.regex_matches)?,
            regex_actions: self.regex_actions,
            forced_keys: self.forced_keys,
        })
    }
}

#[cfg(test)]
mod tests {}
