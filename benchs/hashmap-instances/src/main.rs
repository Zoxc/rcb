use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

fn map<K: Hash + Debug + Eq + Clone, V: Debug>(k: K, v: V) {
    let mut map = HashMap::new();
    map.insert(k.clone(), v);
    map.reserve(1000);
    dbg!(map.get(&k), map.iter().next());
}

fn values<K: Hash + Debug + Eq + Clone>(k: K) {
    map(k.clone(), ());
    map(k.clone(), "");
    map(k.clone(), true);
    map(k.clone(), 1i8);
    map(k.clone(), 1u8);
    map(k.clone(), 1u32);
    map(k.clone(), 1i32);
    map(k.clone(), vec![1u32]);
    map(k.clone(), vec![1i32]);
}

fn main() {
    values(());
    values("");
    values(true);
    values(1i8);
    values(1u8);
    values(1u64);
    values(1i64);
    values(1usize);
    values(1isize);
    values(String::new());
    values(vec![""]);
    values(vec![1u32]);
    values(vec![1i32]);
}
