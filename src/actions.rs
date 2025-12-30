//! Action matching framework for INI processing

use log::warn;
use regex::RegexSet;
use std::borrow::Cow;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug)]
struct ActionMatcher<Action> {
    /// Literal matches and associated actions
    literal_actions: HashMap<String, Action>,
    /// Regex matches and associated actions
    regex_matches: RegexSet,
    /// Associated actions for regex matches
    regex_actions: Vec<Action>,
}

impl<Action> ActionMatcher<Action> {
    /// Create a builder for this struct.
    #[must_use]
    fn builder() -> ActionMatcherBuilder<Action> {
        ActionMatcherBuilder::<Action>::new()
    }

    /// Lookup if there is an action for a specific entry
    pub(crate) fn find_action<'this>(
        &'this self,
        entry: &str,
        warn_on_multiple_matches: bool,
    ) -> Option<&'this Action> {
        // First literal actions
        if let Some(act) = self.literal_actions.get(entry) {
            return Some(act);
        }
        // Finally regex matches
        let re_matches = self.regex_matches.matches(entry);
        if re_matches.matched_any() {
            let matches: Vec<_> = re_matches.iter().collect();
            if matches.len() != 1 && warn_on_multiple_matches {
                let printable_key = entry.replace('\0', "/");
                warn!(target: "ini-merge",
                      "Overlapping regex matches for {printable_key}, first action taken. If this is intentional add the no-warn-multiple-key-matches directive");
            }
            let m = matches
                .first()
                .expect("Impossible: At least one match exists");
            return Some(
                self.regex_actions
                    .get(*m)
                    .expect("Impossible: At least one action exists for each match"),
            );
        }
        None
    }
}

/// Builder for [`ActionMatcher`].
#[derive(Debug)]
struct ActionMatcherBuilder<Action> {
    literal_actions: HashMap<String, Action>,
    regex_matches: Vec<String>,
    regex_actions: Vec<Action>,
}

impl<Action> ActionMatcherBuilder<Action> {
    /// Create a new builder
    #[must_use]
    fn new() -> Self {
        Self {
            literal_actions: Default::default(),
            regex_matches: Default::default(),
            regex_actions: Default::default(),
        }
    }

    /// Add an action for an exact match of `entry`
    fn add_literal_action(&mut self, entry: String, action: Action) -> &mut Self {
        self.literal_actions.insert(entry, action);
        self
    }

    /// Add an action for a regex match of `entry`
    fn add_regex_action(&mut self, entry: String, action: Action) -> &mut Self {
        self.regex_actions.push(action);
        self.regex_matches.push(entry);
        self
    }

    /// Build the [Actions] struct
    ///
    /// Errors if a regex fails to compile.
    fn build(self) -> Result<ActionMatcher<Action>, ActionsBuilderError> {
        Ok(ActionMatcher {
            literal_actions: self.literal_actions,
            regex_matches: RegexSet::new(self.regex_matches)
                .map_err(|e| ActionsBuilderError::RegexCompile(Box::new(e)))?,
            regex_actions: self.regex_actions,
        })
    }
}

/// Handles matching on INI lines and mapping the matches to generic actions
/// to be performed
#[derive(Debug)]
pub struct Actions<Action, SectionAction> {
    section_actions: ActionMatcher<SectionAction>,
    key_actions: ActionMatcher<Action>,
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
        self.section_actions
            .find_action(section, self.warn_on_multiple_matches)
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
        if let Some(act) = self
            .key_actions
            .find_action(&sec_key, self.warn_on_multiple_matches)
        {
            return Some(Cow::Borrowed(act));
        }
        None
    }
}

/// Builder for [Actions].
#[derive(Debug)]
pub struct ActionsBuilder<Action, SectionAction> {
    section_actions: ActionMatcherBuilder<SectionAction>,
    key_actions: ActionMatcherBuilder<Action>,
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
            section_actions: ActionMatcher::<SectionAction>::builder(),
            key_actions: ActionMatcher::<Action>::builder(),
            warn_on_multiple_matches: true,
        }
    }

    /// Add an ignore for a given section (exact match)
    pub fn add_section_literal_action(
        &mut self,
        section: String,
        action: SectionAction,
    ) -> &mut Self {
        self.section_actions.add_literal_action(section, action);
        self
    }

    /// Add an action for a regex match of a section
    pub fn add_section_regex_action(
        &mut self,
        section: String,
        action: SectionAction,
    ) -> &mut Self {
        self.section_actions.add_regex_action(section, action);
        self
    }

    /// Add an action for an exact match of section and key
    pub fn add_literal_action(&mut self, section: String, key: &str, action: Action) -> &mut Self {
        let actual_key = section + "\0" + key;
        self.key_actions.add_literal_action(actual_key, action);
        self
    }

    /// Add an action for a regex match of a section and key
    pub fn add_regex_action(&mut self, section: &str, key: &str, action: Action) -> &mut Self {
        let actual_key = format!("(?:{section})\0(?:{key})");
        self.key_actions.add_regex_action(actual_key, action);
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
            section_actions: self.section_actions.build()?,
            key_actions: self.key_actions.build()?,
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
