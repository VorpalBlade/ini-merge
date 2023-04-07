use ini_roundtrip::Parser;
use lending_iterator::prelude::*;
use ouroboros::self_referencing;
use std::io::Read;

#[self_referencing]
pub(crate) struct Loader {
    data: String,
    #[borrows(data)]
    #[covariant]
    parser: Parser<'this>,
}

// For now, this is how lending iterators work. I hope it switches to proper
// GATs some time soon.
#[gat]
impl LendingIterator for Loader {
    type Item<'next> = <Parser<'next> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item<'_>> {
        self.with_parser_mut(|parser| parser.next())
    }
}

pub(crate) fn load_ini(data: &mut impl Read) -> Result<Loader, std::io::Error> {
    let mut buf = String::new();
    data.read_to_string(&mut buf)?;
    Ok(LoaderBuilder {
        data: buf,
        parser_builder: |data: &String| Parser::new(data),
    }
    .build())
}
