//! Action matching framework for INI processing

use std::{borrow::Cow, collections::HashMap};

use regex::RegexSet;

/// Collects all the ways we can ignore, transform etc (mutations)
#[derive(Debug)]
pub struct Actions<Action, SectionAction>
where
    Action: From<SectionAction> + Clone,
    for<'a> Action: From<&'a SectionAction>,
    SectionAction: Clone,
{
    /// Actions for whole sections.
    section_actions: HashMap<String, SectionAction>,
    /// Literal matches and associated actions on (section, key)
    literal_actions: HashMap<String, Action>,
    /// Regex matches on (section, key)
    /// We use the null byte as a separator between the key and value here.
    regex_matches: RegexSet,
    /// Associated actions for regex matches
    regex_actions: Vec<Action>,
}

impl<Action, SectionAction> Actions<Action, SectionAction>
where
    Action: From<SectionAction> + Clone,
    for<'a> Action: From<&'a SectionAction>,
    SectionAction: Clone,
{
    /// Create a builder for this struct.
    pub fn builder() -> ActionsBuilder<Action, SectionAction> {
        ActionsBuilder::<Action, SectionAction>::new()
    }

    pub(crate) fn find_section_action(&self, section: &str) -> Option<&SectionAction> {
        self.section_actions.get(section)
    }

    pub(crate) fn find_action<'this>(
        &'this self,
        section: &str,
        key: &str,
    ) -> Option<Cow<'this, Action>> {
        // Section actions have priority.
        if let Some(sec_act) = self.find_section_action(section) {
            return Some(Cow::Owned(sec_act.into()));
        }
        let sec_key = section.to_string() + "\0" + key;
        if let Some(act) = self.literal_actions.get(sec_key.as_str()) {
            return Some(Cow::Borrowed(act));
        }
        let re_matches = self.regex_matches.matches(sec_key.as_str());
        if re_matches.matched_any() {
            let m = re_matches.iter().next().unwrap();
            return Some(Cow::Borrowed(self.regex_actions.get(m).unwrap()));
        }
        None
    }
}

/// Builder for [Mutations].
#[derive(Debug)]
pub struct ActionsBuilder<Action, SectionAction>
where
    Action: From<SectionAction> + Clone,
    for<'a> Action: From<&'a SectionAction>,
    SectionAction: Clone,
{
    section_actions: HashMap<String, SectionAction>,
    literal_actions: HashMap<String, Action>,
    regex_matches: Vec<String>,
    regex_actions: Vec<Action>,
}

impl<Action, SectionAction> Default for ActionsBuilder<Action, SectionAction>
where
    Action: From<SectionAction> + Clone,
    for<'a> Action: From<&'a SectionAction>,
    SectionAction: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Action, SectionAction> ActionsBuilder<Action, SectionAction>
where
    Action: From<SectionAction> + Clone,
    for<'a> Action: From<&'a SectionAction>,
    SectionAction: Clone,
{
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            section_actions: Default::default(),
            literal_actions: Default::default(),
            regex_matches: Default::default(),
            regex_actions: Default::default(),
        }
    }

    /// Add an ignore for a given section (exact match)
    pub fn add_section_action(&mut self, section: impl Into<String>, action: SectionAction) {
        self.section_actions.insert(section.into(), action);
    }

    /// Add an action for an exact match of section and key
    pub fn add_literal_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) {
        self.literal_actions
            .insert(section.into() + "\0" + key.as_ref(), action);
    }

    /// Add an action for a regex match of a section and key
    pub fn add_regex_action(
        &mut self,
        section: impl Into<String>,
        key: impl AsRef<str>,
        action: Action,
    ) {
        self.regex_actions.push(action);
        self.regex_matches
            .push(section.into() + "\0" + key.as_ref());
    }

    /// Build the Mutations struct
    ///
    /// Errors if a regex fails to compile.
    pub fn build(self) -> Result<Actions<Action, SectionAction>, regex::Error> {
        Ok(Actions {
            section_actions: self.section_actions,
            literal_actions: self.literal_actions,
            regex_matches: RegexSet::new(self.regex_matches)?,
            regex_actions: self.regex_actions,
        })
    }
}