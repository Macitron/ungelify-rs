use crate::mpk::MagesEntry;
use indexmap::IndexMap;

pub struct Entries<'a> {
    entry_values: indexmap::map::Values<'a, u32, MagesEntry>,
}

impl Entries<'_> {
    pub(in crate::mpk) fn new(entry_map: &IndexMap<u32, MagesEntry>) -> Entries {
        Entries {
            entry_values: entry_map.values(),
        }
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = &'a MagesEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.entry_values.next()
    }
}
