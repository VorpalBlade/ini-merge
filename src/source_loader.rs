use lending_iterator::prelude::*;
use std::{borrow::Cow, collections::HashMap, io::Read};
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SectionAndKey<'a>(Cow<'a, str>, Cow<'a, str>);

impl<'a> SectionAndKey<'a> {
    pub(crate) fn new(section: Cow<'a, str>, key: Cow<'a, str>) -> Self {
        Self(section, key)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourceValue {
    raw_line: String,
    val: Option<String>,
}

/// Error type for loading the source.
#[derive(Debug, Error)]
pub(crate) enum SourceLoaderError {
    #[error("Failed to load due to IO error: {0}")]
    Load(#[source] std::io::Error),
    #[error("Parse error {0}")]
    Parse(String),
}

impl SourceValue {
    pub(crate) fn new(raw_line: String, value: Option<String>) -> Self {
        Self {
            raw_line,
            val: value,
        }
    }

    pub(crate) fn raw(&self) -> &str {
        self.raw_line.as_str()
    }

    pub(crate) fn value(&self) -> Option<&str> {
        self.val.as_deref()
    }
}

#[derive(Debug, Default)]
pub(crate) struct SourceIni {
    section_headers: HashMap<String, String>,
    values: HashMap<SectionAndKey<'static>, SourceValue>,
}

impl SourceIni {
    /// Iterator over all sections
    pub(crate) fn sections(&self) -> impl Iterator<Item = (&String, &String)> {
        self.section_headers.iter()
    }

    /// True if the section exists in the source
    pub(crate) fn has_section(&self, name: &str) -> bool {
        self.section_headers.contains_key(name)
    }

    /// Get all entries in a section
    pub(crate) fn section_entries(
        &self,
        name: String,
    ) -> impl Iterator<Item = (&Cow<'_, str>, &'_ SourceValue)> {
        self.values
            .iter()
            .filter_map(move |(k, v)| if k.0 == name { Some((&k.1, v)) } else { None })
    }

    /// Get a specific entry for a section & key
    pub(crate) fn property<'result, 'key: 'result, 'this: 'result>(
        &'this self,
        item: &SectionAndKey<'key>,
    ) -> Option<&'result SourceValue> {
        self.values.get(item)
    }
}

pub(crate) fn load_source_ini(data: &mut impl Read) -> Result<SourceIni, SourceLoaderError> {
    let mut loader = crate::loader::load_ini(data).map_err(SourceLoaderError::Load)?;
    let mut result = SourceIni::default();
    let mut cur_section = crate::OUTSIDE_SECTION.to_string();
    result
        .section_headers
        .insert(cur_section.clone(), cur_section.clone());

    while let Some(ref item) = loader.next() {
        match *item {
            ini_roundtrip::Item::Error(err) => return Err(SourceLoaderError::Parse(err.into())),
            ini_roundtrip::Item::Section { name, raw } => {
                result
                    .section_headers
                    .insert(name.to_string(), raw.to_string());
                cur_section.clear();
                cur_section.push_str(name);
            }
            ini_roundtrip::Item::SectionEnd => (),
            ini_roundtrip::Item::Property { key, val, raw } => {
                result.values.insert(
                    SectionAndKey(cur_section.clone().into(), key.to_string().into()),
                    SourceValue::new(raw.to_string(), val.map(str::to_string)),
                );
            }
            ini_roundtrip::Item::Comment { raw: _ } => (),
            ini_roundtrip::Item::Blank { raw: _ } => (),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::{
        source_loader::{SectionAndKey, SourceValue},
        OUTSIDE_SECTION,
    };

    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use std::collections::VecDeque;

    /// Test data
    const TEST_DATA: &str = indoc! {"
    ; Some terrible INI (as seen in the wild)
    # With different comments
    firstkey=1
    [section]
    a = 2
    b = 3

    [sec2][aaa]
    a =   9
    "};

    #[test]
    fn load_basic_ini() {
        let mut mut_data: VecDeque<_> = TEST_DATA.as_bytes().to_owned().into();
        let result = super::load_source_ini(&mut mut_data).unwrap();

        assert_eq!(result.section_headers.len(), 3);
        assert_eq!(
            result.section_headers.get(OUTSIDE_SECTION).unwrap(),
            OUTSIDE_SECTION
        );
        assert_eq!(result.section_headers.get("section").unwrap(), "[section]");
        assert_eq!(
            result.section_headers.get("sec2][aaa").unwrap(),
            "[sec2][aaa]"
        );

        assert_eq!(result.values.len(), 4);
        assert_eq!(
            *result
                .values
                .get(&SectionAndKey(OUTSIDE_SECTION.into(), "firstkey".into()))
                .unwrap(),
            SourceValue::new("firstkey=1".into(), Some("1".into()))
        );
        assert_eq!(
            *result
                .values
                .get(&SectionAndKey("section".into(), "a".into()))
                .unwrap(),
            SourceValue::new("a = 2".into(), Some("2".into()))
        );
        assert_eq!(
            *result
                .values
                .get(&SectionAndKey("section".into(), "b".into()))
                .unwrap(),
            SourceValue::new("b = 3".into(), Some("3".into()))
        );
        assert_eq!(
            *result
                .values
                .get(&SectionAndKey("sec2][aaa".into(), "a".into()))
                .unwrap(),
            SourceValue::new("a =   9".into(), Some("9".into()))
        );
    }
}
