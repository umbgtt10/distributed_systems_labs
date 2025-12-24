use map_reduce_core::state_access::StateAccess;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Thread-safe state access using Arc<Mutex<>>
#[derive(Clone)]
pub struct LocalStateAccess {
    map: Arc<Mutex<HashMap<String, Vec<usize>>>>,
}

impl LocalStateAccess {
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_map(&self) -> Arc<Mutex<HashMap<String, Vec<usize>>>> {
        Arc::clone(&self.map)
    }
}

impl StateAccess for LocalStateAccess {
    fn initialize(&self, keys: Vec<String>) {
        let mut map = self.map.lock().unwrap();
        for key in keys {
            map.insert(key, vec![0]);
        }
    }

    fn update(&self, key: String, value: i32) {
        let mut map = self.map.lock().unwrap();
        map.entry(key)
            .or_insert_with(|| vec![0])
            .push(value as usize);
    }

    fn replace(&self, key: String, value: i32) {
        let mut map = self.map.lock().unwrap();
        if let Some(values) = map.get_mut(&key) {
            if !values.is_empty() {
                values[0] = value as usize;
            } else {
                values.push(value as usize);
            }
        } else {
            map.insert(key, vec![value as usize]);
        }
    }

    fn get(&self, key: &str) -> Vec<i32> {
        let map = self.map.lock().unwrap();
        map.get(key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|v| v as i32)
            .collect()
    }
}
