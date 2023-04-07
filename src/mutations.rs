//! Define mutations that can be applied.

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
    /// Custom transform
    Transform(Box<dyn Transformer>),
}

/// Collects all the ways we can ignore, transform etc (mutations)
#[derive(Debug)]
pub struct Mutations {
    /// Section names to ignore.
    ignore_sections: HashSet<String>,
    /// Literal matches and associated actions on (section, key)
    literal_actions: HashMap<String, Action>,
    /// Regex matches on (section, key)
    /// We use the null byte as a separator between the key and value here.
    regex_matches: RegexSet,
    /// Associated actions for regex matches
    regex_actions: Vec<Action>,
}

impl Mutations {
    /// Create a builder for this struct.
    pub fn builder() -> MutationsBuilder {
        MutationsBuilder::new()
    }

    pub(crate) fn is_section_ignored(&self, section: &str) -> bool {
        self.ignore_sections.contains(section)
    }

    pub(crate) fn find_action(&self, section: &str, key: &str) -> &Action {
        if self.is_section_ignored(section) {
            return &Action::Ignore;
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
    ignore_sections: HashSet<String>,
    literal_actions: HashMap<String, Action>,
    regex_matches: Vec<String>,
    regex_actions: Vec<Action>,
}

impl MutationsBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an ignore for a given section (exact match)
    #[must_use]
    pub fn add_ignore_section(self, section: impl Into<String>) -> Self {
        fn inner(mut this: MutationsBuilder, section: String) -> MutationsBuilder {
            this.ignore_sections.insert(section);
            this
        }
        inner(self, section.into())
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

    /// Build the Mutations struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Mutations, regex::Error> {
        Ok(Mutations {
            ignore_sections: self.ignore_sections,
            literal_actions: self.literal_actions,
            regex_matches: RegexSet::new(self.regex_matches)?,
            regex_actions: self.regex_actions,
        })
    }
}

#[cfg(test)]
mod tests {}
