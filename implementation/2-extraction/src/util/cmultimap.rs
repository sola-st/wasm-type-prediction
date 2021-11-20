use std::cmp::Reverse;
use std::collections::HashMap;
use std::hash::Hash;

use chashmap::CHashMap;
use itertools::Itertools;

/// Simple, concurrent multimap. That is, a mapping from keys of type `K` to
/// bags (unsorted multisets) of type `V`.
#[derive(Debug, Clone)]
pub struct CMultiMap<K, V>(CHashMap<K, HashMap<V, usize>>);

impl<K: Hash + PartialEq, V: Hash + Eq + Clone> CMultiMap<K, V> {
    pub fn new() -> Self {
        Self(CHashMap::new())
    }

    pub fn insert(&self, key: K, value: &V) {
        self.0.upsert(
            key,
            || {
                let mut map = HashMap::new();
                map.insert(value.clone(), 1);
                map
            },
            |map| {
                *map.entry(value.clone()).or_insert(0) += 1;
            }
        );
    }
}

pub struct IntoIter<K, V>(chashmap::IntoIter<K, HashMap<V, usize>>);

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, Vec<(V, usize)>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, vs)| {
            let mut vs = vs.into_iter().collect_vec();
            vs.sort_by_key(|(_v, count)| Reverse(*count));
            (k, vs)
        })
    }
}

impl<K, V> IntoIterator for CMultiMap<K, V> {
    type Item = (K, Vec<(V, usize)>);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}