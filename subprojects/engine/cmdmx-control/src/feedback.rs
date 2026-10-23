//! Feedback diffing: send only what changed (CTL-17..21).

use std::collections::HashMap;
use std::hash::Hash;

pub struct Feedback<K, V> {
    last: HashMap<K, V>,
    touched: Vec<K>,
}

impl<K, V> Default for Feedback<K, V> {
    fn default() -> Self {
        Feedback {
            last: HashMap::new(),
            touched: Vec::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V: PartialEq + Clone> Feedback<K, V> {
    /// `wanted`: what the engine says now. `echo`: what inputs set this tick
    /// (never sent back). `touched`: elements a hand is on (not sent until released).
    pub fn tick(
        &mut self,
        wanted: impl IntoIterator<Item = (K, V)>,
        echo: &[(K, V)],
        touched: &[K],
    ) -> Vec<(K, V)> {
        let mut order = Vec::new();
        let mut values: HashMap<K, V> = HashMap::new();
        for (k, v) in wanted {
            if values.insert(k.clone(), v).is_none() {
                order.push(k);
            }
        }
        for (k, v) in echo {
            self.last.insert(k.clone(), v.clone());
        }
        let mut out = Vec::new();
        for k in order {
            let Some(v) = values.remove(&k) else { continue };
            if echo.iter().any(|(e, _)| *e == k) || touched.contains(&k) {
                continue;
            }
            if self.touched.contains(&k) || self.last.get(&k) != Some(&v) {
                self.last.insert(k.clone(), v.clone());
                out.push((k, v));
            }
        }
        self.touched = touched.to_vec();
        out
    }

    /// Forget what was sent, so the full state goes out again.
    pub fn clear(&mut self) {
        self.last.clear();
    }
}
