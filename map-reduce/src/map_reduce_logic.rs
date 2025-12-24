use std::collections::HashMap;

/// Pure business logic for mapping phase
/// Searches for target words in data and returns counts
pub fn map_logic(data: &[String], targets: &[String]) -> HashMap<String, i32> {
    let mut results = HashMap::new();

    for target in targets {
        let mut count = 0;
        for text in data {
            if text.contains(target) {
                count += 1;
            }
        }
        results.insert(target.clone(), count);
    }

    results
}

/// Pure business logic for reduce phase
/// Sums all values for a given key
pub fn reduce_logic(values: Vec<i32>) -> i32 {
    values.into_iter().sum()
}
