//! INI merger functionality

pub mod mutations;

#[cfg(test)]
mod tests;

use self::mutations::{Action, Mutations, SectionAction};
use crate::{
    loader::{self, Loader},
    source_loader::{self, SectionAndKey, SourceIni, SourceValue},
};
use lending_iterator::prelude::*;
use thiserror::Error;
use std::{borrow::Cow, collections::HashSet, io::Read};

/// Error type for INI merger
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MergeError {
    /// An error while loading the target INI
    #[error("Failed to load target INI due to {0}")]
    TargetLoad(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
    /// An error while loading the source INI
    #[error("Failed to load source INI due to {0}")]
    SourceLoad(#[source] Box<dyn std::error::Error + 'static + Send + Sync>),
    /// A transformer reported an error
    #[error("Failed to apply transform {transformer} on {section}->{key} due to {reason}")]
    TransformerError {
        /// Transformer being applied
        transformer: String,
        section: String,
        key: String,
        reason: String,
    },
}

/// State tracking for the merge algorithm
#[derive(Debug)]
struct MergeState {
    /// Buffer building up the merged result
    result: Vec<String>,
    /// Temporary buffer that may be discarded or appended to [result] depending
    /// on what follows
    pending_lines: Vec<String>,
    /// All the section names we have seen so far
    seen_sections: HashSet<String>,
    /// All the keys we have seen so far in the current section (cleared for
    /// each new section)
    seen_keys: HashSet<String>,
    /// Name of the current section
    cur_section: String,
}

impl MergeState {
    fn new() -> Self {
        Self {
            result: Vec::default(),
            pending_lines: Vec::default(),
            seen_sections: HashSet::default(),
            seen_keys: HashSet::default(),
            cur_section: crate::OUTSIDE_SECTION.to_string(),
        }
    }

    /// Push a line to either pending lines or directly to the output.
    fn push_raw(&mut self, raw: String) {
        if self.pending_lines.is_empty() {
            self.result.push(raw);
        } else {
            self.pending_lines.push(raw);
        }
    }

    /// Emit the pending section header (if any)
    ///
    /// This deals with the case of a section missing from the source + an ignore key
    /// on an entry in that section. Without this, we would emit the entry without
    /// the section header.
    ///
    /// Comments from such sections might also end up pending.
    fn emit_pending_lines(&mut self) {
        self.result.append(&mut self.pending_lines);
    }

    /// Emit lines that only exist in the source or are forced by setters.
    ///
    /// Call just before switching to the next section.
    fn emit_non_target_lines(&mut self, source: &SourceIni, mutations: &Mutations) {
        if source.has_section(self.cur_section.as_str()) {
            match mutations.find_section_action(self.cur_section.as_str()) {
                None => {
                    let mut unseen_entries: Vec<_> = source
                        .section_entries(self.cur_section.clone())
                        .filter(|e| !self.seen_keys.contains(e.0.as_ref()))
                        .collect();
                    unseen_entries.sort_by_key(|e| e.0);
                    for (key, value) in unseen_entries {
                        let action = mutations.find_action(self.cur_section.as_str(), key);
                        self.seen_keys.insert(key.to_string());
                        self.emit_kv(action.as_deref(), key, Some(value), None);
                    }
                }
                Some(SectionAction::Ignore) => (),
                Some(SectionAction::Delete) => (),
            }
        }
        self.emit_force_keys(mutations);

        self.seen_keys.clear();
    }

    /// Emit lines from forced keys in the current section
    fn emit_force_keys(&mut self, mutations: &Mutations) {
        if let Some(forced_keys) = mutations.forced_keys.get(&self.cur_section) {
            self.emit_pending_lines();
            let mut forced_keys: Vec<_> = forced_keys
                .iter()
                .filter(|&e| !self.seen_keys.contains(e))
                .collect();
            forced_keys.sort();
            for key in forced_keys {
                let action = mutations.find_action(self.cur_section.as_str(), key);
                self.emit_kv(action.as_deref(), key, None, None);
            }
        }
    }

    /// Emit a key-value line, handling transforms. Ignores are NOT handled here fully.
    fn emit_kv(
        &mut self,
        action: Option<&Action>,
        key: &str,
        source: Option<&SourceValue>,
        target: Option<ini_roundtrip::Item>,
    ) {
        match action {
            None => {
                match source {
                    Some(val) => self.result.push(val.raw().into()),
                    // PANIC safety: In all cases were we are called with action pass, we should
                    // have a source line. This invariant is upheld in MutationsBuilder when it
                    // constructs forced_keys.
                    None => panic!("This should never happen"),
                }
            }
            Some(Action::Ignore) => (),
            Some(Action::Delete) => (),
            Some(Action::Transform(transform)) => {
                let src =
                    source.map(|v| crate::Property::from_src(self.cur_section.as_str(), key, v));
                let tgt = target
                    .and_then(|v| crate::Property::try_from_ini(self.cur_section.as_str(), v));
                let transform_result = transform.call(&src, &tgt);
                match transform_result {
                    crate::mutations::transforms::TransformerAction::Nothing => (),
                    crate::mutations::transforms::TransformerAction::Line(raw_line) => {
                        self.result.push(raw_line.into_owned());
                    }
                }
            }
        }
    }
}

/// Process the target file, merging the state of source and target files
pub(crate) fn merge<'a>(
    target: &'a mut Loader,
    source: &'a SourceIni,
    mutations: &Mutations,
) -> Vec<String> {
    let mut state = MergeState::new();

    while let Some(ref entry) = target.next() {
        match *entry {
            ini_roundtrip::Item::Error(raw) => {
                // TODO: Log warning
                state.push_raw(raw.into());
            }
            ini_roundtrip::Item::Comment { raw } | ini_roundtrip::Item::Blank { raw } => {
                state.push_raw(raw.into());
            }
            ini_roundtrip::Item::Section { name, raw } => {
                // Emit any pending source only lines. Can't be done in SectionEnd,
                // since there can be keys before the first section.
                state.emit_non_target_lines(source, mutations);
                // Bookkeeping
                state.cur_section.clear();
                state.cur_section.push_str(name);
                state.seen_sections.insert(name.into());
                state.seen_keys.clear();
                state.pending_lines.clear();

                match mutations.find_section_action(name) {
                    Some(SectionAction::Ignore) => state.push_raw(raw.into()),
                    None if source.has_section(name) => state.push_raw(raw.into()),
                    // We cannot yet be sure that this section shouldn't exist.
                    // It is possible that a key in this section is ignored, even
                    // though the whole section is not.
                    None => state.pending_lines.push(raw.into()),
                    // We will definitely skip the section in this case.
                    Some(SectionAction::Delete) => (),
                }
            }
            ini_roundtrip::Item::SectionEnd => (),
            target @ ini_roundtrip::Item::Property { key, val: _, raw } => {
                // Bookkeeping
                let action = mutations.find_action(&state.cur_section, key);
                let src_property = source.property(&SectionAndKey::new(
                    Cow::Owned(state.cur_section.clone()),
                    Cow::Borrowed(key),
                ));
                match action.as_deref() {
                    None => {
                        if let Some(src_val) = src_property {
                            state.seen_keys.insert(key.into());
                            state.emit_pending_lines();
                            state.emit_kv(action.as_deref(), key, Some(src_val), Some(target));
                        }
                    }
                    Some(Action::Ignore) => {
                        state.seen_keys.insert(key.into());
                        state.emit_pending_lines();
                        state.result.push(raw.into());
                    }
                    Some(Action::Delete) => {
                        // Nothing to do, just don't emit anything
                    }
                    Some(Action::Transform(_)) => {
                        state.seen_keys.insert(key.into());
                        state.emit_pending_lines();
                        state.emit_kv(action.as_deref(), key, src_property, Some(target));
                    }
                }
            }
        }
    }

    // End of system file, emit source only keys for the last section.
    state.emit_non_target_lines(source, mutations);

    // Go through and emit any source only sections
    let mut unseen_sections: HashSet<_> = source
        .sections()
        .filter(|x| !state.seen_sections.contains(x.0))
        .map(|(section, raw)| (section, raw.to_owned()))
        .collect();
    unseen_sections.extend(
        mutations
            .forced_keys
            .keys()
            .filter(|&x| !state.seen_sections.contains(x))
            .map(|section| (section, format!("[{section}]"))),
    );
    let mut unseen_sections: Vec<_> = unseen_sections.into_iter().collect();
    unseen_sections.sort_by_key(|e| e.0);
    for (section, raw) in unseen_sections {
        if section == crate::OUTSIDE_SECTION {
            // This case is handled above by the Section case for the first section.
            continue;
        }
        match mutations.find_section_action(section) {
            None => (),
            Some(SectionAction::Ignore) => continue,
            Some(SectionAction::Delete) => continue,
        }
        state.cur_section.clear();
        state.cur_section.push_str(section);
        state.seen_keys.clear();
        state.seen_sections.insert(section.into());
        state.pending_lines.clear();

        state.result.push(raw.clone());
        for (key, value) in source.section_entries(section.clone()) {
            let action = mutations.find_action(section, key);
            state.seen_keys.insert(key.to_string());
            state.emit_kv(action.as_deref(), key, Some(value), None);
        }
        state.emit_force_keys(mutations)
    }

    state.result
}

/// Merge two INI files, giving the merged file as a vector of strings, one per line.
pub fn merge_ini(
    target: &mut impl Read,
    source: &mut impl Read,
    mutations: &mutations::Mutations,
) -> Result<Vec<String>, MergeError> {
    let mut target =
        loader::load_ini(target).map_err(|inner| MergeError::TargetLoad(inner.into()))?;
    let source = source_loader::load_source_ini(source)
        .map_err(|inner| MergeError::SourceLoad(inner.into()))?;
    Ok(merge(&mut target, &source, mutations))
}
