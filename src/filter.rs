//! INI filtering functionality

use lending_iterator::prelude::*;
use std::io::Read;
use thiserror::Error;

use crate::{
    actions::{Actions, ActionsBuilder},
    loader::{self, Loader},
};

/// Operations that can be set for filtering
#[derive(Debug, Clone, Copy)]
pub enum FilterAction {
    /// Remove a matching entry entirely
    Remove,
    /// Replace the *value* of an entry with the given string.
    /// Separator format (with or without spaces) is auto detected.
    Replace(&'static str),
}

impl From<&'_ FilterAction> for FilterAction {
    fn from(value: &'_ FilterAction) -> Self {
        *value
    }
}

/// Filter actions object
pub type FilterActions = Actions<FilterAction, FilterAction>;
/// Filter actions builder
pub type FilterActionsBuilder = ActionsBuilder<FilterAction, FilterAction>;

/// Error type for INI merger
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FilterError {
    /// An error while loading the INI
    #[error("Failed to load input INI due to {0}")]
    Load(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
}

/// State tracking for the merge algorithm
#[derive(Debug)]
struct FilterState {
    /// Buffer building up the merged result
    result: Vec<String>,
    /// Temporary buffer that may be discarded or appended to [result] depending
    /// on what follows
    pending_lines: Vec<String>,
    /// Name of the current section
    cur_section: String,
}

impl FilterState {
    fn new() -> Self {
        Self {
            result: Default::default(),
            pending_lines: Default::default(),
            cur_section: crate::OUTSIDE_SECTION.to_string(),
        }
    }

    /// Flush pending to output and push an additional string
    fn push(&mut self, raw: String) {
        self.emit_pending_lines();
        self.result.push(raw);
    }

    /// Push a line to either pending lines or directly to the output.
    fn maybe_push(&mut self, raw: String) {
        if self.pending_lines.is_empty() {
            self.result.push(raw);
        } else {
            self.pending_lines.push(raw);
        }
    }

    /// Push a line to pending lines
    fn push_pending(&mut self, raw: String) {
        self.pending_lines.push(raw);
    }

    /// Emit the pending lines (if any)
    ///
    /// This deals with the case where we are not sure if we will emit anything
    /// in a given section yet. Comments from such sections might also end up pending.
    fn emit_pending_lines(&mut self) {
        self.result.append(&mut self.pending_lines);
    }
}

pub(crate) fn filter(input: &mut Loader, actions: &FilterActions) -> Vec<String> {
    let mut state = FilterState::new();

    while let Some(ref entry) = input.next() {
        match *entry {
            ini_roundtrip::Item::Error(raw) => {
                // TODO: Log warning
                state.push_pending(raw.into());
            }
            ini_roundtrip::Item::Comment { raw } | ini_roundtrip::Item::Blank { raw } => {
                match actions.find_section_action(&state.cur_section) {
                    None | Some(FilterAction::Replace(_)) => state.maybe_push(raw.into()),
                    Some(FilterAction::Remove) => (),
                }
            }
            ini_roundtrip::Item::Section { name, raw } => {
                state.cur_section.clear();
                state.cur_section.push_str(name);
                state.pending_lines.clear();

                match actions.find_section_action(name) {
                    Some(FilterAction::Remove) => (),
                    // For sections, replace all the values in the section, not the section itself.
                    Some(FilterAction::Replace(_)) => state.push_pending(raw.into()),
                    None => state.push_pending(raw.into()),
                }
            }
            ini_roundtrip::Item::SectionEnd => (),
            ini_roundtrip::Item::Property { key, val, raw } => {
                let action = actions.find_action(&state.cur_section, key);
                match action.as_deref() {
                    None => state.push(raw.into()),
                    Some(FilterAction::Remove) => (),
                    Some(FilterAction::Replace(replacement)) => {
                        // Extract the separator
                        match val {
                            Some(value) => {
                                let separator =
                                    raw.get(key.len()..(raw.len() - value.len())).unwrap_or("=");
                                state.push(format!("{key}{separator}{replacement}"))
                            }
                            // There is no value, nothing to hide...
                            None => state.push(raw.into()),
                        }
                    }
                }
            }
        }
    }

    state.result
}

/// Filter an INI file
pub fn filter_ini(
    input: &mut impl Read,
    actions: &FilterActions,
) -> Result<Vec<String>, FilterError> {
    let mut target = loader::load_ini(input).map_err(|inner| FilterError::Load(inner.into()))?;
    Ok(filter(&mut target, actions))
}

#[cfg(test)]
mod tests {

    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use std::collections::VecDeque;

    use super::{FilterAction, FilterActionsBuilder};

    const INPUT: &str = indoc! {"
        ; A comment
        a=1
        b_removed=2
        c_replaced=3

        [s1]
        ; d
        a = 42
        b_removed=43
        c_replaced=44
        aa_replaced =42
        aaa_replaced= 42
        aaa_replaced   =  42

        [s2_removed]
        a = 42
        d

        [s3_replaced]
        a = 42
        c_replaced=HIDDEN
        d

        [s5]
        b = c
        ; Literally matched (removed)

        [s4]
        b = c
        ; Literally matched

        "};

    const EXPECTED: &str = indoc! {"
        ; A comment
        a=1
        c_replaced=HIDDEN

        [s1]
        ; d
        a = 42
        c_replaced=HIDDEN
        aa_replaced =HIDDEN
        aaa_replaced= HIDDEN
        aaa_replaced   =  HIDDEN

        [s3_replaced]
        a = HIDDEN
        c_replaced=HIDDEN
        d

        "};

    #[test]
    fn test_merge_ini() {
        let mut input: VecDeque<_> = INPUT.as_bytes().to_owned().into();

        // Test a bunch of different actions and matchers.
        let mut actions = FilterActionsBuilder::new();
        actions.add_section_action("s4", FilterAction::Remove);
        actions.add_literal_action("s5", "b", FilterAction::Remove);
        // Note: priority is not guaranteed when there are overlapping matches
        actions.add_regex_action(".*", ".*_replaced", FilterAction::Replace("HIDDEN"));
        actions.add_regex_action(".*", ".*_removed", FilterAction::Remove);
        actions.add_regex_action(".*_removed", ".*", FilterAction::Remove);
        actions.add_regex_action(".*_replaced", ".*", FilterAction::Replace("HIDDEN"));
        let mutations = actions.build().unwrap();

        let result = super::filter_ini(&mut input, &mutations).unwrap();

        assert_eq!(EXPECTED, result.join("\n") + "\n");
    }
}
