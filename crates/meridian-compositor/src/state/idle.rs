use std::collections::HashSet;
use std::hash::Hash;

/// Tracks currently-active idle inhibitors keyed by `K`
/// (in production: `WlSurface`). The boolean returned from
/// add/remove indicates whether the *aggregate* inhibition
/// state transitioned (false→true on first add, true→false
/// on last remove). Callers use that to drive
/// `IdleNotifierState::set_is_inhibited`.
#[derive(Debug, Default)]
pub struct IdleInhibitorSet<K> {
    surfaces: HashSet<K>,
}

impl<K: Eq + Hash> IdleInhibitorSet<K> {
    pub fn new() -> Self {
        Self {
            surfaces: HashSet::new(),
        }
    }

    pub fn is_inhibited(&self) -> bool {
        !self.surfaces.is_empty()
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    /// Insert. Returns true if this transitioned 0→N inhibitors
    /// (i.e. the aggregate state flipped to inhibited).
    pub fn add(&mut self, key: K) -> bool {
        let was_empty = self.surfaces.is_empty();
        let inserted = self.surfaces.insert(key);
        inserted && was_empty
    }

    /// Remove. Returns true if this transitioned N→0 (i.e. the
    /// aggregate state flipped to NOT inhibited).
    pub fn remove(&mut self, key: &K) -> bool {
        let removed = self.surfaces.remove(key);
        removed && self.surfaces.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::IdleInhibitorSet;

    #[test]
    fn new_default_is_not_inhibited() {
        let set = IdleInhibitorSet::<String>::new();
        assert!(!set.is_inhibited());
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn first_add_transitions_to_inhibited_returns_true() {
        let mut set = IdleInhibitorSet::<String>::new();
        assert!(set.add("a".to_string()));
        assert!(set.is_inhibited());
        assert!(!set.is_empty());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn second_add_does_not_transition_returns_false() {
        let mut set = IdleInhibitorSet::<String>::new();
        assert!(set.add("a".to_string()));
        assert!(!set.add("b".to_string()));
        assert!(set.is_inhibited());
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn remove_non_last_returns_false_still_inhibited() {
        let mut set = IdleInhibitorSet::<String>::new();
        set.add("a".to_string());
        set.add("b".to_string());
        assert!(!set.remove(&"a".to_string()));
        assert!(set.is_inhibited());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn remove_last_returns_true_not_inhibited() {
        let mut set = IdleInhibitorSet::<String>::new();
        set.add("a".to_string());
        assert!(set.remove(&"a".to_string()));
        assert!(!set.is_inhibited());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn remove_unknown_key_returns_false_no_transition() {
        let mut set = IdleInhibitorSet::<String>::new();
        assert!(!set.remove(&"missing".to_string()));
        assert!(!set.is_inhibited());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn add_existing_key_returns_false() {
        let mut set = IdleInhibitorSet::<String>::new();
        assert!(set.add("a".to_string()));
        assert!(!set.add("a".to_string()));
        assert!(set.is_inhibited());
        assert_eq!(set.len(), 1);
    }
}
