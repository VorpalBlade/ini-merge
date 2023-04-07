//! Merger function

use crate::{
    loader::Loader,
    mutations::{Action, Mutations},
    source_loader::{SectionAndKey, SourceIni, SourceValue},
};
use lending_iterator::prelude::*;
use std::{borrow::Cow, collections::HashSet};

#[derive(Debug)]
struct MergeState {
    result: Vec<String>,
    pending_lines: Vec<String>,
    seen_sections: HashSet<String>,
    seen_keys: HashSet<String>,
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

    /// Emit lines that only exist in the source.
    ///
    /// Call just before switching to the next section.
    fn emit_source_only_lines(&mut self, source: &SourceIni, mutations: &Mutations) {
        if source.has_section(self.cur_section.as_str())
            && !mutations.is_section_ignored(self.cur_section.as_str())
        {
            let mut unseen_entries: Vec<_> = source
                .section_entries(self.cur_section.clone())
                .filter(|e| !self.seen_keys.contains(e.0.as_ref()))
                .collect();
            unseen_entries.sort_by_key(|e| e.0);
            for (key, value) in unseen_entries {
                let action = mutations.find_action(self.cur_section.as_str(), key);
                self.emit_kv(action, key, value, None);
            }
        }
        self.seen_keys.clear();
    }

    /// Emit a key-value line, handling transforms. Ignores are NOT handled here fully.
    fn emit_kv(
        &mut self,
        action: &Action,
        key: &str,
        source: &SourceValue,
        target: Option<ini_roundtrip::Item>,
    ) {
        match action {
            Action::Pass => {
                self.result.push(source.raw().into());
            }
            Action::Ignore => (),
            Action::Transform(transform) => {
                let src = crate::Property::from_src(self.cur_section.as_str(), key, source);
                let tgt = target
                    .and_then(|v| crate::Property::try_from_ini(self.cur_section.as_str(), v));
                let transform_result = transform.call(&Some(src), &tgt);
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
                state.emit_source_only_lines(source, mutations);
                // Bookkeeping
                state.cur_section.clear();
                state.cur_section.push_str(name);
                state.seen_sections.insert(name.into());
                state.seen_keys.clear();
                state.pending_lines.clear();

                if mutations.is_section_ignored(name) || source.has_section(name) {
                    state.push_raw(raw.into());
                } else {
                    // We cannot yet be sure that this section shouldn't exist.
                    // It is possible that a key in this section is ignored, even
                    // though the whole section is not.
                    state.pending_lines.push(raw.into());
                }
            }
            ini_roundtrip::Item::SectionEnd => (),
            target @ ini_roundtrip::Item::Property { key, val: _, raw } => {
                // Bookkeeping
                state.seen_keys.insert(key.into());
                let action = mutations.find_action(&state.cur_section, key);
                if let Action::Ignore = action {
                    state.emit_pending_lines();
                    state.result.push(raw.into());
                } else if let Some(src_val) = source.property(&SectionAndKey::new(
                    Cow::Owned(state.cur_section.clone()),
                    Cow::Borrowed(key),
                )) {
                    state.emit_pending_lines();
                    state.emit_kv(action, key, src_val, Some(target));
                }
            }
        }
    }

    // End of system file, emit source only keys for the last section.
    state.emit_source_only_lines(source, mutations);

    // Go through and emit any source only sections
    let unseed_sections: Vec<_> = source
        .sections()
        .filter(|x| !state.seen_sections.contains(x.0))
        .collect();
    for (section, raw) in unseed_sections {
        if section == crate::OUTSIDE_SECTION {
            // This case is handled above by the Section case for the first section.
            continue;
        }
        if mutations.is_section_ignored(section) {
            continue;
        }
        state.result.push(raw.clone());
        for (key, value) in source.section_entries(section.clone()) {
            let action = mutations.find_action(section, key);
            state.emit_kv(action, key, value, None);
        }
    }

    state.result
}
