//! DAG validation + level assignment (host `validate-plan.py` parity).

use super::parse::PrEntry;
use std::collections::{HashMap, HashSet, VecDeque};

/// Check unique IDs, valid dependency references, and no cycles.
pub fn validate_dag(entries: &[PrEntry]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        if !seen.insert(entry.id.clone()) {
            errors.push(format!("Duplicate PR ID: '{}'", entry.id));
        }
    }
    for entry in entries {
        for dep in &entry.dependencies {
            if !seen.contains(dep) {
                let dep_label = dep.replacen("pr-", "PR ", 1);
                let entry_label = entry.id.replacen("pr-", "PR ", 1);
                errors.push(format!(
                    "Dependency '{dep_label}' in {entry_label} does not reference a valid PR ID"
                ));
            }
        }
    }
    if errors.is_empty() {
        errors.extend(detect_cycles(entries));
    }
    errors
}

fn detect_cycles(entries: &[PrEntry]) -> Vec<String> {
    let mut in_degree: HashMap<String, usize> = entries.iter().map(|e| (e.id.clone(), 0)).collect();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let dep_map: HashMap<String, Vec<String>> = entries
        .iter()
        .map(|e| (e.id.clone(), e.dependencies.clone()))
        .collect();

    for entry in entries {
        for dep in &entry.dependencies {
            children
                .entry(dep.clone())
                .or_default()
                .push(entry.id.clone());
            *in_degree.entry(entry.id.clone()).or_default() += 1;
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0usize;

    while let Some(node) = queue.pop_front() {
        visited += 1;
        if let Some(kids) = children.get(&node) {
            for child in kids {
                if let Some(deg) = in_degree.get_mut(child) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }

    if visited == entries.len() {
        return vec![];
    }

    let unvisited: Vec<String> = entries
        .iter()
        .filter(|e| in_degree.get(&e.id).copied().unwrap_or(0) > 0)
        .map(|e| e.id.clone())
        .collect();
    if let Some(cycle) = trace_cycle(&dep_map, &unvisited) {
        return vec![format!("Cycle detected: {}", cycle.join(" -> "))];
    }
    let mut sorted = unvisited;
    sorted.sort();
    vec![format!("Cycle detected involving: {}", sorted.join(", "))]
}

fn trace_cycle(
    dep_map: &HashMap<String, Vec<String>>,
    unvisited_ids: &[String],
) -> Option<Vec<String>> {
    if unvisited_ids.is_empty() {
        return None;
    }
    let unvisited: HashSet<&str> = unvisited_ids.iter().map(String::as_str).collect();
    let mut current = unvisited_ids[0].as_str();
    let mut path = vec![current.to_owned()];
    let mut visited_in_path = HashSet::new();
    visited_in_path.insert(current.to_owned());

    loop {
        let mut next_node: Option<&str> = None;
        if let Some(deps) = dep_map.get(current) {
            for dep in deps {
                if unvisited.contains(dep.as_str()) {
                    next_node = Some(dep.as_str());
                    break;
                }
            }
        }
        let Some(next) = next_node else {
            break;
        };
        if visited_in_path.contains(next) {
            let idx = path.iter().position(|p| p == next)?;
            let mut cycle = path[idx..].to_vec();
            cycle.push(next.to_owned());
            return Some(cycle);
        }
        path.push(next.to_owned());
        visited_in_path.insert(next.to_owned());
        current = next;
    }
    None
}

fn pr_sort_key(pr_id: &str) -> (u8, i64, String) {
    let suffix = pr_id.split_once('-').map(|(_, s)| s).unwrap_or(pr_id);
    if let Ok(n) = suffix.parse::<i64>() {
        (0, n, String::new())
    } else {
        (1, 0, suffix.to_owned())
    }
}

/// Return `{pr_id: level}`; level 0 = no deps.
pub fn compute_levels(entries: &[PrEntry]) -> HashMap<String, u32> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = entries.iter().map(|e| (e.id.clone(), 0)).collect();

    for e in entries {
        for dep in &e.dependencies {
            children.entry(dep.clone()).or_default().push(e.id.clone());
            *in_degree.entry(e.id.clone()).or_default() += 1;
        }
    }

    let mut levels: HashMap<String, u32> = HashMap::new();
    let mut queue = VecDeque::new();
    for (eid, deg) in &in_degree {
        if *deg == 0 {
            levels.insert(eid.clone(), 0);
            queue.push_back(eid.clone());
        }
    }

    while let Some(node) = queue.pop_front() {
        let node_level = levels.get(&node).copied().unwrap_or(0);
        if let Some(kids) = children.get(&node) {
            for child in kids {
                let candidate = node_level + 1;
                let cur = levels.get(child).copied().unwrap_or(0);
                levels.insert(child.clone(), cur.max(candidate));
                if let Some(deg) = in_degree.get_mut(child) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }
    levels
}

/// Flatten the DAG into Graphite stack order (stable within levels).
pub fn linearize(entries: &[PrEntry], levels: &HashMap<String, u32>) -> Vec<String> {
    let mut by_level: HashMap<u32, Vec<String>> = HashMap::new();
    for e in entries {
        let lv = levels.get(&e.id).copied().unwrap_or(0);
        by_level.entry(lv).or_default().push(e.id.clone());
    }
    let mut level_keys: Vec<u32> = by_level.keys().copied().collect();
    level_keys.sort_unstable();
    let mut order = Vec::new();
    for lv in level_keys {
        if let Some(ids) = by_level.get_mut(&lv) {
            ids.sort_by_key(|a| pr_sort_key(a));
            order.extend(ids.iter().cloned());
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::plan_validate::parse::PrEntry;

    fn entry(id: &str, deps: &[&str]) -> PrEntry {
        PrEntry {
            id: id.to_owned(),
            number: id.trim_start_matches("pr-").to_owned(),
            title: id.to_owned(),
            files: vec![],
            dependencies: deps.iter().map(|s| (*s).to_owned()).collect(),
            description: String::new(),
        }
    }

    #[test]
    fn levels_and_linearize() {
        let entries = vec![
            entry("pr-0", &[]),
            entry("pr-1", &["pr-0"]),
            entry("pr-2", &["pr-0"]),
        ];
        assert!(validate_dag(&entries).is_empty());
        let levels = compute_levels(&entries);
        assert_eq!(levels["pr-0"], 0);
        assert_eq!(levels["pr-1"], 1);
        assert_eq!(levels["pr-2"], 1);
        let order = linearize(&entries, &levels);
        assert_eq!(order[0], "pr-0");
        assert!(order[1..].contains(&"pr-1".to_owned()));
        assert!(order[1..].contains(&"pr-2".to_owned()));
    }

    #[test]
    fn numeric_sort_pr_2_before_pr_10() {
        let entries = vec![entry("pr-2", &[]), entry("pr-10", &[])];
        let levels = compute_levels(&entries);
        let order = linearize(&entries, &levels);
        assert_eq!(order, vec!["pr-2".to_owned(), "pr-10".to_owned()]);
    }
}
