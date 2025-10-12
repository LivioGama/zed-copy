use std::cmp;

/// A change in a diff
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffChange {
    /// Lines that are the same in both versions
    Equal { old_index: usize, new_index: usize, count: usize },
    /// Lines deleted from old version
    Delete { old_index: usize, count: usize },
    /// Lines inserted in new version
    Insert { new_index: usize, count: usize },
    /// Lines replaced (delete + insert)
    Replace { 
        old_index: usize, 
        old_count: usize, 
        new_index: usize, 
        new_count: usize 
    },
}

/// Compute diff between two texts using Myers' algorithm
pub fn diff_lines(old_text: &str, new_text: &str) -> Vec<DiffChange> {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let n = old_lines.len();
    let m = new_lines.len();
    let max = n + m;

    let mut v: Vec<isize> = vec![0; 2 * max + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    // Myers' algorithm
    'outer: for d in 0..=max {
        trace.push(v.clone());
        
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let mut x = if k == -(d as isize) || (k != d as isize && v[(max as isize + k - 1) as usize] < v[(max as isize + k + 1) as usize]) {
                v[(max as isize + k + 1) as usize]
            } else {
                v[(max as isize + k - 1) as usize] + 1
            };

            let mut y = x - k;

            while x < n as isize && y < m as isize && old_lines[x as usize] == new_lines[y as usize] {
                x += 1;
                y += 1;
            }

            v[(max as isize + k) as usize] = x;

            if x >= n as isize && y >= m as isize {
                break 'outer;
            }
        }
    }

    // Backtrack to find the actual diff
    let mut changes = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;

    for (d, v) in trace.iter().enumerate().rev() {
        let k = x - y;
        let prev_k = if k == -(d as isize) || (k != d as isize && v[(max as isize + k - 1) as usize] < v[(max as isize + k + 1) as usize]) {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v[(max as isize + prev_k) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
        }

        if d > 0 {
            if x == prev_x {
                // Insert
                changes.push(DiffChange::Insert { 
                    new_index: y as usize - 1, 
                    count: 1 
                });
            } else if y == prev_y {
                // Delete
                changes.push(DiffChange::Delete { 
                    old_index: x as usize - 1, 
                    count: 1 
                });
            }
        }

        x = prev_x;
        y = prev_y;
    }

    changes.reverse();
    merge_adjacent_changes(changes)
}

/// Merge adjacent changes of the same type
fn merge_adjacent_changes(changes: Vec<DiffChange>) -> Vec<DiffChange> {
    if changes.is_empty() {
        return changes;
    }

    let mut merged = Vec::new();
    let mut current = changes[0].clone();

    for change in changes.into_iter().skip(1) {
        match (&current, &change) {
            (
                DiffChange::Delete { old_index: i1, count: c1 },
                DiffChange::Delete { old_index: i2, count: c2 },
            ) if i1 + c1 == *i2 => {
                current = DiffChange::Delete { 
                    old_index: *i1, 
                    count: c1 + c2 
                };
            }
            (
                DiffChange::Insert { new_index: i1, count: c1 },
                DiffChange::Insert { new_index: i2, count: c2 },
            ) if i1 + c1 == *i2 => {
                current = DiffChange::Insert { 
                    new_index: *i1, 
                    count: c1 + c2 
                };
            }
            _ => {
                merged.push(current);
                current = change;
            }
        }
    }

    merged.push(current);
    merged
}

/// Format diff as a unified diff string
pub fn format_unified_diff(old_text: &str, new_text: &str, old_name: &str, new_name: &str) -> String {
    let changes = diff_lines(old_text, new_text);
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let mut output = String::new();
    output.push_str(&format!("--- {}\n", old_name));
    output.push_str(&format!("+++ {}\n", new_name));

    for change in changes {
        match change {
            DiffChange::Delete { old_index, count } => {
                output.push_str(&format!("@@ -{},{} @@\n", old_index + 1, count));
                for i in 0..count {
                    output.push_str(&format!("-{}\n", old_lines[old_index + i]));
                }
            }
            DiffChange::Insert { new_index, count } => {
                output.push_str(&format!("@@ +{},{} @@\n", new_index + 1, count));
                for i in 0..count {
                    output.push_str(&format!("+{}\n", new_lines[new_index + i]));
                }
            }
            DiffChange::Replace { old_index, old_count, new_index, new_count } => {
                output.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    old_index + 1, old_count, new_index + 1, new_count
                ));
                for i in 0..old_count {
                    output.push_str(&format!("-{}\n", old_lines[old_index + i]));
                }
                for i in 0..new_count {
                    output.push_str(&format!("+{}\n", new_lines[new_index + i]));
                }
            }
            DiffChange::Equal { old_index, new_index: _, count } => {
                for i in 0..cmp::min(count, 3) {
                    output.push_str(&format!(" {}\n", old_lines[old_index + i]));
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_simple() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2_modified\nline3";
        
        let changes = diff_lines(old, new);
        assert!(!changes.is_empty());
    }

    #[test]
    fn test_diff_insert() {
        let old = "line1\nline3";
        let new = "line1\nline2\nline3";
        
        let changes = diff_lines(old, new);
        assert!(changes.iter().any(|c| matches!(c, DiffChange::Insert { .. })));
    }

    #[test]
    fn test_diff_delete() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";
        
        let changes = diff_lines(old, new);
        assert!(changes.iter().any(|c| matches!(c, DiffChange::Delete { .. })));
    }
}
