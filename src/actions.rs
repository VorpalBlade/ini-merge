//! Action matching framework for INI processing

use log::warn;
use regex::RegexSet;
use std::borrow::Cow;
use std::collections::HashMap;
use thiserror::Error;

/// Handles matching on INI lines and mapping the matches to generic actions
/// to be performed
#[derive(Debug)]
pub struct Actions<Action, SectionAction> {
    /// Actions for whole sections.
    section_actions: HashMap<String, SectionAction>,
    /// Literal matches and associated actions on (section, key)
    literal_actions: HashMap<String, Action>,
    /// Regex matches on (section, key)
    /// We use the null byte as a separator between the key and value here.
    regex_matches: RegexSet,
    /// Associated actions for regex matches
    regex_actions: Vec<Action>,
    /// Warn on multiple matches (default: true)
    warn_on_multiple_matches: bool,
}

impl<Action, SectionAction> Actions<Action, SectionAction> {
    /// Create a builder for this struct.
    #[must_use]
    pub fn builder() -> ActionsBuilder<Action, SectionAction> {
        ActionsBuilder::<Action, SectionAction>::new()
    }

    /// Lookup if there is a section action for the whole section
    pub(crate) fn find_section_action(&self, section: &str) -> Option<&SectionAction> {
        self.section_actions.get(section)
    }
}

impl<Action, SectionAction> Actions<Action, SectionAction>
where
    for<'a> Action: From<&'a SectionAction> + From<SectionAction> + Clone,
{
    /// Lookup if there is an action (or section action) for a specific section
    /// and key
    pub(crate) fn find_action<'this>(
        &'this self,
        section: &str,
        key: &str,
    ) -> Option<Cow<'this, Action>> {
        // Section actions have priority.
        if let Some(sec_act) = self.find_section_action(section) {
            return Some(Cow::Owned(sec_act.into()));
        }
        // Then literal actions
        let sec_key = section.to_string() + "\0" + key;
        if let Some(act) = self.literal_actions.get(sec_key.as_str()) {
            return Some(Cow::Borrowed(act));
        }
        // Finally regex matches
        let re_matches = self.regex_matches.matches(sec_key.as_str());
        if re_matches.matched_any() {
            let matches: Vec<_> = re_matches.iter().collect();
            if matches.len() != 1 && self.warn_on_multiple_matches {
                warn!(target: "ini-merge",
                      "Overlapping regex matches for {section}/{key}, first action taken. If this is intentional add the no-warn-multiple-key-matches directive");
            }
            let m = matches
                .first()
                .expect("Impossible: At least one match exists");
            return Some(Cow::Borrowed(
                self.regex_actions
                    .get(*m)
                    .expect("Impossible: At least one action exists for each match"),
            ));
        }
        None
    }
}

/// Builder for [Actions].
#[derive(Debug)]
pub struct ActionsBuilder<Action, SectionAction> {
    section_actions: HashMap<String, SectionAction>,
    literal_actions: HashMap<String, Action>,
    regex_matches: Vec<String>,
    regex_actions: Vec<Action>,
    /// Warn on multiple matches (default: true)
    warn_on_multiple_matches: bool,
}

impl<Action, SectionAction> Default for ActionsBuilder<Action, SectionAction> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Action, SectionAction> ActionsBuilder<Action, SectionAction> {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            section_actions: Default::default(),
            literal_actions: Default::default(),
            regex_matches: Default::default(),
            regex_actions: Default::default(),
            warn_on_multiple_matches: true,
        }
    }

    /// Add an ignore for a given section (exact match)
    pub fn add_section_action(
        &mut self,
        section: impl Into<String>,
        action: SectionAction,
    ) -> &mut Self {
        self.section_actions.insert(section.into(), action);
        self
    }

    /// Add an action for an exact match of section and key
    pub fn add_literal_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) -> &mut Self {
        self.literal_actions
            .insert(section.into() + "\0" + key.as_ref(), action);
        self
    }

    /// Add an action for a regex match of a section and key
    pub fn add_regex_action(
        &mut self,
        section: impl AsRef<str>,
        key: impl AsRef<str>,
        action: Action,
    ) -> &mut Self {
        fn inner<Action, SectionAction>(
            this: &mut ActionsBuilder<Action, SectionAction>,
            section: &str,
            key: &str,
            action: Action,
        ) {
            this.regex_actions.push(action);
            this.regex_matches.push(format!("(?:{section})\0(?:{key})"));
        }
        inner(self, section.as_ref(), key.as_ref(), action);
        self
    }

    /// Set if there should be a warning on multiple matches
    pub fn warn_on_multiple_matches(&mut self, warn: bool) -> &mut Self {
        self.warn_on_multiple_matches = warn;
        self
    }

    /// Build the [Actions] struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Actions<Action, SectionAction>, ActionsBuilderError> {
        Ok(Actions {
            section_actions: self.section_actions,
            literal_actions: self.literal_actions,
            regex_matches: RegexSet::new(self.regex_matches)
                .map_err(|e| ActionsBuilderError::RegexCompile(Box::new(e)))?,
            regex_actions: self.regex_actions,
            warn_on_multiple_matches: self.warn_on_multiple_matches,
        })
    }
}

/// Error type for [`ActionsBuilder`]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ActionsBuilderError {
    /// A regular expression failed to compile
    #[error("Failed to compile a regular expression: {0}")]
    RegexCompile(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
}
