use crate::mutations::Action;
use crate::mutations::MutationsBuilder;
use crate::mutations::SectionAction;
use crate::mutations::transforms::TransformKdeShortcut;
use crate::mutations::transforms::TransformUnsortedLists;
use indoc::indoc;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;

const SOURCE: &str = indoc! {"
    ; Comments are ignored in source
    src_first=1
    [s1]
    a = 42
    playmedia=none,,Play media playback
    unsorted_same=1,2,3,4
    unsorted_diferent=1,2,3,5

    [s2]
    b = value
    c

    [s4]
    source_only = 42

    [s5]
    a_ign = 2
    aaa = 3
    "};

const TARGET: &str = indoc! {"
    ; Comments are copied from target
    tgt_first=1
    [s1]
    a = 32
    b = will be discarded
    c = ignored, kept
    playmedia=none,none,Play media playback
    unsorted_same=4,3,2,1
    unsorted_diferent=3,2,1

    [s2]
    b = overwritten
    d
    e

    [s3]
    ignored, and kept = 3

    [s5]
    b_ign = 2
    aaa = 2
    "};

const EXPECTED: &str = indoc! {"
    ; Comments are copied from target
    src_first=1
    [s1]
    a = 42
    c = ignored, kept
    playmedia=none,none,Play media playback
    unsorted_same=4,3,2,1
    unsorted_diferent=1,2,3,5

    [s2]
    b = value
    e

    c
    [s3]
    ignored, and kept = 3

    [s5]
    b_ign = 2
    aaa = 3
    [s4]
    source_only = 42
    "};

#[test]
fn test_merge_ini() {
    let mut src: VecDeque<_> = SOURCE.as_bytes().to_owned().into();
    let mut tgt: VecDeque<_> = TARGET.as_bytes().to_owned().into();

    // Test a bunch of different actions and matchers.
    let mut mutations = MutationsBuilder::new();
    mutations.add_section_literal_action("s3".into(), SectionAction::Ignore);
    mutations.add_literal_action("s1".into(), "c", Action::Ignore);
    mutations.add_literal_action("s2".into(), "e", Action::Ignore);
    mutations.add_literal_action(
        "s1".into(),
        "playmedia",
        Action::Transform(TransformKdeShortcut.into()),
    );
    mutations.add_regex_action("s5", ".*_ign", Action::Ignore);
    mutations.add_regex_action(
        "s1",
        "unsorted_.*",
        Action::Transform(TransformUnsortedLists::new(',').into()),
    );
    let mutations = mutations.build().unwrap();

    let result = super::merge_ini(&mut tgt, &mut src, &mutations).unwrap();

    assert_eq!(EXPECTED, result.join("\n") + "\n");
}
