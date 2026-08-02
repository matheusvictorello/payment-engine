pub trait Store<K, V> {
    fn remove(&mut self, key: &K) -> Option<V>;

    fn insert(&mut self, key: K, val: V);
}
