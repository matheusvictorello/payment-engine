use crate::core::business::Store;
use std::collections::HashMap;
use std::hash::Hash;

impl<K, V> Store<K, V> for HashMap<K, V>
where
    K: Eq + Hash,
{
    fn remove(&mut self, key: &K) -> Option<V> {
        self.remove(key)
    }

    fn insert(&mut self, key: K, val: V) {
        self.insert(key, val);
    }
}
