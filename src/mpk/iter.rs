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

pub struct EntriesMut<'a> {
    entry_values: indexmap::map::ValuesMut<'a, u32, MagesEntry>,
}

impl EntriesMut<'_> {
    pub(in crate::mpk) fn new(entry_map: &mut IndexMap<u32, MagesEntry>) -> EntriesMut {
        EntriesMut {
            entry_values: entry_map.values_mut(),
        }
    }
}

impl<'a> Iterator for EntriesMut<'a> {
    type Item = &'a mut MagesEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.entry_values.next()
    }
}

pub struct IntoEntries {
    entry_values: indexmap::map::IntoValues<u32, MagesEntry>,
}

impl IntoEntries {
    pub(in crate::mpk) fn new(entry_map: IndexMap<u32, MagesEntry>) -> Self {
        Self {
            entry_values: entry_map.into_values(),
        }
    }
}

impl Iterator for IntoEntries {
    type Item = MagesEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.entry_values.next()
    }
}
