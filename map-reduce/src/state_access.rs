use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait for accessing shared state across workers
/// Abstracts the storage mechanism (local, Redis, RPC, etc.)
pub trait StateAccess: Clone + Send + Sync + 'static {
    /// Update a key with a value (append for mappers)
    fn update(&self, key: String, value: i32);

    /// Replace the entire value for a key (used by reducers)
    fn replace(&self, key: String, value: i32);

    /// Get all values for a key
    fn get(&self, key: &str) -> Vec<i32>;

    /// Initialize keys with empty vectors
    fn initialize(&self, keys: Vec<String>);
}

/// Local in-memory state using Arc<Mutex<HashMap>>
#[derive(Clone)]
pub struct LocalStateAccess {
    map: Arc<Mutex<HashMap<String, Vec<i32>>>>,
}

impl LocalStateAccess {
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the underlying map for result extraction
    pub fn get_map(&self) -> Arc<Mutex<HashMap<String, Vec<i32>>>> {
        self.map.clone()
    }
}

impl StateAccess for LocalStateAccess {
    fn update(&self, key: String, value: i32) {
        let mut map = self.map.lock().unwrap();
        map.entry(key).or_default().push(value);
    }

    fn replace(&self, key: String, value: i32) {
        let mut map = self.map.lock().unwrap();
        map.insert(key, vec![value]);
    }

    fn get(&self, key: &str) -> Vec<i32> {
        let map = self.map.lock().unwrap();
        map.get(key).cloned().unwrap_or_default()
    }

    fn initialize(&self, keys: Vec<String>) {
        let mut map = self.map.lock().unwrap();
        for key in keys {
            map.entry(key).or_default();
        }
    }
}
